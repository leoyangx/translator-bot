//! Tauri 命令：配置、模型、试译（Rust 原生实现）。
use std::sync::Mutex;

use serde_json::{json, Value};
use tauri::State;

use crate::config::{save_config as write_config, Config, APP_VERSION};
use crate::db::Db;
use crate::languages::{language_name, resolve_direction};
use crate::prompts::{DEFAULT_EXPLAIN_PROMPT, DEFAULT_TRANSLATION_PROMPT};
use crate::provider::Provider;
use crate::translator::Translator;

pub struct AppState {
    pub root: std::path::PathBuf,
    pub cfg: Mutex<Config>,
    pub port: Mutex<Option<u16>>,
    pub bot: Mutex<Option<teloxide::dispatching::ShutdownToken>>,
    pub bot_started_at: Mutex<Option<u64>>,
}

fn err(message: impl Into<String>) -> Value {
    json!({ "ok": false, "message": message.into() })
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> Value {
    let cfg = state.cfg.lock().map(|c| c.clone()).unwrap_or_default();
    let languages: Vec<Value> = crate::languages::LANGUAGES
        .iter()
        .map(|l| json!({ "code": l.code, "name": l.zh }))
        .collect();
    let tones: Vec<Value> = crate::prompts::TONE_NAMES
        .iter()
        .map(|(key, name)| json!({ "key": key, "name": name }))
        .collect();
    json!({
        "version": APP_VERSION,
        "config": cfg,
        "languages": languages,
        "tones": tones,
        "defaults": {
            "translation_prompt": DEFAULT_TRANSLATION_PROMPT,
            "explain_prompt": DEFAULT_EXPLAIN_PROMPT,
        }
    })
}

#[tauri::command]
pub fn save_config(state: State<'_, AppState>, payload: Config) -> Value {
    let issues = payload.validate();
    if let Err(exc) = write_config(&state.root, &payload) {
        return err(format!("保存失败：{exc}"));
    }
    if let Ok(mut guard) = state.cfg.lock() {
        *guard = payload;
    }
    json!({ "ok": true, "message": "已保存", "issues": issues })
}

#[tauri::command]
pub fn validate_config(payload: Config) -> Value {
    json!({ "issues": payload.validate() })
}

#[tauri::command]
pub async fn list_models(base_url: String, api_key: String, proxy: String) -> Value {
    if base_url.trim().is_empty() || api_key.trim().is_empty() {
        return err("先填接口地址和 API Key");
    }
    let provider = match Provider::new(&base_url, &api_key, &proxy, 20) {
        Ok(p) => p,
        Err(exc) => return err(format!("初始化失败：{exc}")),
    };
    match provider.models().await {
        Ok(models) => json!({ "ok": true, "models": models }),
        Err(exc) => err(exc.to_string()),
    }
}

#[tauri::command]
pub async fn test_model(
    base_url: String,
    api_key: String,
    model: String,
    proxy: String,
) -> Value {
    if base_url.trim().is_empty() || api_key.trim().is_empty() || model.trim().is_empty() {
        return err("接口地址、API Key、模型名都要填");
    }
    let provider = match Provider::new(&base_url, &api_key, &proxy, 20) {
        Ok(p) => p,
        Err(exc) => return err(format!("初始化失败：{exc}")),
    };
    let started = std::time::Instant::now();
    let models = provider.models().await.unwrap_or_default();
    match provider
        .chat(
            &[crate::prompts::Message {
                role: "user".into(),
                content: "回复一个字：好".into(),
            }],
            &model,
            0.3,
            Some(16),
        )
        .await
    {
        Ok(result) => json!({
            "ok": true,
            "message": format!("连接正常（{} ms）", started.elapsed().as_millis()),
            "latency_ms": started.elapsed().as_millis() as u64,
            "models": models,
            "sample": result.text.chars().take(40).collect::<String>(),
        }),
        Err(exc) => err(exc.to_string()),
    }
}

#[tauri::command]
pub async fn test_translate(
    state: State<'_, AppState>,
    text: String,
    tone: String,
) -> Result<Value, String> {
    let cfg = crate::config::load_config(&state.root).unwrap_or_default();
    let model = match cfg.default_model() {
        Some(m) => m.clone(),
        None => return Err("还没有可用的模型，请先到「配置」页填写 API Key".into()),
    };
    let provider = match Provider::new(&model.base_url, &model.api_key, &cfg.proxy, 45) {
        Ok(p) => p,
        Err(exc) => return Err(format!("初始化失败：{exc}")),
    };
    let db = match Db::open(&state.root.join("data").join("bot.db")) {
        Ok(d) => d,
        Err(exc) => return Err(format!("数据库打开失败：{exc}")),
    };
    let (src, tgt) = resolve_direction(
        &text,
        &cfg.languages.mine,
        &cfg.languages.partners,
    );
    let translator = Translator {
        provider: &provider,
        db: &db,
        cfg: &cfg,
        model: model.model.clone(),
    };
    let started = std::time::Instant::now();
    match translator.translate(&text, &src, &tgt, &tone).await {
        Ok((output, cached)) => Ok(json!({
            "ok": true,
            "translation": output,
            "src_name": language_name(&src),
            "tgt_name": language_name(&tgt),
            "elapsed_ms": started.elapsed().as_millis() as u64,
            "cached": cached,
        })),
        Err(exc) => Err(exc.to_string()),
    }
}

#[tauri::command]
pub fn backend_port(state: State<'_, AppState>) -> Option<u16> {
    state.port.lock().ok().and_then(|p| *p)
}

#[tauri::command]
pub async fn backend_call(
    state: State<'_, AppState>,
    method: String,
    args: Vec<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let port = state
        .port
        .lock()
        .ok()
        .and_then(|p| *p)
        .ok_or_else(|| "后端还没就绪".to_string())?;
    let url = format!("http://127.0.0.1:{port}/api/{method}");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    let request = client
        .post(url)
        .header("X-Console", "1")
        .json(&args)
        .send();
    let resp = match tokio::time::timeout(std::time::Duration::from_secs(75), request).await {
        Ok(Ok(resp)) => resp,
        Ok(Err(e)) => return Err(format!("本地服务连接失败：{e}")),
        Err(_) => return Err("操作超时（75 秒），请检查网络或模型接口".into()),
    };
    if !resp.status().is_success() {
        return Err(format!("本地服务返回 HTTP {}", resp.status()));
    }
    resp.json().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn front_log(state: State<'_, AppState>, message: String) {
    let path = state.root.join("logs").join("host.log");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "[ui] {message}");
    }
}

