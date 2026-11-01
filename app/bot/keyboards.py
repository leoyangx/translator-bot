"""Inline 键盘：译文下方的操作按钮。"""
from __future__ import annotations

from aiogram.types import InlineKeyboardMarkup
from aiogram.utils.keyboard import InlineKeyboardBuilder


def result_keyboard(
    pending_id: int, *, tones: bool = True, explain: bool = True
) -> InlineKeyboardMarkup:
    kb = InlineKeyboardBuilder()
    if tones:
        kb.button(text="更正式", callback_data=f"tone:formal:{pending_id}")
        kb.button(text="更口语", callback_data=f"tone:casual:{pending_id}")
        kb.button(text="直译", callback_data=f"tone:literal:{pending_id}")
    if explain:
        kb.button(text="解释一下", callback_data=f"explain:{pending_id}")

    if not kb.buttons:
        return InlineKeyboardMarkup(inline_keyboard=[])
    if tones and explain:
        kb.adjust(3, 1)
    else:
        kb.adjust(1)
    return kb.as_markup()
