// 翻译机器人 · Tauri 宿主
// 单实例 + 拉起 Python 后端 + 系统 WebView2 窗口 + 托盘
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

const ICON_PNG: &[u8] = include_bytes!("../../assets/icon-512.png");
const TITLE: &str = "翻译机器人";

struct Backend(Mutex<Option<Child>>);

fn root_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn log_line(root: &Path, msg: &str) {
    use std::io::Write;
    let dir = root.join("logs");
    let _ = fs::create_dir_all(&dir);
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("host.log"))
    {
        let _ = writeln!(f, "{}", msg);
    }
}

/// 安装版：程序在 %LOCALAPPDATA%\Programs\TranslatorBot，负载与数据在 %LOCALAPPDATA%\TranslatorBot
/// 绿色版：exe 与 backend\ 同目录。返回 (数据根目录, 后端 exe)
fn resolve_paths(exe_dir: &Path) -> (PathBuf, PathBuf) {
    if let Ok(local) = env::var("LOCALAPPDATA") {
        let base = PathBuf::from(local).join("TranslatorBot");
        let backend = base
            .join("app")
            .join("backend")
            .join("TranslatorBotBackend.exe");
        if backend.exists() {
            return (base, backend);
        }
    }
    let backend = exe_dir.join("backend").join("TranslatorBotBackend.exe");
    (exe_dir.to_path_buf(), backend)
}

fn start_backend(home: &Path, backend: &Path) -> Option<Child> {
    log_line(home, &format!("backend: {}", backend.display()));
    Command::new(backend)
        .current_dir(home)
        .env("TRANSLATORBOT_HOME", home)
        .spawn()
        .ok()
}

fn wait_for_port(root: &Path, timeout: Duration) -> Option<String> {
    let port_file = root.join("data").join("ui.port");
    let _ = fs::remove_file(&port_file);
    let start = Instant::now();
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .setup(|app| {
            let exe_dir = root_dir();
            let (root, backend) = resolve_paths(&exe_dir);
            log_line(&root, "Tauri 宿主启动");

            let child = start_backend(&root, &backend).ok_or("后端启动失败")?;
            let url = wait_for_port(&root, Duration::from_secs(30)).ok_or("后端未就绪")?;
            log_line(&root, &format!("backend ready: {}", url));
            app.manage(Backend(Mutex::new(Some(child))));

            let icon = Image::from_bytes(ICON_PNG)?;
            let parsed = url.parse().map_err(|e| format!("URL 无效: {}", e))?;

            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(parsed))
                .title(TITLE)
                .inner_size(1240.0, 880.0)
                .min_inner_size(1020.0, 680.0)
                .background_color(tauri::window::Color(11, 15, 26, 255))
                .data_directory(root.join("data").join("webview2"))
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

            log_line(&root, "窗口与托盘就绪");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri 运行失败");
}


