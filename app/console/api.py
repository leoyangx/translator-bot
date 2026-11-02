"""桌面控制台桥接层：暴露给窗口页面的所有能力。"""
from __future__ import annotations

import asyncio
import json
import os
import shutil
import subprocess
import sys
import threading
import time
import zipfile
from pathlib import Path
from typing import Any

import aiohttp

from ..config import (
    APP_VERSION,
    Config,
    ConfigError,
    config_from_dict,
    config_to_dict,
    load_config,
    save_config,
)
from ..core.languages import LANGUAGES, language_name, resolve_direction
from ..core.prompts import (
    DEFAULT_EXPLAIN_PROMPT,
    DEFAULT_TRANSLATION_PROMPT,
    TONE_NAMES,
)
from ..core.translator import Translator
from ..paths import app_root
from ..providers.openai_compat import OpenAICompatProvider
from ..storage.db import Database
from ..storage.repositories import Repo

CREATE_NO_WINDOW = 0x08000000 if os.name == "nt" else 0


class BotProcess:
    """机器人跑在后端进程内的线程里，任务管理器只显示两个进程。"""

    def __init__(self) -> None:
        self.proc = None
        self.started_at: float = 0.0
        self._thread: threading.Thread | None = None
        self._loop: asyncio.AbstractEventLoop | None = None
        self._running = False

    def is_running(self) -> bool:
        return bool(self._thread and self._thread.is_alive() and self._running)

    def _worker(self) -> None:
        loop = asyncio.new_event_loop()
        self._loop = loop
        asyncio.set_event_loop(loop)
        try:
            loop.run_until_complete(self._main())
        finally:
            try:
                loop.run_until_complete(loop.shutdown_asyncgens())
            finally:
                loop.close()

    async def _main(self) -> None:
        from ..main import run as bot_run

        self._running = True
        try:
            await bot_run(setup_logs=False)
        except Exception:  # noqa: BLE001
            log.exception("机器人异常退出")
        finally:
            self._running = False

    def start(self) -> tuple[bool, str]:
        if self.is_running():
            return True, "已经在运行"
        self._thread = threading.Thread(target=self._worker, name="bot-runner", daemon=True)
        self.started_at = time.time()
        self._thread.start()
        time.sleep(1.2)
        if not self._running and self._thread and not self._thread.is_alive():
            return False, "启动失败，请查看日志"
        return True, "已启动"

    def stop(self) -> tuple[bool, str]:
        if not self.is_running():
            self._thread = None
            return True, "本来就没在运行"
        loop = self._loop
        if loop is not None:
            try:
                for task in asyncio.all_tasks(loop):
                    loop.call_soon_threadsafe(task.cancel)
            except RuntimeError:
                pass
        if self._thread is not None:
            self._thread.join(timeout=10)
        self._running = False
        self._thread = None
        self._loop = None
        return True, "已停止"

    def restart(self) -> tuple[bool, str]:
        self.stop()
        time.sleep(0.6)
        return self.start()


