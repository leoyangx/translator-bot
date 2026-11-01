"""把翻译结果渲染成 Telegram 友好的 HTML。"""
from __future__ import annotations

import html
import re

from ..core.languages import language_name
from ..core.models import Translation
from ..core.prompts import TONE_NAMES

TG_LIMIT = 3900

_MD_BOLD = re.compile(r"\*\*(.+?)\*\*", re.S)


def render_explanation(text: str) -> str:
    escaped = html.escape(text)
    return _MD_BOLD.sub(r"<b>\1</b>", escaped)


def direction_label(t: Translation) -> str:
    src = language_name(t.src_lang)
    tgt = language_name(t.tgt_lang)
    return f"{src} → {tgt}"


def render_translation(
    t: Translation, *, show_source: bool = True, copy_block: bool = False
) -> str:
    head = f"<b>{direction_label(t)}</b> · {TONE_NAMES.get(t.tone, t.tone)}"
    if t.cached:
        head += " · 缓存"

    blocks = [head]
    if show_source:
        blocks.append(f"<blockquote>{html.escape(t.source_text)}</blockquote>")
    target = html.escape(t.target_text)
    if copy_block:
        target = f"<pre>{target}</pre>"
    blocks.append(target)
    return "\n\n".join(blocks)


def split_text(text: str, limit: int = TG_LIMIT) -> list[str]:
    if len(text) <= limit:
        return [text]

    chunks: list[str] = []
    current = ""
    for line in text.split("\n"):
        while len(line) > limit:
            if current:
                chunks.append(current)
                current = ""
            chunks.append(line[:limit])
            line = line[limit:]
        candidate = f"{current}\n{line}" if current else line
        if len(candidate) > limit:
            chunks.append(current)
            current = line
        else:
            current = candidate
    if current:
        chunks.append(current)
    return chunks
