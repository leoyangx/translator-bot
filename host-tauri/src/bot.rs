//! Telegram 机器人：teloxide 长轮询 + 内联键盘菜单，随主进程运行。
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

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase", description = "可用命令")]
pub enum Cmd {
    #[command(description = "打开主菜单")]
    Start,
    #[command(description = "主菜单")]
    Menu,
    #[command(description = "切换语言")]
    Lang,
    #[command(description = "切换语气")]
    Tone,
    #[command(description = "术语表")]
    Glossary,
    #[command(description = "今日用量")]
    Stats,
}

pub struct BotCtx {
    pub root: std::path::PathBuf,
    pub cfg: Mutex<Config>,
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
    format!(
        "<b>翻译机器人</b>\n\n把要翻译的话直接发给我：\n· {partners} → {mine}\n· {mine} → {partner}\n\n当前语气：{tone}\n直接发消息即可翻译。",
        partners = partners,
        mine = language_name(&cfg.languages.mine),
        partner = language_name(cfg.languages.partners.first().map(|s| s.as_str()).unwrap_or("en")),
        tone = tone_name(&cfg.features.default_tone),
    )
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
        vec![InlineKeyboardButton::callback("❓ 使用说明", "help")],
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
            InlineKeyboardButton::callback("自然", "tone:natural"),
            InlineKeyboardButton::callback("正式", "tone:formal"),
        ],
        vec![
            InlineKeyboardButton::callback("口语", "tone:casual"),
            InlineKeyboardButton::callback("直译", "tone:literal"),
        ],
        vec![InlineKeyboardButton::callback("⬅️ 返回菜单", "menu")],
    ])
}

async fn handle_command(bot: Bot, msg: Message, cmd: Cmd, ctx: Arc<BotCtx>) -> ResponseResult<()> {
    let cfg = ctx.cfg.lock().map(|c| c.clone()).unwrap_or_default();
    match cmd {
        Cmd::Start | Cmd::Menu => {
            bot.send_message(msg.chat.id, intro_text(&cfg))
                .parse_mode(ParseMode::Html)
                .reply_markup(main_menu())
                .await?;
        }
        Cmd::Lang => {
            bot.send_message(msg.chat.id, "选择你的语言（对方语言会跟着换）")
                .reply_markup(lang_menu())
                .await?;
        }
        Cmd::Tone => {
            bot.send_message(msg.chat.id, "选择默认翻译语气")
                .reply_markup(tone_menu())
                .await?;
        }
        Cmd::Glossary => {
            bot.send_message(msg.chat.id, glossary_text(&ctx))
                .parse_mode(ParseMode::Html)
                .reply_markup(back_menu())
                .await?;
        }
        Cmd::Stats => {
            bot.send_message(msg.chat.id, stats_text(&ctx))
                .parse_mode(ParseMode::Html)
                .reply_markup(back_menu())
                .await?;
        }
    }
    Ok(())
}

fn glossary_text(ctx: &BotCtx) -> String {
    let db = match Db::open(&ctx.root.join("data").join("bot.db")) {
        Ok(d) => d,
        Err(_) => return "读取术语表失败".into(),
    };
    let items = db.glossary();
    if items.is_empty() {
        return "<b>术语表</b>\n\n还没有条目。\n添加方式：<code>/glossary add 原文 = 译文</code>".into();
    }
    let mut text = String::from("<b>术语表</b>\n\n");
    for (src, tgt) in items.iter().take(30) {
        text.push_str(&format!("· {src} = {tgt}\n"));
    }
    text.push_str("\n添加：<code>/glossary add 原文 = 译文</code>");
    text
}

fn stats_text(ctx: &BotCtx) -> String {
    let db = match Db::open(&ctx.root.join("data").join("bot.db")) {
        Ok(d) => d,
        Err(_) => return "读取统计失败".into(),
    };
    format!(
        "<b>今日用量</b>\n\n翻译次数：{count}\n缓存条目：{cache}\n术语条目：{glossary}",
        count = db.usage_today(),
        cache = db.cache_count(),
        glossary = db.glossary().len(),
    )
}

