//! Telegram 机器人：teloxide 长轮询 + 内联/常驻键盘，随主进程运行。
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};
use teloxide::utils::command::BotCommands;

use crate::config::{save_config, Config};
use crate::db::Db;
use crate::languages::{language_name, resolve_direction, LANGUAGES};
use crate::prompts::tone_name;
use crate::provider::Provider;
use crate::translator::Translator;

pub const CONTACT_URL: &str = "https://t.me/lucky_morrow";

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "可用命令")]
pub enum Cmd {
    #[command(description = "打开主菜单")]
    Start,
    #[command(description = "主菜单")]
    Menu,
}

pub struct BotCtx {
    pub root: std::path::PathBuf,
    pub cfg: Mutex<Config>,
    /// 每个会话最近一次待翻译文本，用于语气按钮重新翻译
    pub last_text: Mutex<HashMap<i64, String>>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn started_at_now() -> u64 {
    now()
}

fn intro_text(cfg: &Config) -> String {
    let partners = cfg
        .languages
        .partners
        .iter()
        .map(|c| language_name(c))
        .collect::<Vec<_>>()
        .join(" / ");
    let mine = language_name(&cfg.languages.mine);
    format!(
"🤖 <b>多语言智能翻译机器人</b>

基于大语言模型驱动，提供高精准度、低延迟的全球多语言互译服务，专为跨国商务交流与社群协作设计。

<b>核心功能</b>

· <b>智能语种感知</b>：自动识别输入语言并完成双向互译，无需频繁手动切换
· <b>多风格语域切换</b>：支持「自然流畅 / 商务正式 / 日常口语 / 精准直译」等多种表达风格
· <b>专业术语表</b>：支持行业行话、品牌专有名词固定译法，保障用词规范与一致性
· <b>上下文语义理解</b>：深度结合前后文连贯语境，准确还原代词与省略句含义
· <b>高速响应缓存</b>：高频与重复内容秒级响应，降低等待时间与资源消耗

<b>使用方式</b>

· <b>私聊交互</b>：直接向机器人发送文本，即时获取翻译结果
· <b>群组协作</b>：在群组内 @机器人 或直接回复机器人的消息触发翻译
· <b>快速配置</b>：点击下方菜单按钮，即可调整目标语言、语气风格、管理术语表及查询用量

当前：{partners} → {mine}，语气 {tone}",
        partners = partners,
        mine = mine,
        tone = tone_name(&cfg.features.default_tone),
    )
}

/// 输入栏下方常驻按钮
fn main_keyboard() -> teloxide::types::KeyboardMarkup {
    use teloxide::types::{KeyboardButton, KeyboardMarkup};
    KeyboardMarkup::new(vec![
        vec![KeyboardButton::new("🌐 语言"), KeyboardButton::new("🎚 语气")],
        vec![KeyboardButton::new("📖 术语表"), KeyboardButton::new("📊 今日用量")],
        vec![KeyboardButton::new("💬 联系"), KeyboardButton::new("❓ 使用说明")],
    ])
    .resize_keyboard()
}

fn main_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🌐 语言", "lang"),
            InlineKeyboardButton::callback("🎚 语气", "tone"),
        ],
        vec![
            InlineKeyboardButton::callback("📖 术语表", "glossary"),
            InlineKeyboardButton::callback("📊 今日用量", "stats"),
        ],
        vec![
            InlineKeyboardButton::url("💬 联系作者", CONTACT_URL.parse().unwrap()),
            InlineKeyboardButton::callback("❓ 使用说明", "help"),
        ],
    ])
}

fn back_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        "⬅️ 返回菜单",
        "menu",
    )]])
}

fn lang_menu() -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = Vec::new();
    for chunk in LANGUAGES.chunks(3) {
        rows.push(
            chunk
                .iter()
                .map(|l| InlineKeyboardButton::callback(l.zh, format!("lang:{}", l.code)))
                .collect(),
        );
    }
    rows.push(vec![InlineKeyboardButton::callback("⬅️ 返回菜单", "menu")]);
    InlineKeyboardMarkup::new(rows)
}