#[tauri::command]
pub fn start_bot(state: State<'_, AppState>) -> serde_json::Value {
    if state.bot.lock().map(|b| b.is_some()).unwrap_or(false) {
        return serde_json::json!({ "ok": true, "message": "已经在运行" });
    }
    // 以磁盘上的配置为准：界面保存后由后端写盘，内存快照可能是旧的
    let cfg = crate::config::load_config(&state.root).unwrap_or_default();
    if let Ok(mut guard) = state.cfg.lock() {
        *guard = cfg.clone();
    }
    if cfg.telegram.token.trim().is_empty() {
        return serde_json::json!({ "ok": false, "message": "还没填 Telegram Bot Token" });
    }
    let ctx = crate::bot::BotCtx {
        root: state.root.clone(),
        cfg: std::sync::Mutex::new(cfg),
        last_text: std::sync::Mutex::new(std::collections::HashMap::new()),
    };
    match crate::bot::spawn(ctx) {
        Some(token) => {
            if let Ok(mut guard) = state.bot.lock() {
                *guard = Some(token);
            }
            if let Ok(mut started) = state.bot_started_at.lock() {
                *started = Some(crate::bot::started_at_now());
            }
            serde_json::json!({ "ok": true, "message": "已启动" })
        }
        None => serde_json::json!({ "ok": false, "message": "启动失败：Token 无效" }),
    }
}

