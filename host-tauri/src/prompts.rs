//! 提示词：与 Python 版保持一致的翻译/解释模板。
use crate::languages::language_name;

pub const TONE_STYLES: &[(&str, &str)] = &[
    (
        "natural",
        "自然口语，像母语者日常说话。按原文语境判断礼貌程度，原文客气就客气、原文随意就随意，不要额外添加客套话。",
    ),
    (
        "formal",
        "正式书面或商务表达，句子结构完整。韩语用 합쇼체（-습니다/-ㅂ니다），日语用 です・ます，不要使用网络缩略语或表情符号。",
    ),
    (
        "casual",
        "朋友之间随口聊天。韩语必须用반말（-아/어/-지/-네 等），禁止 -습니다/-ㅂ니다，不要每句都加 -요；日语用タメ口（だ・である），不用です・ます。句子要短，允许口语缩略，不要使用书面连接词。保留原文的 ㅋㅋ/ㅎㅎ/emoji，不要把它们翻译成文字。不要自动补充寒暄语（例如“希望你过得好”）。提到对方的长辈或第三方长辈时：称谓与动词使用敬语形式（어머님、아버님、계시다、드시다、-시-），但句子结尾仍用반말，不要整句改成敬语。",
    ),
    (
        "literal",
        "忠实直译，尽量保留原句结构和语序，不添加解释。",
    ),
];

pub const TONE_NAMES: &[(&str, &str)] = &[
    ("natural", "自然"),
    ("formal", "正式（长辈/客户）"),
    ("casual", "口语（朋友闲聊）"),
    ("literal", "直译"),
];

pub const DEFAULT_TRANSLATION_PROMPT: &str = "你是一名专业译者。本次任务：把{src}翻译成{tgt}。

必须遵守：
1. 只输出译文本体。不要解释、不要加引号、不要写\"译文：\"之类的标签、不要重复原文。
2. 保留原文的换行、emoji、数字、单位、网址、@用户名、话题标签。
3. 人名、公司名、品牌名用通用译法；没有通用译法的保留原文，不要音译生造。
4. 语气：{tone}。
5. 原文里的敬语、谦辞要在译文里体现；原文省略的主语按上下文补全，但不要编造信息。
6. 拿不准的词按最可能的意思直接译，不要输出括号注释或\"可能是指\"。
7. 原文已经是目标语言时，如实返回原文，不要擅自改写。
8. 不要使用任何 Markdown 标记（**、__、#、`），只输出纯文本。{glossary}";

pub const DEFAULT_EXPLAIN_PROMPT: &str = "你是一名语言老师。用户会给你一句外语，请用简体中文解释它：
1. 逐句直译（拆开语序说明）；
2. 标出敬语/敬称等级和说话人关系；
3. 解释俚语、缩写、网络用语；
4. 如果是容易误译的地方，指出为什么。

如果用户给的是中文，不要讲解，只回一句：\"这句是中文，没有需要讲解的语法点。\"

格式要求：用短段落或\"-\"开头的项目符号，总长度控制在 200 字以内。
不要使用 ** 加粗、# 标题、` 代码块等 Markdown 标记，直接输出纯文本。";

pub const PUNCTUATION_RULE: &str =
    "\n9. 严格保留原文标点：原文用空格断句就保留空格；原文没有逗号、句号的地方不要自行添加，也不要删改原有标点。";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

fn glossary_block(items: &[(String, String)]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n术语表（强制遵守，出现即按此翻译）：\n");
    for (src, tgt) in items.iter().take(80) {
        out.push_str(&format!("- {src} = {tgt}\n"));
    }
    out
}

pub fn tone_name(tone: &str) -> &'static str {
    TONE_NAMES
        .iter()
        .find(|(k, _)| *k == tone)
        .map(|(_, v)| *v)
        .unwrap_or("自然")
}

pub fn build_messages(
    text: &str,
    src: &str,
    tgt: &str,
    tone: &str,
    glossary: &[(String, String)],
    context: &[String],
    template: &str,
) -> Vec<Message> {
    let tone_style = TONE_STYLES
        .iter()
        .find(|(k, _)| *k == tone)
        .map(|(_, v)| *v)
        .unwrap_or(TONE_STYLES[0].1);

    let tpl = if template.trim().is_empty() {
        DEFAULT_TRANSLATION_PROMPT
    } else {
        template
    };

    let mut system = tpl
        .replace("{src}", &language_name(src))
        .replace("{tgt}", &language_name(tgt))
        .replace("{tone}", tone_style)
        .replace("{glossary}", &glossary_block(glossary));
    system.push_str(PUNCTUATION_RULE);

    let mut user_parts: Vec<String> = Vec::new();
    if !context.is_empty() {
        let joined = context
            .iter()
            .map(|c| format!("- {c}"))
            .collect::<Vec<_>>()
            .join("\n");
        user_parts.push(format!("【上文，仅供理解指代，不要翻译】\n{joined}"));
    }
    user_parts.push(format!("【待翻译内容】\n{text}"));

    vec![
        Message { role: "system".into(), content: system },
        Message { role: "user".into(), content: user_parts.join("\n\n") },
    ]
}

pub fn build_explain_messages(text: &str, template: &str) -> Vec<Message> {
    let system = if template.trim().is_empty() {
        DEFAULT_EXPLAIN_PROMPT
    } else {
        template
    };
    vec![
        Message { role: "system".into(), content: system.to_string() },
        Message { role: "user".into(), content: text.to_string() },
    ]
}
