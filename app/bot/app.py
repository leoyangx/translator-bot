"""组装 Dispatcher：注册中间件和路由。"""
from __future__ import annotations

from aiogram import Dispatcher

from ..storage.repositories import Repo
from .handlers import common, translate
from .middlewares import AccessMiddleware, RateLimitMiddleware


def build_dispatcher(repo: Repo, admin_ids: list[int] | None = None) -> Dispatcher:
    dp = Dispatcher()

    access = AccessMiddleware(repo, admin_ids)
    dp.message.outer_middleware(access)
    dp.callback_query.outer_middleware(access)

    dp.message.middleware(RateLimitMiddleware())

    dp.include_router(common.router)
    dp.include_router(translate.router)
    return dp