fn tone_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("自然流畅", "tone:natural"),
            InlineKeyboardButton::callback("商务正式", "tone:formal"),
        ],
        vec![
            InlineKeyboardButton::callback("日常口语", "tone:casual"),
            InlineKeyboardButton::callback("精准直译", "tone:literal"),
        ],
        vec![InlineKeyboardButton::callback("⬅️ 返回菜单", "menu")],
    ])
}

fn glossary_text(ctx: &BotCtx) -> String {
    let items = Db::open(&ctx.root.join("data").join("bot.db"))
        .map(|d| d.glossary())
        .unwrap_or_default();
    if items.is_empty() {
        return "<b>📖 专业术语表</b>\n\n还没有条目。\n\n<b>为什么用它</b>\n· 固定品牌名、产品名的译法，避免每次译得不一样\n· 行业术语按你的规范翻译\n\n<b>怎么添加</b>\n发送：<code>/glossary add 原文 = 译文</code>\n例如：<code>/glossary add BIN = BIN 码</code>\n\n添加后所有翻译立即生效。".into();
    }
    let mut text = String::from("<b>📖 专业术语表</b>\n\n");
    for (src, tgt) in items.iter().take(30) {
        text.push_str(&format!("· <code>{src}</code> → {tgt}\n"));
    }
    text.push_str(&format!(
        "\n共 {} 条。添加：<code>/glossary add 原文 = 译文</code>",
        items.len()
    ));
    text
}

fn stats_text(ctx: &BotCtx) -> String {
    let (calls, cache, glossary) = match Db::open(&ctx.root.join("data").join("bot.db")) {
        Ok(d) => (d.usage_today(), d.cache_count(), d.glossary().len()),
        Err(_) => (0, 0, 0),
    };
    format!(
        "<b>📊 今日用量</b>\n\n翻译次数：{calls}\n缓存条目：{cache}\n术语条目：{glossary}\n\n缓存条目越多，重复内容响应越快。"
    )
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn render_result(src: &str, tgt: &str, tone: &str, original: &str, output: &str) -> String {
    format!(
        "<b>{src} → {tgt} · {tone}</b>\n\n<blockquote>{original}</blockquote>\n\n<pre>{output}</pre>",
        src = language_name(src),
        tgt = language_name(tgt),
        tone = tone_name(tone),
        original = html_escape(original),
        output = html_escape(output),
    )
}

fn result_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("更正式", "tone:formal"),
            InlineKeyboardButton::callback("更口语", "tone:casual"),
        ],
        vec![InlineKeyboardButton::callback("直译", "tone:literal")],
    ])
}

async fn do_translate(
    text: &str,
    tone: &str,
    ctx: &BotCtx,
) -> Result<Option<(String, String, String)>, String> {
    let cfg = ctx.cfg.lock().map(|c| c.clone()).unwrap_or_default();
    let model = cfg
        .default_model()
        .cloned()
        .ok_or_else(|| "还没有可用的模型，请先在控制台「配置」里填写 API Key".to_string())?;
    let provider = Provider::new(&model.base_url, &model.api_key, &cfg.proxy, 60)
        .map_err(|e| format!("模型初始化失败：{e}"))?;
    let db = Db::open(&ctx.root.join("data").join("bot.db"))
        .map_err(|e| format!("数据库打开失败：{e}"))?;
    let (src, tgt) = resolve_direction(text, &cfg.languages.mine, &cfg.languages.partners);
    let translator = Translator {
        provider: &provider,
        db: &db,
        cfg: &cfg,
        model: model.model.clone(),
    };
    match translator.translate(text, &src, &tgt, tone).await {
        Ok((output, _cached)) => {
            db.bump_usage();
            Ok(Some((src, tgt, output)))
        }
        Err(e) => Err(e.to_string()),
    }
}

