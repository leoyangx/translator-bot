"""日志：控制台 + 按天重置的文件，统一 UTF-8，避免 Windows 控制台乱码。"""
from __future__ import annotations

import logging
import sys
from datetime import date, datetime, timedelta
from logging.handlers import RotatingFileHandler
from pathlib import Path

_FORMAT = "%(asctime)s %(levelname)-7s %(name)-18s %(message)s"
_KEEP = ("bot.log", "app.log", "host.log")


def force_utf8_stdio() -> None:
    for stream in (sys.stdout, sys.stderr):
        if stream is None:  # pythonw 下没有控制台
            continue
        try:
            if (stream.encoding or "").lower() not in ("utf-8", "utf8"):
                stream.reconfigure(encoding="utf-8", errors="replace")
        except Exception:  # 某些环境下 stream 不可重配置，忽略
            pass


class DailyResetHandler(logging.FileHandler):
    """每天零点自动清空日志文件，只保留当天内容。"""

    def __init__(self, path: Path) -> None:
        super().__init__(path, mode="a", encoding="utf-8")
        self._day = date.today()

    def emit(self, record: logging.LogRecord) -> None:
        today = date.today()
        if today != self._day:
            self._day = today
            try:
                self.acquire()
                try:
                    if self.stream:
                        self.stream.close()
                    self.stream = self._open_truncate()
                finally:
                    self.release()
            except Exception:
                pass
        super().emit(record)

    def _open_truncate(self):
        return open(self.baseFilename, "w", encoding="utf-8")  # type: ignore[arg-type]


def cleanup_old_logs(log_dir: Path) -> None:
    """删掉不是今天的日志文件（含历史轮转文件）。"""
    if not log_dir.exists():
        return
    today = date.today()
    for path in log_dir.glob("*.log*"):
        try:
            if path.name in _KEEP:
                continue
            if datetime.fromtimestamp(path.stat().st_mtime).date() < today:
                path.unlink(missing_ok=True)
        except OSError:
            pass


def truncate_other_logs(log_dir: Path) -> None:
    """把非 Python 管理的日志（如 host.log）清空。"""
    for name in _KEEP:
        path = log_dir / name
        try:
            if path.exists():
                path.write_text("", encoding="utf-8")
        except OSError:
            pass


def seconds_until_midnight(now: datetime | None = None) -> float:
    now = now or datetime.now()
    tomorrow = (now + timedelta(days=1)).replace(
        hour=0, minute=0, second=10, microsecond=0
    )
    return max(30.0, (tomorrow - now).total_seconds())


def setup_logging(log_dir: Path, level: int = logging.INFO, filename: str = "bot.log") -> None:
    log_dir.mkdir(parents=True, exist_ok=True)
    cleanup_old_logs(log_dir)

    root = logging.getLogger()
    root.setLevel(level)
    root.handlers.clear()

    formatter = logging.Formatter(_FORMAT)

    file_handler = DailyResetHandler(log_dir / filename)
    file_handler.setFormatter(formatter)
    root.addHandler(file_handler)

    if sys.stdout is not None:
        console = logging.StreamHandler(sys.stdout)
        console.setFormatter(formatter)
        root.addHandler(console)

    logging.getLogger("aiogram").setLevel(logging.WARNING)
    logging.getLogger("aiohttp").setLevel(logging.WARNING)