#[tauri::command]
pub fn stop_bot(state: State<'_, AppState>) -> serde_json::Value {
    let token = state.bot.lock().ok().and_then(|mut b| b.take());
    match token {
        Some(t) => {
            let _ = t.shutdown();
            if let Ok(mut started) = state.bot_started_at.lock() {
                *started = None;
            }
            serde_json::json!({ "ok": true, "message": "已停止" })
        }
        None => serde_json::json!({ "ok": true, "message": "本来就没在运行" }),
    }
}

#[tauri::command]
pub fn restart_bot(state: State<'_, AppState>) -> serde_json::Value {
    if let Some(token) = state.bot.lock().ok().and_then(|mut b| b.take()) {
        let _ = token.shutdown();
    }
    if let Ok(mut started) = state.bot_started_at.lock() {
        *started = None;
    }
    std::thread::sleep(std::time::Duration::from_millis(800));
    start_bot(state)
}

#[tauri::command]
pub fn bot_status(state: State<'_, AppState>) -> serde_json::Value {
    let running = state.bot.lock().map(|b| b.is_some()).unwrap_or(false);
    let started_at = state.bot_started_at.lock().ok().and_then(|s| *s);
    serde_json::json!({
        "running": running,
        "started_at": started_at,
        "pid": if running { std::process::id() } else { 0 },
    })
}

fn now_day() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() + 8 * 3600)
        .unwrap_or(0);
    let days = secs / 86400;
    let mut year = 1970i64;
    let mut rem = days as i64;
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let yd = if leap { 366 } else { 365 };
        if rem < yd { break; }
        rem -= yd;
        year += 1;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let months = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0usize;
    for (i, mm) in months.iter().enumerate() {
        if rem < *mm as i64 { m = i; break; }
        rem -= *mm as i64;
    }
    format!("{}-{:02}-{:02}", year, m + 1, rem + 1)
}

#[tauri::command]
pub fn get_stats(state: State<'_, AppState>) -> Value {
    let db = Db::open(&state.root.join("data").join("bot.db")).ok();
    let (calls, cache, glossary) = match &db {
        Some(db) => (db.usage_today(), db.cache_count(), db.glossary().len()),
        None => (0, 0, 0),
    };
    let _ = now_day();
    json!({ "today_calls": calls, "today_tokens": 0, "cache": cache, "glossary": glossary })
}

#[tauri::command]
pub fn read_logs(state: State<'_, AppState>, limit: usize) -> Vec<String> {
    let path = state.root.join("logs").join("host.log");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    let start = lines.len().saturating_sub(limit.max(1));
    lines[start..].to_vec()
}

#[tauri::command]
pub async fn test_token(token: String) -> Value {
    let token = token.trim().to_string();
    if token.is_empty() {
        return err("先填 Bot Token");
    }
    let url = format!("https://api.telegram.org/bot{token}/getMe");
    let client = match reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)).build() {
        Ok(c) => c,
        Err(e) => return err(e.to_string()),
    };
    match client.get(url).send().await {
        Ok(resp) => {
            let status = resp.status();
            let body: Value = resp.json().await.unwrap_or(Value::Null);
            if status.is_success() && body["ok"].as_bool().unwrap_or(false) {
                let username = body["result"]["username"].as_str().unwrap_or("");
                json!({ "ok": true, "message": format!("已连接 @{username}") })
            } else {
                err("Token 无效或已被撤销")
            }
        }
        Err(e) => err(format!("连接 Telegram 失败：{e}")),
    }
}

#[tauri::command]
pub fn open_folder(state: State<'_, AppState>, kind: String) -> Value {
    let path = match kind.as_str() {
        "root" => state.root.clone(),
        "data" => state.root.join("data"),
        _ => state.root.join("logs"),
    };
    let _ = std::fs::create_dir_all(&path);
    let _ = std::process::Command::new("explorer").arg(&path).spawn();
    json!({ "ok": true, "path": path.to_string_lossy() })
}