async fn handle_glossary_add(bot: Bot, msg: Message, body: &str, ctx: Arc<BotCtx>) -> ResponseResult<()> {
    let rest = body.trim().strip_prefix("add").unwrap_or("").trim();
    let parts: Vec<&str> = rest.split('=').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if parts.len() != 2 {
        bot.send_message(msg.chat.id, "格式：/glossary add 原文 = 译文").await?;
        return Ok(());
    }
    let db = Db::open(&ctx.root.join("data").join("bot.db")).ok();
    if let Some(db) = db {
        db.glossary_put(parts[0], parts[1]);
        bot.send_message(msg.chat.id, format!("已添加：{} = {}", parts[0], parts[1]))
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

    let cfg = ctx.cfg.lock().map(|c| c.clone()).unwrap_or_default();
    let model = match cfg.default_model() {
        Some(m) => m.clone(),
        None => {
            bot.send_message(msg.chat.id, "还没有可用的模型，请先在控制台「配置」里填写 API Key")
                .await?;
            return Ok(());
        }
    };

    let provider = match Provider::new(&model.base_url, &model.api_key, &cfg.proxy, 60) {
        Ok(p) => p,
        Err(e) => {
            bot.send_message(msg.chat.id, format!("模型初始化失败：{e}")).await?;
            return Ok(());
        }
    };
    let db = match Db::open(&ctx.root.join("data").join("bot.db")) {
        Ok(d) => d,
        Err(e) => {
            bot.send_message(msg.chat.id, format!("数据库打开失败：{e}")).await?;
            return Ok(());
        }
    };

    let (src, tgt) = resolve_direction(&text, &cfg.languages.mine, &cfg.languages.partners);
    let translator = Translator {
        provider: &provider,
        db: &db,
        cfg: &cfg,
        model: model.model.clone(),
    };

    match translator
        .translate(&text, &src, &tgt, &cfg.features.default_tone)
        .await
    {
        Ok((output, _cached)) => {
            db.bump_usage();
            bot.send_message(msg.chat.id, output).await?;
        }
        Err(e) => {
            bot.send_message(msg.chat.id, format!("翻译失败：{e}")).await?;
        }
    }
    Ok(())
}

async fn handle_callback(bot: Bot, q: CallbackQuery, ctx: Arc<BotCtx>) -> ResponseResult<()> {
    let data = q.data.unwrap_or_default();
    let cfg = ctx.cfg.lock().map(|c| c.clone()).unwrap_or_default();

    let (text, markup) = match data.as_str() {
        "menu" | "help" => (intro_text(&cfg), main_menu()),
        "lang" => ("选择你的语言（对方语言会跟着换）".to_string(), lang_menu()),
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
                format!("已切换：我的语言 = {}；对方语言 = {}", language_name(code), cfg2
                    .languages
                    .partners
                    .iter()
                    .map(|c| language_name(c))
                    .collect::<Vec<_>>()
                    .join(" / ")),
                back_menu(),
            )
        }
        other if other.starts_with("tone:") => {
            let tone = other.trim_start_matches("tone:");
            let mut cfg2 = cfg.clone();
            cfg2.features.default_tone = tone.to_string();
            if let Ok(mut guard) = ctx.cfg.lock() {
                *guard = cfg2.clone();
            }
            let _ = save_config(&ctx.root, &cfg2);
            (format!("已切换默认语气：{}", tone_name(tone)), back_menu())
        }
        _ => (intro_text(&cfg), main_menu()),
    };

    let _ = bot.answer_callback_query(q.id).await;
    if let Some(message) = q.message {
        let _ = bot
            .edit_message_text(message.chat().id, message.id(), text)
            .parse_mode(ParseMode::Html)
            .reply_markup(markup)
            .await;
    }
    Ok(())
}

/// 启动机器人，返回关闭句柄。
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
        .branch(Update::filter_message().filter_command::<Cmd>().endpoint(handle_command))
        .branch(Update::filter_message().endpoint(handle_message))
        .branch(Update::filter_callback_query().endpoint(handle_callback));

    let mut dispatcher = Dispatcher::builder(bot.clone(), handler)
        .dependencies(dptree::deps![shared])
        .build();
    let shutdown = dispatcher.shutdown_token();

    tauri::async_runtime::spawn(async move {
        let _ = bot
            .set_my_commands(Cmd::bot_commands())
            .await;
        dispatcher.dispatch().await;
    });
    Some(shutdown)
}
