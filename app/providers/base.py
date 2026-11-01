"""模型服务抽象：任何 OpenAI 兼容接口都能接进来。"""
from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol


@dataclass(slots=True)
class ChatResult:
    text: str
    prompt_tokens: int = 0
    completion_tokens: int = 0


class ChatProvider(Protocol):
    async def chat(
        self,
        messages: list[dict],
        *,
        model: str | None = None,
        temperature: float = 0.3,
        max_tokens: int | None = None,
    ) -> ChatResult: ...

    async def close(self) -> None: ...
