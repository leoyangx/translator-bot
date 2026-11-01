"""中间件：白名单、限流。"""
from __future__ import annotations

import logging
import time

from aiogram import BaseMiddleware
from aiogram.types import CallbackQuery, Message

from ..storage.repositories import Repo

log = logging.getLogger(__name__)


class AccessMiddleware(BaseMiddleware):
    """只允许主人和管理员使用；第一个使用者自动成为主人。"""

    def __init__(self, repo: Repo, admin_ids: list[int] | None = None) -> None:
        self._repo = repo
        self._admin_ids = set(admin_ids or [])

    async def __call__(self, handler, event, data):
        user = data.get("event_from_user")
        if user is None:
            return await handler(event, data)

        owner = self._repo.get_owner()
        if owner is None:
            owner = user.id
            self._repo.set_owner(owner)
            log.info("首个使用者注册为主人：%s (%s)", user.id, user.full_name)

        if user.id == owner or user.id in self._admin_ids:
            return await handler(event, data)

        log.warning("拒绝未授权用户：%s (%s)", user.id, user.full_name)
        text = f"这个机器人没有对你开放。\n你的 Telegram ID：<code>{user.id}</code>"
        if isinstance(event, Message):
            await event.answer(text)
        elif isinstance(event, CallbackQuery):
            await event.answer("未授权", show_alert=True)
        return None


class RateLimitMiddleware(BaseMiddleware):
    """同一用户最小间隔，防止连点或刷屏烧额度。"""

    def __init__(self, interval: float = 1.0) -> None:
        self._interval = interval
        self._last: dict[int, float] = {}

    async def __call__(self, handler, event, data):
        user = data.get("event_from_user")
        if user is not None and isinstance(event, Message):
            now = time.monotonic()
            last = self._last.get(user.id, 0.0)
            if now - last < self._interval:
                await event.answer("慢一点，上一条还在处理～")
                return None
            self._last[user.id] = now
        return await handler(event, data)
