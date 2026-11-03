//! Telegram 机器人：teloxide 长轮询，随主进程运行，可随时停止。
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use teloxide::prelude::*;

use crate::config::Config;
use crate::db::Db;
use crate::languages::resolve_direction;
use crate::provider::Provider;
use crate::translator::Translator;

pub struct BotCtx {
    pub root: std::path::PathBuf,
    pub cfg: Config,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

async fn handle_message(bot: Bot, msg: Message, ctx: Arc<BotCtx>) -> ResponseResult<()> {
    let Some(text) = msg.text() else {
        return Ok(());
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }
    if text == "/start" || text == "/help" {
        bot.send_message(
            msg.chat.id,
            "把要翻译的话直接发给我：外语会译成中文，中文会译成对方语言。",
        )
        .await?;
        return Ok(());
    }
    if text.starts_with('/') {
        return Ok(());
    }

    let model = match ctx.cfg.default_model() {
        Some(m) => m.clone(),
        None => {
            bot.send_message(msg.chat.id, "还没有可用的模型，请先在控制台「配置」里填写 API Key")
                .await?;
            return Ok(());
        }
    };

    let provider = match Provider::new(&model.base_url, &model.api_key, &ctx.cfg.proxy, 60) {
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

    let (src, tgt) = resolve_direction(text, &ctx.cfg.languages.mine, &ctx.cfg.languages.partners);
    let translator = Translator {
        provider: &provider,
        db: &db,
        cfg: &ctx.cfg,
        model: model.model.clone(),
    };

    match translator.translate(text, &src, &tgt, "natural").await {
        Ok((output, _cached)) => {
            bot.send_message(msg.chat.id, output).await?;
        }
        Err(e) => {
            bot.send_message(msg.chat.id, format!("翻译失败：{e}")).await?;
        }
    }
    Ok(())
}

/// 启动机器人，返回关闭句柄；调用 `shutdown()` 即优雅停止。
pub fn spawn(ctx: BotCtx) -> Option<teloxide::dispatching::ShutdownToken> {
    let token_str = ctx.cfg.telegram.token.trim().to_string();
    if token_str.is_empty() {
        return None;
    }
    let bot = Bot::new(token_str);
    let shared = Arc::new(ctx);
    let handler = Update::filter_message().endpoint(handle_message);
    let mut dispatcher = Dispatcher::builder(bot, handler)
        .dependencies(dptree::deps![shared])
        .build();
    let shutdown = dispatcher.shutdown_token();
    tauri::async_runtime::spawn(async move {
        dispatcher.dispatch().await;
    });
    Some(shutdown)
}

pub fn started_at_now() -> u64 {
    now()
}
