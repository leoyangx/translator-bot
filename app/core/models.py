"""领域模型：翻译请求与结果。"""
from __future__ import annotations

from dataclasses import dataclass


@dataclass(slots=True)
class Translation:
    source_text: str
    target_text: str
    src_lang: str
    tgt_lang: str
    tone: str = "natural"
    cached: bool = False
    prompt_tokens: int = 0
    completion_tokens: int = 0

    @property
    def direction(self) -> str:
        return f"{self.src_lang}->{self.tgt_lang}"
