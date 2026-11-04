// 翻译机器人 · Tauri 宿主
// 单实例 + 内嵌后端解包 + 系统 WebView2 窗口 + 托盘 + 自动更新（多线路）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bot;
mod commands;
mod config;
mod db;
mod languages;
mod prompts;
mod provider;
mod translator;


use std::{
    env, fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::Mutex,
    thread,
    time::Duration,
};

use serde::Serialize;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_updater::UpdaterExt;

const ICON_PNG: &[u8] = include_bytes!("../../assets/icon-512.png");
const TITLE: &str = "翻译机器人";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const GH_BASE: &str = "https://github.com/leoyangx/translator-bot/releases/latest/download/";
const MIRRORS: [&str; 2] = [
    "https://ghproxy.net/https://github.com/leoyangx/translator-bot/releases/latest/download/",
    "https://gh-proxy.com/https://github.com/leoyangx/translator-bot/releases/latest/download/",
];

struct Backend(Mutex<Option<Child>>);

#[derive(Serialize)]
struct UpdateInfo {
    available: bool,
    version: Option<String>,
    notes: Option<String>,
    error: Option<String>,
}

fn exe_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 数据目录：安装版放 %LOCALAPPDATA%\TranslatorBot，绿色版放 exe 同目录
fn data_home() -> PathBuf {
    if let Ok(local) = env::var("LOCALAPPDATA") {
        PathBuf::from(local).join("TranslatorBot")
    } else {
        exe_dir()
    }
}

fn log_line(home: &Path, msg: &str) {
    use std::io::Write;
    let dir = home.join("logs");
    let _ = fs::create_dir_all(&dir);
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("host.log"))
    {
        let _ = writeln!(f, "{}", msg);
    }
}

/// 首次运行（或版本变化）时把内嵌后端解包到数据目录
fn start_backend(home: &Path, backend: &Path) -> Option<Child> {
    log_line(home, &format!("backend: {}", backend.display()));
    Command::new(backend)
        .current_dir(home)
        .env("TRANSLATORBOT_HOME", home)
        .spawn()
        .ok()
}

fn wait_for_port(home: &Path, timeout: Duration) -> Option<String> {
    let port_file = home.join("data").join("ui.port");
    let _ = fs::remove_file(&port_file);
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Ok(text) = fs::read_to_string(&port_file) {
            let t = text.trim();
            if !t.is_empty() {
                return Some(format!("http://127.0.0.1:{}/", t));
            }
        }
        thread::sleep(Duration::from_millis(150));
    }
    None
}

/// 读取 config.yaml 的更新设置：mode = auto | direct | custom
fn update_settings(home: &Path) -> (String, String) {
    let path = home.join("config.yaml");
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return ("auto".into(), String::new()),
    };
    let value: serde_yaml::Value = match serde_yaml::from_str(&text) {
        Ok(v) => v,
        Err(_) => return ("auto".into(), String::new()),
    };
    let section = value.get("update");
    let mode = section
        .and_then(|u| u.get("mode"))
        .and_then(|v| v.as_str())
        .unwrap_or("auto")
        .to_string();
    let proxy = section
        .and_then(|u| u.get("custom_proxy"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    (mode, proxy)
}

async fn reachable(base: &str) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    let url = format!("{}latest.json", base);
    matches!(client.get(url).send().await, Ok(r) if r.status().is_success())
}

/// 按 自定义 → 官方直连 → 公共镜像 的顺序找一条能通的线路
async fn resolve_base(home: &Path) -> Option<String> {
    let (mode, proxy) = update_settings(home);
    let mut candidates: Vec<String> = Vec::new();
    if mode == "custom" && !proxy.is_empty() {
        candidates.push(if proxy.ends_with('/') {
            proxy.clone()
        } else {
            format!("{}/", proxy)
        });
    }
    candidates.push(GH_BASE.to_string());
    if mode != "direct" {
        for m in MIRRORS {
            candidates.push(m.to_string());
        }
    }
    for base in candidates {
        if reachable(&base).await {
            return Some(base);
        }
    }
    None
}