async fn handle_command(bot: Bot, msg: Message, cmd: Cmd, ctx: Arc<BotCtx>) -> ResponseResult<()> {
    let cfg = ctx.cfg.lock().map(|c| c.clone()).unwrap_or_default();
    let _ = cmd;
    bot.send_message(msg.chat.id, intro_text(&cfg))
        .parse_mode(ParseMode::Html)
        .reply_markup(main_menu())
        .await?;
    let _ = ctx;
    Ok(())
}

async fn handle_glossary_add(
    bot: Bot,
    msg: Message,
    body: &str,
    ctx: Arc<BotCtx>,
) -> ResponseResult<()> {
    let rest = body.trim().strip_prefix("add").unwrap_or("").trim();
    let parts: Vec<&str> = rest
        .split('=')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() != 2 {
        bot.send_message(msg.chat.id, "格式：/glossary add 原文 = 译文").await?;
        return Ok(());
    }
    if let Ok(db) = Db::open(&ctx.root.join("data").join("bot.db")) {
        db.glossary_put(parts[0], parts[1]);
        bot.send_message(msg.chat.id, format!("已添加：{} → {}", parts[0], parts[1]))
            .reply_markup(back_menu())
            .await?;
    }
    Ok(())
}

async fn handle_message(bot: Bot, msg: Message, ctx: Arc<BotCtx>) -> ResponseResult<()> {
    let Some(text) = msg.text() else {
        return Ok(());
    };
    let text = text.trim().to_string();
    if text.is_empty() {
        return Ok(());
    }
    let cfg = ctx.cfg.lock().map(|c| c.clone()).unwrap_or_default();

    match text.as_str() {
        "🌐 语言" => {
            bot.send_message(msg.chat.id, "选择你的语言（对方语言会跟着换）")
                .reply_markup(lang_menu())
                .await?;
            return Ok(());
        }
        "🎚 语气" => {
            bot.send_message(msg.chat.id, "选择默认翻译语气")
                .reply_markup(tone_menu())
                .await?;
            return Ok(());
        }
        "📖 术语表" => {
            bot.send_message(msg.chat.id, glossary_text(&ctx))
                .parse_mode(ParseMode::Html)
                .reply_markup(back_menu())
                .await?;
            return Ok(());
        }
        "📊 今日用量" => {
            bot.send_message(msg.chat.id, stats_text(&ctx))
                .parse_mode(ParseMode::Html)
                .reply_markup(back_menu())
                .await?;
            return Ok(());
        }
        "💬 联系" => {
            bot.send_message(msg.chat.id, format!("作者联系方式：{CONTACT_URL}"))
                .reply_markup(back_menu())
                .await?;
            return Ok(());
        }
        "❓ 使用说明" => {
            bot.send_message(msg.chat.id, intro_text(&cfg))
                .parse_mode(ParseMode::Html)
                .reply_markup(main_menu())
                .await?;
            return Ok(());
        }
        _ => {}
    }

    if text.starts_with("/glossary") {
        let body = text.trim_start_matches("/glossary").trim();
        if body.starts_with("add") {
            return handle_glossary_add(bot, msg, body, ctx).await;
        }
        bot.send_message(msg.chat.id, glossary_text(&ctx))
            .parse_mode(ParseMode::Html)
            .reply_markup(back_menu())
            .await?;
        return Ok(());
    }
    if text.starts_with('/') {
        return Ok(());
    }

    if let Ok(mut map) = ctx.last_text.lock() {
        map.insert(msg.chat.id.0, text.clone());
    }

    let placeholder = bot.send_message(msg.chat.id, "翻译中…").await?;
    match do_translate(&text, &cfg.features.default_tone, &ctx).await {
        Ok(Some((src, tgt, output))) => {
            let body = render_result(&src, &tgt, &cfg.features.default_tone, &text, &output);
            let _ = bot
                .edit_message_text(msg.chat.id, placeholder.id, body)
                .parse_mode(ParseMode::Html)
                .reply_markup(result_menu())
                .await;
        }
        Ok(None) => {}
        Err(e) => {
            let _ = bot
                .edit_message_text(msg.chat.id, placeholder.id, format!("翻译失败：{e}"))
                .await;
        }
    }
    Ok(())
}

