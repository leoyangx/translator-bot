"""翻译编排：查缓存、拼提示词、调模型、清洗输出。"""
from __future__ import annotations

import asyncio
import hashlib
import logging
import os
import re

from ..providers.base import ChatProvider
from ..providers.openai_compat import ProviderError
from ..storage.repositories import Repo
from .models import Translation
from .prompts import build_explain_messages, build_messages

log = logging.getLogger(__name__)

_MAX_GLOSSARY_ENTRIES = 80

# 多人同时翻译时的并发上限，避免把上游 API 打爆
_CONCURRENCY = max(1, int(os.getenv("TRANSLATORBOT_CONCURRENCY", "8") or 8))
_SEMAPHORE = asyncio.Semaphore(_CONCURRENCY)


class InputTooLong(ValueError):
    pass


def normalize(text: str) -> str:
    text = text.replace("\u00a0", " ").replace("\r\n", "\n").replace("\r", "\n")
    text = re.sub(r"[ \t]+\n", "\n", text)
    text = re.sub(r"\n{3,}", "\n\n", text)
    return text.strip()


_PREFIX_RE = re.compile(r"^(译文|翻译结果|翻译|中文|韩语|韩文|英文|英语|日语)\s*[:：]\s*")
_MD_BOLD_RE = re.compile(r"\*\*(.+?)\*\*|__(.+?)__", re.S)
_QUOTE_PAIRS = (('"', '"'), ("“", "”"), ("'", "'"), ("「", "」"), ("『", "』"))


def strip_markdown(text: str) -> str:
    text = _MD_BOLD_RE.sub(lambda m: m.group(1) or m.group(2), text)
    return text.replace("`", "")


def clean_output(text: str) -> str:
    text = text.strip()
    if text.startswith("```"):
        text = "\n".join(
            line for line in text.splitlines() if not line.strip().startswith("```")
        ).strip()
    text = _PREFIX_RE.sub("", text)
    text = strip_markdown(text)
    text = _PREFIX_RE.sub("", text)
    text = text.strip()
    for left, right in _QUOTE_PAIRS:
        if len(text) > 1 and text.startswith(left) and text.endswith(right):
            text = text[1:-1].strip()
            break
    return text


class Translator:
    def __init__(
        self,
        provider: ChatProvider,
        repo: Repo,
        *,
        model: str,
        temperature: float = 0.3,
        max_input_chars: int = 6000,
        prompt_template: str = "",
        explain_template: str = "",
        keep_punctuation: bool = True,
    ) -> None:
        self._provider = provider
        self._repo = repo
        self._model = model
        self._temperature = temperature
        self._max_input_chars = max_input_chars
        self._prompt_template = prompt_template
        self._explain_template = explain_template
        self._keep_punctuation = keep_punctuation
        self._prompt_hash = hashlib.sha256(
            f"{prompt_template}|{explain_template}".encode("utf-8")
        ).hexdigest()[:10]

    def _cache_key(self, text: str, src: str, tgt: str, tone: str) -> str:
        raw = "|".join(
            [
                "v2",
                self._model,
                tone,
                f"{src}>{tgt}",
                self._prompt_hash,
                self._repo.glossary_signature(),
                text,
            ]
        )
        return hashlib.sha256(raw.encode("utf-8")).hexdigest()

    async def translate(
        self,
        text: str,
        *,
        src: str,
        tgt: str,
        tone: str = "natural",
        context: list[str] | None = None,
        use_cache: bool = True,
    ) -> Translation:
        text = normalize(text)
        if not text:
            raise ValueError("内容为空")

        limit = self._max_input_chars
        if len(text) > limit:
            raise InputTooLong(f"文本太长（{len(text)} 字符），请分段发送，每段不超过 {limit} 字符")

        cache_key = self._cache_key(text, src, tgt, tone)
        if use_cache:
            hit = self._repo.cache_get(cache_key)
            if hit:
                return Translation(
                    source_text=text,
                    target_text=hit,
                    src_lang=src,
                    tgt_lang=tgt,
                    tone=tone,
                    cached=True,
                )

        glossary = self._repo.glossary_list()[:_MAX_GLOSSARY_ENTRIES]
        messages = build_messages(
            text, src, tgt, tone, glossary, context, self._prompt_template
        )
        async with _SEMAPHORE:
            result = await self._provider.chat(
                messages, model=self._model, temperature=self._temperature
            )
        output = clean_output(result.text)
        if not output:
            raise ProviderError("模型返回了空译文，请重试")

        if use_cache:
            self._repo.cache_put(cache_key, text, output)

        return Translation(
            source_text=text,
            target_text=output,
            src_lang=src,
            tgt_lang=tgt,
            tone=tone,
            prompt_tokens=result.prompt_tokens,
            completion_tokens=result.completion_tokens,
        )

    async def explain(self, text: str) -> str:
        async with _SEMAPHORE:
            result = await self._provider.chat(
                build_explain_messages(normalize(text), self._explain_template),
                model=self._model,
                temperature=0.4,
            )
        return clean_output(result.text)
