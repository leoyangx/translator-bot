"""机器人入口：装配依赖、启动长轮询。"""
from __future__ import annotations

import asyncio
import logging

from aiogram import Bot
from aiogram.client.default import DefaultBotProperties
from aiogram.client.session.aiohttp import AiohttpSession
from aiogram.enums import ParseMode
from aiogram.types import BotCommand, ErrorEvent

from .bot.app import build_dispatcher
from .bot.context import ContextStore
from .config import APP_VERSION, ConfigError, load_config
from .core.translator import Translator
from .lock import SingleInstance
from .logging_setup import force_utf8_stdio, setup_logging
from .paths import app_root
from .providers.openai_compat import OpenAICompatProvider
from .storage.db import Database
from .storage.repositories import Repo

log = logging.getLogger(__name__)

COMMANDS = [
    BotCommand(command="start", description="开始使用"),
    BotCommand(command="help", description="用法说明"),
    BotCommand(command="lang", description="切换语言"),
    BotCommand(command="tone", description="默认语气"),
    BotCommand(command="glossary", description="术语表"),
    BotCommand(command="stats", description="今日用量"),
    BotCommand(command="doctor", description="自检"),
    BotCommand(command="about", description="版本信息"),
]


async def run() -> None:
    force_utf8_stdio()

    lock = SingleInstance()
    if not lock.acquire():
        print("已经有一个实例在运行了，本次不重复启动。")
        return

    root = app_root()
    cfg = load_config()
    setup_logging(root / "logs")
    log.info("启动 v%s：模型=%s 代理=%s", APP_VERSION, cfg.default_model.model if cfg.default_model else "无", cfg.proxy or "直连")

    model = cfg.default_model
    if model is None:
        raise ConfigError("没有可用模型，请先在控制台里添加并填写 API Key")

    db = Database(root / "data" / "bot.db")
    db.connect()
    repo = Repo(db)

    try:
        repo.pending_cleanup(7)
        repo.cache_trim(5000)
    except Exception:  # noqa: BLE001
        log.warning("启动清理失败，忽略", exc_info=True)

    provider = OpenAICompatProvider(
        model.base_url,
        model.api_key,
        model.model,
        timeout=60,
        proxy=cfg.proxy or None,
    )
    translator = Translator(
        provider,
        repo,
        model=model.model,
        temperature=0.3,
        max_input_chars=cfg.features.max_input_chars,
        prompt_template=cfg.prompts.translation,
        explain_template=cfg.prompts.explain,
    )

    session = AiohttpSession(proxy=cfg.proxy or None)
    bot = Bot(
        token=cfg.telegram.token,
        session=session,
        default=DefaultBotProperties(parse_mode=ParseMode.HTML),
    )

    dp = build_dispatcher(repo, cfg.telegram.admin_ids)
    dp["config"] = cfg
    dp["repo"] = repo
    dp["translator"] = translator
    dp["provider"] = provider
    dp["context"] = ContextStore(cfg.features.history_size)

    @dp.errors()
    async def on_error(event: ErrorEvent) -> bool:
        log.error("未捕获异常：%s", event.exception, exc_info=event.exception)
        return True

    me = await bot.get_me()
    dp["bot_username"] = me.username or ""
    log.info("已连接 @%s（%s）", me.username, me.id)
    await bot.set_my_commands(COMMANDS)

    try:
        await dp.start_polling(bot, drop_pending_updates=True)
    finally:
        await provider.close()
        await session.close()
        db.close()
        lock.release()
        log.info("已停止")


def main() -> None:
    try:
        asyncio.run(run())
    except ConfigError as exc:
        print(f"配置错误：{exc}")
        raise SystemExit(1)
    except KeyboardInterrupt:
        print("收到中断，已退出")


if __name__ == "__main__":
    main()
