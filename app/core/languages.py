"""多语言支持：语言表、文字系统识别、语向推断。"""
from __future__ import annotations

import unicodedata

# code -> (中文名, 英文名, 文字系统)
LANGUAGES: dict[str, dict[str, str]] = {
    "zh": {"zh": "中文", "en": "Chinese", "script": "han"},
    "ko": {"zh": "韩语", "en": "Korean", "script": "hangul"},
    "ja": {"zh": "日语", "en": "Japanese", "script": "kana"},
    "en": {"zh": "英语", "en": "English", "script": "latin"},
    "th": {"zh": "泰语", "en": "Thai", "script": "thai"},
    "vi": {"zh": "越南语", "en": "Vietnamese", "script": "latin"},
    "id": {"zh": "印尼语", "en": "Indonesian", "script": "latin"},
    "ru": {"zh": "俄语", "en": "Russian", "script": "cyrillic"},
    "es": {"zh": "西班牙语", "en": "Spanish", "script": "latin"},
    "fr": {"zh": "法语", "en": "French", "script": "latin"},
    "de": {"zh": "德语", "en": "German", "script": "latin"},
    "pt": {"zh": "葡萄牙语", "en": "Portuguese", "script": "latin"},
    "ar": {"zh": "阿拉伯语", "en": "Arabic", "script": "arabic"},
    "hi": {"zh": "印地语", "en": "Hindi", "script": "devanagari"},
}

RANGES: dict[str, list[tuple[int, int]]] = {
    "hangul": [(0xAC00, 0xD7A3), (0x1100, 0x11FF), (0x3130, 0x318F)],
    "kana": [(0x3040, 0x309F), (0x30A0, 0x30FF)],
    "han": [(0x4E00, 0x9FFF), (0x3400, 0x4DBF), (0xF900, 0xFAFF)],
    "thai": [(0x0E00, 0x0E7F)],
    "cyrillic": [(0x0400, 0x04FF)],
    "arabic": [(0x0600, 0x06FF), (0x0750, 0x077F)],
    "devanagari": [(0x0900, 0x097F)],
    "latin": [(0x0041, 0x005A), (0x0061, 0x007A), (0x00C0, 0x024F)],
}

_LATIN_CANDIDATES = ["en", "es", "fr", "de", "pt", "id", "vi"]


def _count(text: str, ranges: list[tuple[int, int]]) -> int:
    total = 0
    for ch in text:
        code = ord(ch)
        if any(lo <= code <= hi for lo, hi in ranges):
            total += 1
    return total


def language_name(code: str, ui: str = "zh") -> str:
    meta = LANGUAGES.get(code)
    if not meta:
        return "原文语言（自动判断）" if ui == "zh" else "the source language"
    return meta.get(ui) or meta["en"]


def _dominant_script(text: str) -> str | None:
    counts = {name: _count(text, ranges) for name, ranges in RANGES.items()}
    for name in ("hangul", "kana", "thai", "arabic", "devanagari", "cyrillic"):
        if counts[name] > 0:
            return name
    if counts["han"] > 0 and counts["han"] >= counts["latin"] * 0.3:
        return "han"
    if counts["latin"] > 0:
        return "latin"
    return None


def _detect_latin(text: str) -> str | None:
    try:
        from langdetect import DetectorFactory, detect_langs
    except ImportError:
        return None

    DetectorFactory.seed = 0
    try:
        ranked = detect_langs(text)
    except Exception:  # noqa: BLE001 - 纯符号或无特征文本
        return None
    if not ranked:
        return None
    best = ranked[0]
    code = str(best.lang)
    if code in _LATIN_CANDIDATES and best.prob >= 0.55:
        return code
    return None


def detect_language(text: str) -> str | None:
    """返回语言代码；无法判断时返回 None。"""
    text = unicodedata.normalize("NFC", text).strip()
    if not text:
        return None

    script = _dominant_script(text)
    if script == "hangul":
        return "ko"
    if script == "kana":
        return "ja"
    if script == "thai":
        return "th"
    if script == "arabic":
        return "ar"
    if script == "devanagari":
        return "hi"
    if script == "cyrillic":
        return "ru"
    if script == "han":
        return "zh"
    if script == "latin":
        return _detect_latin(text) or None
    return None


def resolve_direction(
    text: str,
    mine: str,
    partners: list[str],
    last_foreign: str | None = None,
) -> tuple[str, str, str | None]:
    """返回 (src, tgt, detected)。

    规则：
      检测到对方语言 → 译成我的语言
      检测到我的语言 → 译成上一次的对方语言（没有就用列表第一个）
      检测不出来     → 当作外语，译成我的语言
    """
    partners = [p for p in partners if p and p != mine] or ["en"]
    detected = detect_language(text)

    if detected in partners:
        return detected, mine, detected
    if detected == mine:
        target = last_foreign if last_foreign in partners else partners[0]
        return mine, target, detected
    if detected and detected in LANGUAGES:
        return detected, mine, detected
    return "auto", mine, None
