"""会话上下文：最近原文 + 上次使用的外语 + 临时语言覆盖。"""
from __future__ import annotations

from collections import deque


class ContextStore:
    def __init__(self, size: int = 3) -> None:
        self._size = max(0, size)
        self._data: dict[int, deque[str]] = {}
        self._last_foreign: dict[int, str] = {}
        self._override: dict[int, str] = {}

    def add(self, chat_id: int, text: str) -> None:
        if self._size == 0:
            return
        self._data.setdefault(chat_id, deque(maxlen=self._size)).append(text)

    def get(self, chat_id: int) -> list[str]:
        return list(self._data.get(chat_id, []))

    def clear(self, chat_id: int) -> None:
        self._data.pop(chat_id, None)
        self._last_foreign.pop(chat_id, None)
        self._override.pop(chat_id, None)

    def set_last_foreign(self, chat_id: int, code: str) -> None:
        self._last_foreign[chat_id] = code

    def get_last_foreign(self, chat_id: int) -> str | None:
        return self._last_foreign.get(chat_id)

    def set_override(self, chat_id: int, code: str) -> None:
        self._override[chat_id] = code

    def clear_override(self, chat_id: int) -> None:
        self._override.pop(chat_id, None)

    def get_override(self, chat_id: int) -> str | None:
        return self._override.get(chat_id)
