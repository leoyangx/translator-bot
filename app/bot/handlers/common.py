"""基础命令：/start /help /id /tone /lang /glossary /stats /doctor /about"""
from __future__ import annotations

import asyncio
import logging

from aiogram import Bot, Router
from aiogram.filters import Command, CommandObject, CommandStart
from aiogram.types import Message

from ...config import APP_VERSION, Config
from ...core.languages import LANGUAGES, language_name
from ...core.prompts import TONE_NAMES
from ...providers.openai_compat import OpenAICompatProvider
from ...storage.repositories import Repo
from ..context import ContextStore

router = Router(name="common")
log = logging.getLogger(__name__)

WELCOME = (
    "👋 我是翻译机器人，支持 {count} 种语言。\n\n"
    "怎么用：\n"
    "• 把外语消息【转发】或【粘贴】给我\n"
    "• 群里【回复】某条消息并 @我\n"
    "• 直接说中文，我译成 {partner}（敬语体）\n\n"
    "译文下面可以点：更正式 / 更口语 / 直译 / 解释\n\n"
    "命令：/help 用法 · /lang 切换语言 · /stats 用量 · /doctor 自检"
)

HELP = (
    "<b>用法</b>\n"
    "1. 转发或粘贴外语 → 得到中文译文（原文 + 译文一起给）\n"
    "2. 发中文 → 得到对方语言的译文，可直接复制发给对方\n"
    "3. 点译文下面的按钮换语气，或让它解释语法和俚语\n\n"
    "<b>命令</b>\n"
    "/lang —— 查看语言设置\n"
    "/lang ko —— 本会话锁定韩语（off 取消）\n"
    "/id —— 查看你的 Telegram ID\n"
    "/tone natural|formal|casual|literal —— 改默认语气\n"
    "/glossary —— 术语表（add 김치 = 泡菜 / del 김치）\n"
    "/stats —— 今天的用量\n"
    "/doctor —— 自检（网络、模型、数据库）\n"
    "/about —— 版本信息"
)


@router.message(CommandStart())
async def cmd_start(message: Message, repo: Repo, config: Config) -> None:
    tone = repo.get_kv("default_tone") or "natural"
    partner = language_name(config.languages.partners[0]) if config.languages.partners else "外语"
    await message.answer(
        WELCOME.format(count=len(LANGUAGES), partner=partner)
        + f"\n\n当前语气：{TONE_NAMES.get(tone, tone)}"
    )


@router.message(Command("help"))
async def cmd_help(message: Message) -> None:
    await message.answer(HELP)


@router.message(Command("about"))
async def cmd_about(message: Message) -> None:
    await message.answer(
        f"<b>翻译机器人</b> v{APP_VERSION}\n"
        f"支持语言：{len(LANGUAGES)} 种\n"
        "开源项目 · MIT License"
    )


@router.message(Command("id"))
async def cmd_id(message: Message) -> None:
    await message.answer(
        f"你的 ID：<code>{message.from_user.id}</code>\n"
        f"当前对话 ID：<code>{message.chat.id}</code>"
    )


@router.message(Command("tone"))
async def cmd_tone(message: Message, repo: Repo, command: CommandObject) -> None:
    arg = (command.args or "").strip().lower()
    current = repo.get_kv("default_tone") or "natural"
    if arg not in TONE_NAMES:
        await message.answer(
            "用法：<code>/tone natural|formal|casual|literal</code>\n"
            f"当前默认：{TONE_NAMES.get(current, current)}"
        )
        return
    repo.set_kv("default_tone", arg)
    await message.answer(f"默认语气已改为：{TONE_NAMES[arg]}")