async fn handle_callback(bot: Bot, q: CallbackQuery, ctx: Arc<BotCtx>) -> ResponseResult<()> {
    let data = q.data.unwrap_or_default();
    let cfg = ctx.cfg.lock().map(|c| c.clone()).unwrap_or_default();
    let _ = bot.answer_callback_query(q.id.clone()).await;

    if let Some(tone) = data.strip_prefix("tone:") {
        if let Some(message) = q.message.clone() {
            let chat_id = message.chat().id;
            let last = ctx
                .last_text
                .lock()
                .ok()
                .and_then(|m| m.get(&chat_id.0).cloned());
            if let Some(text) = last {
                if let Ok(Some((src, tgt, output))) = do_translate(&text, tone, &ctx).await {
                    let body = render_result(&src, &tgt, tone, &text, &output);
                    let _ = bot
                        .edit_message_text(chat_id, message.id(), body)
                        .parse_mode(ParseMode::Html)
                        .reply_markup(result_menu())
                        .await;
                }
            }
        }
        return Ok(());
    }

    let (text, markup) = match data.as_str() {
        "menu" | "help" => (intro_text(&cfg), main_menu()),
        "lang" => (
            "选择你的语言（对方语言会跟着换）".to_string(),
            lang_menu(),
        ),
        "tone" => ("选择默认翻译语气".to_string(), tone_menu()),
        "glossary" => (glossary_text(&ctx), back_menu()),
        "stats" => (stats_text(&ctx), back_menu()),
        other if other.starts_with("lang:") => {
            let code = other.trim_start_matches("lang:");
            let mut cfg2 = cfg.clone();
            cfg2.languages.mine = code.to_string();
            let partners: Vec<String> = LANGUAGES
                .iter()
                .filter(|l| l.code != code)
                .take(3)
                .map(|l| l.code.to_string())
                .collect();
            cfg2.languages.partners = partners;
            if let Ok(mut guard) = ctx.cfg.lock() {
                *guard = cfg2.clone();
            }
            let _ = save_config(&ctx.root, &cfg2);
            (
                format!(
                    "已切换：我的语言 = {}；对方语言 = {}",
                    language_name(code),
                    cfg2.languages
                        .partners
                        .iter()
                        .map(|c| language_name(c))
                        .collect::<Vec<_>>()
                        .join(" / ")
                ),
                back_menu(),
            )
        }
        _ => (intro_text(&cfg), main_menu()),
    };

    if let Some(message) = q.message {
        let _ = bot
            .edit_message_text(message.chat().id, message.id(), text)
            .parse_mode(ParseMode::Html)
            .reply_markup(markup)
            .await;
    }
    Ok(())
}

pub fn spawn(ctx: BotCtx) -> Option<teloxide::dispatching::ShutdownToken> {
    let token_str = ctx
        .cfg
        .lock()
        .map(|c| c.telegram.token.trim().to_string())
        .unwrap_or_default();
    if token_str.is_empty() {
        return None;
    }
    let bot = Bot::new(token_str);
    let shared = Arc::new(ctx);

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Cmd>()
                .endpoint(handle_command),
        )
        .branch(Update::filter_message().endpoint(handle_message))
        .branch(Update::filter_callback_query().endpoint(handle_callback));

    let mut dispatcher = Dispatcher::builder(bot.clone(), handler)
        .dependencies(dptree::deps![shared])
        .build();
    let shutdown = dispatcher.shutdown_token();

    tauri::async_runtime::spawn(async move {
        let _ = bot.set_my_commands(Cmd::bot_commands()).await;
        dispatcher.dispatch().await;
    });
    Some(shutdown)
}
