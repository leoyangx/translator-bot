"""提示词：模板可被用户在页面里覆盖。"""
from __future__ import annotations

from .languages import language_name

TONE_STYLES: dict[str, str] = {
    "natural": "自然口语，像母语者日常说话",
    "formal": "正式书面语，适合商务、邮件、公告",
    "casual": "轻松随意，像朋友间聊天",
    "literal": "忠实直译，尽量保留原句结构和语序",
}

TONE_NAMES: dict[str, str] = {
    "natural": "自然",
    "formal": "正式",
    "casual": "口语",
    "literal": "直译",
}

DEFAULT_TRANSLATION_PROMPT = """你是一名专业译者。本次任务：把{src}翻译成{tgt}。

必须遵守：
1. 只输出译文本体。不要解释、不要加引号、不要写"译文："之类的标签、不要重复原文。
2. 保留原文的换行、emoji、数字、单位、网址、@用户名、话题标签。
3. 人名、公司名、品牌名用通用译法；没有通用译法的保留原文，不要音译生造。
4. 语气：{tone}。
5. 原文里的敬语、谦辞要在译文里体现；原文省略的主语按上下文补全，但不要编造信息。
6. 拿不准的词按最可能的意思直接译，不要输出括号注释或"可能是指"。
7. 原文已经是目标语言时，如实返回原文，不要擅自改写。
8. 不要使用任何 Markdown 标记（**、__、#、`），只输出纯文本。{glossary}"""

DEFAULT_GLOSSARY_BLOCK = """
术语表（强制遵守，出现即按此翻译）：
{items}"""

DEFAULT_EXPLAIN_PROMPT = """你是一名语言老师。用户会给你一句外语，请用简体中文解释它：
1. 逐句直译（拆开语序说明）；
2. 标出敬语/敬称等级和说话人关系；
3. 解释俚语、缩写、网络用语；
4. 如果是容易误译的地方，指出为什么。

如果用户给的是中文，不要讲解，只回一句："这句是中文，没有需要讲解的语法点。"

格式要求：用短段落或"-"开头的项目符号，总长度控制在 200 字以内。
不要使用 ** 加粗、# 标题、` 代码块等 Markdown 标记，直接输出纯文本。"""


def _glossary_block(glossary: list[tuple[str, str]] | None) -> str:
    if not glossary:
        return ""
    items = "\n".join(f"- {s} = {t}" for s, t in glossary)
    return DEFAULT_GLOSSARY_BLOCK.format(items=items)


def build_messages(
    text: str,
    src: str,
    tgt: str,
    tone: str = "natural",
    glossary: list[tuple[str, str]] | None = None,
    context: list[str] | None = None,
    template: str | None = None,
) -> list[dict]:
    template = (template or "").strip() or DEFAULT_TRANSLATION_PROMPT
    try:
        system = template.format(
            src=language_name(src),
            tgt=language_name(tgt),
            tone=TONE_STYLES.get(tone, TONE_STYLES["natural"]),
            glossary=_glossary_block(glossary),
        )
    except (KeyError, IndexError, ValueError):
        # 用户模板写坏了（比如多了个 {），退回默认模板
        system = DEFAULT_TRANSLATION_PROMPT.format(
            src=language_name(src),
            tgt=language_name(tgt),
            tone=TONE_STYLES.get(tone, TONE_STYLES["natural"]),
            glossary=_glossary_block(glossary),
        )

    user_parts: list[str] = []
    if context:
        joined = "\n".join(f"- {c}" for c in context)
        user_parts.append(f"【上文，仅供理解指代，不要翻译】\n{joined}")
    user_parts.append(f"【待翻译内容】\n{text}")

    return [
        {"role": "system", "content": system},
        {"role": "user", "content": "\n\n".join(user_parts)},
    ]


def build_explain_messages(text: str, template: str | None = None) -> list[dict]:
    system = (template or "").strip() or DEFAULT_EXPLAIN_PROMPT
    return [
        {"role": "system", "content": system},
        {"role": "user", "content": text},
    ]