@router.message(Command("lang"))
async def cmd_lang(
    message: Message, config: Config, context: ContextStore, command: CommandObject
) -> None:
    arg = (command.args or "").strip().lower()
    chat_id = message.chat.id

    if not arg:
        partners = "、".join(language_name(code) for code in config.languages.partners)
        override = context.get_override(chat_id)
        lines = [
            f"我的语言：{language_name(config.languages.mine)}",
            f"对方语言：{partners or '未设置'}",
        ]
        if override:
            lines.append(f"本会话已锁定：{language_name(override)}")
        lines.append("")
        lines.append("临时锁定：<code>/lang ko</code>　取消：<code>/lang off</code>")
        lines.append("永久修改请在桌面控制台的「语言」页设置。")
        await message.answer("\n".join(lines))
        return

    if arg in ("off", "auto", "取消"):
        context.clear_override(chat_id)
        await message.answer("已取消语言锁定，恢复自动判断。")
        return

    if arg not in LANGUAGES:
        await message.answer(
            "不支持这个语言代码。可用：" + "、".join(LANGUAGES.keys())
        )
        return

    context.set_override(chat_id, arg)
    await message.answer(f"本会话已锁定对方语言：{language_name(arg)}（<code>/lang off</code> 取消）")


@router.message(Command("glossary"))
async def cmd_glossary(message: Message, repo: Repo, config: Config, command: CommandObject) -> None:
    if not config.features.glossary:
        await message.answer("术语表功能已在控制台关闭。")
        return

    args = (command.args or "").strip()

    if args.startswith("add "):
        body = args[4:]
        if "=" not in body:
            await message.answer("格式：<code>/glossary add 김치 = 泡菜</code>")
            return
        source, target = (part.strip() for part in body.split("=", 1))
        if not source or not target:
            await message.answer("格式：<code>/glossary add 김치 = 泡菜</code>")
            return
        repo.glossary_add(source, target)
        await message.answer(f"已记录：{source} → {target}")
        return

    if args.startswith("del "):
        source = args[4:].strip()
        ok = repo.glossary_delete(source)
        await message.answer("已删除" if ok else "术语表里没有这个词")
        return

    pairs = repo.glossary_list()
    if not pairs:
        await message.answer(
            "术语表是空的。\n加一条：<code>/glossary add 김치 = 泡菜</code>\n"
            "（人名、公司名、游戏术语加进去后，以后永远按这个译）"
        )
        return

    lines = [f"术语表（{len(pairs)} 条）："]
    lines += [f"• {s} → {t}" for s, t in pairs]
    await message.answer("\n".join(lines))


@router.message(Command("stats"))
async def cmd_stats(message: Message, repo: Repo, config: Config) -> None:
    today = repo.usage_today()
    tokens = today["prompt_tokens"] + today["completion_tokens"]
    model = config.default_model
    await message.answer(
        f"<b>今日用量</b>\n"
        f"调用：{today['calls']} 次\n"
        f"Token：{tokens}（输入 {today['prompt_tokens']} / 输出 {today['completion_tokens']}）\n"
        f"缓存条目：{repo.cache_count()}\n"
        f"模型：{model.model if model else '未配置'}"
    )


@router.message(Command("doctor"))
async def cmd_doctor(
    message: Message,
    bot: Bot,
    repo: Repo,
    config: Config,
    provider: OpenAICompatProvider,
) -> None:
    lines = ["🩺 <b>自检</b>"]

    try:
        me = await asyncio.wait_for(bot.get_me(), timeout=15)
        lines.append(f"Telegram：正常（@{me.username}）")
    except Exception as exc:  # noqa: BLE001
        lines.append(f"Telegram：异常 —— {exc}")

    model = config.default_model
    try:
        models = await asyncio.wait_for(provider.models(), timeout=20)
        lines.append(f"模型服务：正常（{model.model if model else '未配置'}）")
        lines.append("可用模型：" + "、".join(models[:8]))
    except Exception as exc:  # noqa: BLE001
        lines.append(f"模型服务：异常 —— {exc}")

    try:
        lines.append(f"数据库：正常（缓存 {repo.cache_count()} 条）")
    except Exception as exc:  # noqa: BLE001
        lines.append(f"数据库：异常 —— {exc}")

    owner = repo.get_owner()
    lines.append(f"主人 ID：{owner if owner else '未设置'}")
    lines.append(f"代理：{config.proxy or '未启用（直连）'}")
    lines.append(f"语言：{'、'.join(language_name(c) for c in config.languages.partners)} → {language_name(config.languages.mine)}")

    await message.answer("\n".join(lines))
