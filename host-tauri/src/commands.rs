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
}

fn err(message: impl Into<String>) -> Value {
    json!({ "ok": false, "message": message.into() })
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> Value {
    let cfg = state.cfg.lock().map(|c| c.clone()).unwrap_or_default();
    json!({
        "version": APP_VERSION,
        "config": cfg,
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
    let cfg = state.cfg.lock().map(|c| c.clone()).unwrap_or_default();
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
    let resp = client
        .post(url)
        .header("X-Console", "1")
        .json(&args)
        .send()
        .await
        .map_err(|e| format!("本地服务连接失败：{e}"))?;
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
