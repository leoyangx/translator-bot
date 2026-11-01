"""翻译主流程：多语种、功能开关、群聊策略。"""
from __future__ import annotations

import logging

from aiogram import Bot, F, Router
from aiogram.enums import ChatAction
from aiogram.types import CallbackQuery, Message

from ...config import Config
from ...core.languages import language_name, resolve_direction
from ...core.translator import InputTooLong, Translator
from ...providers.openai_compat import ProviderError
from ...storage.repositories import Repo
from ..context import ContextStore
from ..formatters import render_explanation, render_translation, split_text
from ..keyboards import result_keyboard

router = Router(name="translate")
log = logging.getLogger(__name__)


def _is_group(message: Message) -> bool:
    return message.chat.type in ("group", "supergroup")


def _should_answer(message: Message, bot: Bot, bot_username: str, mode: str) -> bool:
    if not _is_group(message):
        return True
    if mode == "all":
        return True

    text = message.text or message.caption or ""
    mentioned = bool(bot_username) and f"@{bot_username}" in text
    replied = message.reply_to_message
    replied_to_bot = bool(
        replied and replied.from_user and replied.from_user.id == bot.id
    )
    if mode == "mention":
        return mentioned
    return mentioned or replied_to_bot


@router.message(F.text, ~F.text.startswith("/"))
async def on_text(
    message: Message,
    bot: Bot,
    repo: Repo,
    config: Config,
    translator: Translator,
    context: ContextStore,
    bot_username: str,
) -> None:
    if not _should_answer(message, bot, bot_username, config.features.group_mode):
        return

    text = message.text or ""
    if message.reply_to_message and message.reply_to_message.text:
        text = message.reply_to_message.text
    text = text.strip()
    if not text:
        return

    chat_id = message.chat.id
    mine = config.languages.mine
    partners = list(config.languages.partners)
    override = context.get_override(chat_id)
    if override:
        partners = [override]

    if config.features.auto_detect:
        src, tgt, detected = resolve_direction(
            text, mine, partners, context.get_last_foreign(chat_id)
        )
    else:
        src, tgt, detected = partners[0], mine, partners[0]

    status = await message.answer("翻译中…")
    try:
        await bot.send_chat_action(chat_id, ChatAction.TYPING)
    except Exception:  # noqa: BLE001
        pass

    try:
        result = await translator.translate(
            text,
            src=src,
            tgt=tgt,
            tone=repo.get_kv("default_tone") or "natural",
            context=context.get(chat_id) if config.features.context else None,
            use_cache=config.features.cache,
        )
    except InputTooLong as exc:
        await status.edit_text(f"⚠️ {exc}")
        return
    except ProviderError as exc:
        await status.edit_text(f"⚠️ 翻译失败：{exc}")
        return
    except Exception:  # noqa: BLE001
        log.exception("翻译失败")
        await status.edit_text("⚠️ 翻译出错了，请稍后重试（详情见 logs/bot.log）")
        return

    pending_id = repo.pending_add(
        message.from_user.id, result.source_text, result.src_lang, result.tgt_lang, result.tone
    )
    if not result.cached:
        model = config.default_model
        repo.usage_add(
            message.from_user.id,
            model.model if model else "",
            result.prompt_tokens,
            result.completion_tokens,
        )

    if detected and detected != mine:
        context.set_last_foreign(chat_id, detected)
        if config.features.context:
            context.add(chat_id, result.source_text)

    show_explain = (
        config.features.explain_button and result.src_lang != mine and result.src_lang != "auto"
    )
    parts = split_text(
        render_translation(result, show_source=config.features.bilingual)
    )
    keyboard = result_keyboard(
        pending_id, tones=config.features.tone_buttons, explain=show_explain
    )
    await status.edit_text(parts[0], reply_markup=keyboard or None)
    for part in parts[1:]:
        await message.answer(part)


@router.callback_query(F.data.startswith("tone:"))
async def on_tone(
    callback: CallbackQuery,
    repo: Repo,
    config: Config,
    translator: Translator,
) -> None:
    if callback.message is None or callback.data is None or not config.features.tone_buttons:
        return
    try:
        _, tone, raw_id = callback.data.split(":", 2)
        pending_id = int(raw_id)
    except ValueError:
        await callback.answer("按钮数据异常", show_alert=True)
        return

    row = repo.pending_get(pending_id)
    if row is None:
        await callback.answer("这条译文已过期，请重新发送原文", show_alert=True)
        return

    await callback.answer("重新翻译中…")
    try:
        result = await translator.translate(
            row["text"],
            src=row["src"],
            tgt=row["tgt"],
            tone=tone,
            use_cache=config.features.cache,
        )
    except (ProviderError, ValueError) as exc:
        await callback.message.answer(f"⚠️ 重译失败：{exc}")
        return

    new_id = repo.pending_add(
        callback.from_user.id, result.source_text, result.src_lang, result.tgt_lang, result.tone
    )
    show_explain = (
        config.features.explain_button
        and result.src_lang != config.languages.mine
        and result.src_lang != "auto"
    )
    await callback.message.edit_text(
        render_translation(result, show_source=config.features.bilingual),
        reply_markup=result_keyboard(
            new_id, tones=config.features.tone_buttons, explain=show_explain
        )
        or None,
    )


@router.callback_query(F.data.startswith("explain:"))
async def on_explain(
    callback: CallbackQuery,
    repo: Repo,
    config: Config,
    translator: Translator,
) -> None:
    if callback.message is None or callback.data is None or not config.features.explain_button:
        return
    try:
        pending_id = int(callback.data.split(":", 1)[1])
    except ValueError:
        await callback.answer("按钮数据异常", show_alert=True)
        return

    row = repo.pending_get(pending_id)
    if row is None:
        await callback.answer("这条译文已过期，请重新发送原文", show_alert=True)
        return
    if row["src"] == config.languages.mine:
        await callback.answer("这句是你的母语，不需要讲解", show_alert=True)
        return

    await callback.answer("讲解中…")
    try:
        explanation = await translator.explain(row["text"])
    except (ProviderError, ValueError) as exc:
        await callback.message.answer(f"⚠️ 讲解失败：{exc}")
        return

    await callback.message.answer(f"<b>讲解</b>\n\n{render_explanation(explanation)}")


@router.message(F.voice | F.audio | F.photo | F.video | F.document | F.sticker)
async def on_media(message: Message) -> None:
    await message.answer("语音、图片、文件翻译还在开发中，先把文字发我就能翻。")