async fn updater_for(app: &AppHandle, base: &str) -> Result<tauri_plugin_updater::Updater, String> {
    let url = format!("{}latest.json", base)
        .parse()
        .map_err(|_| "更新地址无效".to_string())?;
    app.updater_builder()
        .endpoints(vec![url])
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_check(app: AppHandle) -> Result<UpdateInfo, String> {
    let home = data_home();
    let base = match resolve_base(&home).await {
        Some(b) => b,
        None => {
            return Ok(UpdateInfo {
                available: false,
                version: None,
                notes: None,
                error: Some("所有更新线路都连不上".into()),
            })
        }
    };
    let updater = updater_for(&app, &base).await?;
    let checked = updater.check().await;
    if let Err(e) = &checked {
        log_line(&home, &format!("update_check 失败: {e}"));
    }
    match checked {
        Ok(Some(update)) => {
            let current = app.package_info().version.to_string();
            let newer = match (
                semver::Version::parse(&update.version),
                semver::Version::parse(&current),
            ) {
                (Ok(remote), Ok(cur)) => remote > cur,
                _ => true,
            };
            if !newer {
                return Ok(UpdateInfo {
                    available: false,
                    version: None,
                    notes: None,
                    error: None,
                });
            }
            Ok(UpdateInfo {
                available: true,
                version: Some(update.version.clone()),
                notes: update.body.clone(),
                error: None,
            })
        }
        Ok(None) => Ok(UpdateInfo {
            available: false,
            version: None,
            notes: None,
            error: None,
        }),
        Err(e) => Ok(UpdateInfo {
            available: false,
            version: None,
            notes: None,
            error: Some(e.to_string()),
        }),
    }
}

#[tauri::command]
async fn update_install(app: AppHandle) -> Result<(), String> {
    let home = data_home();
    let base = resolve_base(&home).await.ok_or("连不上更新服务器")?;
    let updater = updater_for(&app, &base).await?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("已经是最新版本")?;
    {
        let current = app.package_info().version.to_string();
        if let (Ok(remote), Ok(cur)) = (
            semver::Version::parse(&update.version),
            semver::Version::parse(&current),
        ) {
            if remote <= cur {
                return Err("已经是最新版本".into());
            }
        }
    }
    let handle = app.clone();
    let result = update
        .download_and_install(
            move |chunk, total| {
                let _ = handle.emit(
                    "update-progress",
                    serde_json::json!({ "chunk": chunk, "total": total }),
                );
            },
            || {},
        )
        .await;
    if let Err(e) = result {
        log_line(&home, &format!("update_install 失败: {e}"));
        return Err(e.to_string());
    }
    app.restart()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            update_check,
            update_install,
            commands::get_state,
            commands::save_config,
            commands::validate_config,
            commands::list_models,
            commands::test_model,
            commands::test_translate,
            commands::backend_port,
            commands::backend_call,
            commands::front_log,
            commands::start_bot,
            commands::stop_bot,
            commands::restart_bot,
            commands::bot_status,
            commands::get_stats,
            commands::read_logs,
            commands::test_token,
            commands::open_folder,
            commands::export_diagnostics,
            commands::get_autostart,
            commands::set_autostart,
            commands::ui_call
        ])
        .setup(|app| {
            let exe = exe_dir();
            let home = if exe.join("backend").join("TranslatorBotBackend.exe").exists() {
                exe.clone()
            } else {
                data_home()
            };
            log_line(&home, &format!("Tauri 宿主启动 v{}", VERSION));

            let cfg = crate::config::load_config(&home).unwrap_or_default();
            let autostart = cfg.app.autostart_bot && !cfg.telegram.token.trim().is_empty();
            app.manage(commands::AppState {
                root: home.clone(),
                cfg: std::sync::Mutex::new(cfg.clone()),
                port: std::sync::Mutex::new(None),
                bot: std::sync::Mutex::new(None),
                bot_started_at: std::sync::Mutex::new(None),
            });
            if autostart {
                if let Some(token) = crate::bot::spawn(crate::bot::BotCtx {
                    root: home.clone(),
                    cfg: std::sync::Mutex::new(cfg.clone()),
                    last_text: std::sync::Mutex::new(std::collections::HashMap::new()),
                }) {
                    let state = app.state::<commands::AppState>();
                    if let Ok(mut guard) = state.bot.lock() {
                        *guard = Some(token);
                    }
                    if let Ok(mut started) = state.bot_started_at.lock() {
                        *started = Some(crate::bot::started_at_now());
                    }
                    log_line(&home, "机器人已自动启动");
                }
            }

            let icon = Image::from_bytes(ICON_PNG).map_err(|e| format!("图标加载失败: {e}"))?;

            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title(TITLE)
                .inner_size(1240.0, 880.0)
                .min_inner_size(1020.0, 680.0)
                .background_color(tauri::window::Color(11, 15, 26, 255))
                .data_directory(home.join("data").join("webview2"))
                .icon(icon.clone())?
                .build()?;

            let w = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = w.hide();
                }
            });

            let show = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&show, &sep, &quit])?;

            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .tooltip(TITLE)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => {
                        if let Some(state) = app.try_state::<Backend>() {
                            if let Ok(mut guard) = state.0.lock() {
                                if let Some(mut c) = guard.take() {
                                    let _ = c.kill();
                                }
                            }
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            log_line(&home, "窗口与托盘就绪");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri 运行失败");
}