#[tauri::command]
pub fn export_diagnostics(state: State<'_, AppState>) -> Value {
    let dir = state.root.join("data").join("diagnostics");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::copy(state.root.join("config.yaml"), dir.join("config.yaml"));
    let _ = std::fs::copy(state.root.join("logs").join("host.log"), dir.join("host.log"));
    let _ = std::process::Command::new("explorer").arg(&dir).spawn();
    json!({ "ok": true, "path": dir.to_string_lossy() })
}

fn autostart_cmd() -> String {
    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run".to_string()
}

#[tauri::command]
pub fn get_autostart() -> Value {
    let output = std::process::Command::new("reg")
        .args(["query", &autostart_cmd(), "/v", "TranslatorBot"])
        .output();
    let enabled = matches!(output, Ok(o) if o.status.success());
    json!({ "enabled": enabled })
}

#[tauri::command]
pub fn set_autostart(enabled: bool) -> Value {
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let status = if enabled {
        std::process::Command::new("reg")
            .args(["add", &autostart_cmd(), "/v", "TranslatorBot", "/t", "REG_SZ", "/d", &exe, "/f"])
            .status()
    } else {
        std::process::Command::new("reg")
            .args(["delete", &autostart_cmd(), "/v", "TranslatorBot", "/f"])
            .status()
    };
    json!({ "ok": matches!(status, Ok(s) if s.success()), "enabled": enabled })
}

#[tauri::command]
pub async fn ui_call(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    method: String,
    args: Vec<Value>,
) -> Result<Value, String> {
    let arg0 = args.first().cloned().unwrap_or(Value::Null);
    let result = match method.as_str() {
        "get_state" => get_state(state),
        "save_config" => {
            let cfg: Config = serde_json::from_value(arg0).map_err(|e| e.to_string())?;
            save_config(state, cfg)
        }
        "validate" => {
            let cfg: Config = serde_json::from_value(arg0).map_err(|e| e.to_string())?;
            validate_config(cfg)
        }
        "list_models" => {
            let base = arg0["base_url"].as_str().unwrap_or("").to_string();
            let key = arg0["api_key"].as_str().unwrap_or("").to_string();
            list_models(base, key, String::new()).await
        }
        "test_model" => {
            let base = arg0["base_url"].as_str().unwrap_or("").to_string();
            let key = arg0["api_key"].as_str().unwrap_or("").to_string();
            let model = arg0["model"].as_str().unwrap_or("").to_string();
            test_model(base, key, model, String::new()).await
        }
        "test_translate" => {
            let text = arg0.as_str().or_else(|| arg0["text"].as_str()).unwrap_or("").to_string();
            let tone = args.get(1).and_then(|v| v.as_str()).unwrap_or("natural").to_string();
            test_translate(state, text, tone).await?
        }
        "get_stats" => get_stats(state),
        "read_logs" => {
            let limit = arg0.as_u64().unwrap_or(200) as usize;
            json!(read_logs(state, limit))
        }
        "test_token" => test_token(arg0.as_str().unwrap_or("").to_string()).await,
        "open_folder" => open_folder(state, arg0.as_str().unwrap_or("logs").to_string()),
        "export_diagnostics" => export_diagnostics(state),
        "get_autostart" => get_autostart(),
        "set_autostart" => set_autostart(arg0.as_bool().unwrap_or(false)),
        "start_bot" => start_bot(state),
        "stop_bot" => stop_bot(state),
        "restart_bot" => restart_bot(state),
        "bot_status" => bot_status(state),
        "quit_app" => {
            app.exit(0);
            json!({ "ok": true })
        }
        other => return Err(format!("未支持的方法：{other}")),
    };
    Ok(result)
}