class ConsoleApi:
    def __init__(self) -> None:
        self.root = app_root()
        self.bot = BotProcess()
        self.window = None
        self.tray = None
        self.on_quit = None
        self._lock = threading.Lock()
        self._update_info: dict[str, Any] = {}

    # ---------- 基础状态 ----------
    def get_state(self) -> dict[str, Any]:
        cfg = self._load()
        return {
            "version": APP_VERSION,
            "config": config_to_dict(cfg),
            "status": self._status(),
            "paths": {
                "root": str(self.root),
                "logs": str(self.root / "logs"),
                "data": str(self.root / "data"),
            },
            "languages": [
                {"code": code, "name": meta["zh"], "en": meta["en"]}
                for code, meta in LANGUAGES.items()
            ],
            "tones": [{"code": k, "name": v} for k, v in TONE_NAMES.items()],
            "defaults": {
                "translation_prompt": DEFAULT_TRANSLATION_PROMPT,
                "explain_prompt": DEFAULT_EXPLAIN_PROMPT,
            },
            "update": self._update_info,
            "frozen": bool(getattr(sys, "frozen", False)),
        }

    def _load(self) -> Config:
        try:
            return load_config()
        except ConfigError:
            from ..config import ensure_config

            return ensure_config()

    def load_config(self) -> Config:
        return self._load()

    def _status(self) -> dict[str, Any]:
        running = self.bot.is_running()
        uptime = ""
        if running and self.bot.started_at:
            seconds = int(time.time() - self.bot.started_at)
            uptime = f"{seconds // 60} 分 {seconds % 60} 秒"
        return {
            "running": running,
            "pid": os.getpid() if running else None,
            "uptime": uptime,
            "started_at": self.bot.started_at if running else None,
        }

    def bot_status(self) -> dict[str, Any]:
        return self._status()

    # ---------- 配置 ----------
    def save_config(self, payload: dict[str, Any]) -> dict[str, Any]:
        try:
            cfg = config_from_dict(payload)
        except Exception as exc:  # noqa: BLE001
            return {"ok": False, "message": f"配置格式错误：{exc}"}

        issues = cfg.validate()
        try:
            save_config(cfg)
        except OSError as exc:
            return {"ok": False, "message": f"保存失败：{exc}"}

        restart_needed = self.bot.is_running()
        return {
            "ok": True,
            "message": "已保存",
            "issues": issues,
            "needs_restart": restart_needed,
            "status": self._status(),
        }

    def validate(self, payload: dict[str, Any]) -> dict[str, Any]:
        cfg = config_from_dict(payload)
        return {"issues": cfg.validate()}

    # ---------- 进程控制 ----------
    def start_bot(self) -> dict[str, Any]:
        ok, msg = self.bot.start()
        return {"ok": ok, "message": msg, "status": self._status()}

    def stop_bot(self) -> dict[str, Any]:
        ok, msg = self.bot.stop()
        return {"ok": ok, "message": msg, "status": self._status()}

    def restart_bot(self) -> dict[str, Any]:
        ok, msg = self.bot.restart()
        return {"ok": ok, "message": msg, "status": self._status()}

    # ---------- 模型 ----------
    def test_token(self, token: str = "") -> dict[str, Any]:
        token = (token or "").strip()
        if not token:
            return {"ok": False, "message": "先填 Bot Token"}

        async def run() -> dict[str, Any]:
            cfg = self._load()
            proxy = cfg.proxy or None
            timeout = aiohttp.ClientTimeout(total=15)
            try:
                async with aiohttp.ClientSession(timeout=timeout) as session:
                    async with session.get(
                        f"https://api.telegram.org/bot{token}/getMe", proxy=proxy
                    ) as resp:
                        data = await resp.json(content_type=None)
            except Exception as exc:  # noqa: BLE001
                return {"ok": False, "message": f"连不上 Telegram：{exc}"}
            if not data.get("ok"):
                return {
                    "ok": False,
                    "message": data.get("description") or "Token 无效",
                }
            user = data.get("result") or {}
            name = user.get("username") or user.get("first_name") or ""
            return {
                "ok": True,
                "message": f"已连接 @{name}",
                "username": user.get("username"),
            }

        return self._run_async(run())

    def test_model(self, model: dict[str, Any]) -> dict[str, Any]:
        base_url = str(model.get("base_url") or "").strip()
        api_key = str(model.get("api_key") or "").strip()
        model_name = str(model.get("model") or "").strip()
        if not base_url or not api_key or not model_name:
            return {"ok": False, "message": "接口地址、API Key、模型名都要填"}

        async def run() -> dict[str, Any]:
            provider = OpenAICompatProvider(base_url, api_key, model_name, timeout=20)
            started = time.perf_counter()
            try:
                models = await provider.models()
                result = await provider.chat(
                    [{"role": "user", "content": "回复一个字：好"}], max_tokens=16
                )
                elapsed = int((time.perf_counter() - started) * 1000)
                return {
                    "ok": True,
                    "message": f"连接正常（{elapsed} ms）",
                    "latency_ms": elapsed,
                    "models": models,
                    "sample": result.text[:40],
                }
            except Exception as exc:  # noqa: BLE001
                return {"ok": False, "message": str(exc)}
            finally:
                await provider.close()

        return self._run_async(run())

    def list_models(self, model: dict[str, Any]) -> dict[str, Any]:
        base_url = str(model.get("base_url") or "").strip()
        api_key = str(model.get("api_key") or "").strip()
        if not base_url or not api_key:
            return {"ok": False, "message": "先填接口地址和 API Key"}

        async def run() -> dict[str, Any]:
            provider = OpenAICompatProvider(base_url, api_key, "x", timeout=15)
            try:
                return {"ok": True, "models": await provider.models()}
            except Exception as exc:  # noqa: BLE001
                return {"ok": False, "message": str(exc)}
            finally:
                await provider.close()

        return self._run_async(run())

    # ---------- 试译 ----------
    def test_translate(self, text: str, tone: str = "natural") -> dict[str, Any]:
        text = (text or "").strip()
        if not text:
            return {"ok": False, "message": "先输入要翻译的内容"}

        cfg = self._load()
        model = cfg.default_model
        if model is None:
            return {"ok": False, "message": "还没有可用的模型"}

        src, tgt, detected = resolve_direction(
            text, cfg.languages.mine, cfg.languages.partners
        )

        async def run() -> dict[str, Any]:
            provider = OpenAICompatProvider(
                model.base_url, model.api_key, model.model, timeout=45, proxy=cfg.proxy or None
            )
            db = Database(self.root / "data" / "bot.db")
            db.connect()
            try:
                translator = Translator(
                    provider,
                    Repo(db),
                    model=model.model,
                    max_input_chars=cfg.features.max_input_chars,
                    prompt_template=cfg.prompts.translation,
                    explain_template=cfg.prompts.explain,
                            )
                started = time.perf_counter()
                result = await translator.translate(
                    text, src=src, tgt=tgt, tone=tone, use_cache=False
                )
                elapsed = int((time.perf_counter() - started) * 1000)
                return {
                    "ok": True,
                    "translation": result.target_text,
                    "src": result.src_lang,
                    "tgt": result.tgt_lang,
                    "detected": detected,
                    "src_name": language_name(result.src_lang),
                    "tgt_name": language_name(result.tgt_lang),
                    "elapsed_ms": elapsed,
                    "tokens": result.prompt_tokens + result.completion_tokens,
                }
            except Exception as exc:  # noqa: BLE001
                return {"ok": False, "message": str(exc)}
            finally:
                await provider.close()
                db.close()

        return self._run_async(run())

    # ---------- 日志 ----------
    def read_logs(self, limit: int = 200) -> list[str]:
        path = self.root / "logs" / "bot.log"
        if not path.exists():
            return ["还没有日志。"]
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        return lines[-max(1, min(limit, 1000)):]

    def open_folder(self, kind: str = "logs") -> dict[str, Any]:
        mapping = {
            "logs": self.root / "logs",
            "data": self.root / "data",
            "root": self.root,
        }
        target = mapping.get(kind, self.root / "logs")
        try:
            target.mkdir(parents=True, exist_ok=True)
            os.startfile(str(target))  # type: ignore[attr-defined]
            return {"ok": True}
        except Exception as exc:  # noqa: BLE001
            return {"ok": False, "message": str(exc)}

    def get_stats(self) -> dict[str, Any]:
        try:
            db = Database(self.root / "data" / "bot.db")
            db.connect()
            repo = Repo(db)
            today = repo.usage_today()
            cached = repo.cache_count()
            db.close()
            return {
                "ok": True,
                "calls": today["calls"],
                "tokens": today["prompt_tokens"] + today["completion_tokens"],
                "input_tokens": today["prompt_tokens"],
                "output_tokens": today["completion_tokens"],
                "cache": cached,
            }
        except Exception as exc:  # noqa: BLE001
            return {"ok": False, "message": str(exc)}

    def export_diagnostics(self) -> dict[str, Any]:
        stamp = time.strftime("%Y%m%d-%H%M%S")
        out_dir = self.root / "data" / "diagnostics"
        out_dir.mkdir(parents=True, exist_ok=True)
        target = out_dir / f"diagnostics-{stamp}.zip"
        try:
            cfg = self._load()
            masked = config_to_dict(cfg)
            for model in masked.get("models", []):
                key = model.get("api_key") or ""
                model["api_key"] = f"***{key[-4:]}" if len(key) > 4 else "***"
            masked["telegram"]["token"] = "***"
            with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as zf:
                zf.writestr("config.masked.json", json.dumps(masked, ensure_ascii=False, indent=2))
                zf.writestr("status.json", json.dumps(self._status(), ensure_ascii=False, indent=2))
                log_path = self.root / "logs" / "bot.log"
                if log_path.exists():
                    zf.write(log_path, "bot.log")
            return {"ok": True, "path": str(target)}
        except Exception as exc:  # noqa: BLE001
            return {"ok": False, "message": str(exc)}

    # ---------- 开机自启 ----------
    def get_autostart(self) -> dict[str, Any]:
        return {"enabled": self._startup_link().exists(), "path": str(self._startup_link())}

    def set_autostart(self, enabled: bool) -> dict[str, Any]:
        link = self._startup_link()
        try:
            if enabled:
                self._create_startup_link(link)
                return {"ok": True, "enabled": True}
            if link.exists():
                link.unlink()
            return {"ok": True, "enabled": False}
        except Exception as exc:  # noqa: BLE001
            return {"ok": False, "message": str(exc)}

    def _startup_link(self) -> Path:
        startup = Path(os.environ["APPDATA"]) / "Microsoft" / "Windows" / "Start Menu" / "Programs" / "Startup"
        return startup / "TranslatorBot.lnk"

    def _host_exe(self) -> Path:
        """开机自启要拉起窗口宿主，而不是后端。"""
        if getattr(sys, "frozen", False):
            # 绿色版：backend\TranslatorBotBackend.exe → 上级目录的 TranslatorBot.exe
            portable = Path(sys.executable).resolve().parent.parent / "TranslatorBot.exe"
            if portable.exists():
                return portable
            # 安装版：程序在 %LOCALAPPDATA%\Programs\TranslatorBot
            local = Path(os.environ.get("LOCALAPPDATA", "")) / "Programs" / "TranslatorBot" / "TranslatorBot.exe"
            if local.exists():
                return local
            return Path(sys.executable)
        return self.root / ".venv" / "Scripts" / "pythonw.exe"

    def _create_startup_link(self, link: Path) -> None:
        if getattr(sys, "frozen", False):
            target = self._host_exe()
            args = ""
        else:
            target = self._host_exe()
            args = "-m app.console.serve"
        ps = (
            "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('%s');"
            "$s.TargetPath='%s';$s.Arguments='%s';$s.WorkingDirectory='%s';$s.Save()"
            % (link, target, args, self.root)
        )
        subprocess.run(
            ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps],
            check=True,
            creationflags=CREATE_NO_WINDOW,
        )

    # ---------- 窗口 ----------
    def quit_app(self) -> dict[str, Any]:
        try:
            self.bot.stop()
        except Exception:  # noqa: BLE001
            pass
        if callable(self.on_quit):
            try:
                self.on_quit()
            except Exception:  # noqa: BLE001
                pass
        return {"ok": True}

    # ---------- 工具 ----------
    @staticmethod
    def _run_async(coro):
        loop = asyncio.new_event_loop()
        try:
            return loop.run_until_complete(coro)
        finally:
            loop.close()
