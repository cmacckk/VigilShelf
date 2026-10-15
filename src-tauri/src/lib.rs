//! VigilShelf —— Windows 版 Folder Hub。
//!
//! 鼠标移到屏幕顶部边缘即滑出的文件浏览器。

mod fonts;
mod fs_ops;
mod hotzone;
mod icons;
mod model;
mod store;
mod window_ctl;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use tauri::{
    AppHandle, Manager, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use hotzone::HotZoneState;
use model::{DirListing, PasteReport, QuickLocation, Settings, Workspace};

/* ------------------------------- 文件系统 ------------------------------- */

#[tauri::command]
fn list_dir(path: String, show_hidden: bool) -> Result<DirListing, String> {
    fs_ops::list_dir_impl(&path, show_hidden)
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    fs_ops::open_path_impl(&path)
}

#[tauri::command]
fn reveal_in_explorer(path: String) -> Result<(), String> {
    fs_ops::reveal_impl(&path)
}

#[tauri::command]
fn trash_path(path: String) -> Result<(), String> {
    fs_ops::trash_impl(&path)
}

#[tauri::command]
fn show_open_with_dialog(path: String) -> Result<(), String> {
    fs_ops::show_open_with_dialog_impl(&path)
}

#[tauri::command]
fn copy_to_clipboard(paths: Vec<String>) -> Result<(), String> {
    fs_ops::copy_to_clipboard_impl(&paths)
}

/// 剪贴板里不是文件时返回 `null`。
#[tauri::command]
fn clipboard_files() -> Result<Option<Vec<String>>, String> {
    fs_ops::clipboard_files_impl()
}

#[tauri::command]
fn paste_into(dest_dir: String, move_mode: bool) -> Result<PasteReport, String> {
    let sources = fs_ops::clipboard_files_impl()?.unwrap_or_default();
    if sources.is_empty() {
        return Ok(PasteReport::default());
    }
    fs_ops::transfer_impl(&sources, &dest_dir, move_mode)
}

#[tauri::command]
fn transfer_into(
    sources: Vec<String>,
    dest_dir: String,
    move_mode: bool,
) -> Result<PasteReport, String> {
    fs_ops::transfer_impl(&sources, &dest_dir, move_mode)
}

#[tauri::command]
fn list_quick_locations() -> Vec<QuickLocation> {
    store::quick_locations()
}

/// 设置面板的字体下拉用。首次调用会拉起一次 PowerShell 读注册表（约几百毫秒），之后走缓存。
#[tauri::command]
fn list_system_fonts() -> Vec<String> {
    fonts::list_system_fonts()
}

/// 一批路径 → PNG data URL。取不到的路径不会出现在结果里，前端保留 SVG 兜底。
///
/// 声明成 `async` 是为了不占主线程：取图标要走 Windows 外壳，冷启动首次调用可能要几百毫秒，
/// 同步命令会把这期间的界面冻住。
#[tauri::command]
async fn shell_icons(paths: Vec<String>) -> HashMap<String, String> {
    icons::shell_icons(&paths)
}

/* ------------------------------- 工作空间 ------------------------------- */

#[tauri::command]
fn get_workspaces(app: AppHandle) -> Vec<Workspace> {
    store::load_workspaces(&app)
}

#[tauri::command]
fn save_workspaces(app: AppHandle, workspaces: Vec<Workspace>) -> Result<(), String> {
    store::save_workspaces(&app, &workspaces)
}

/* -------------------------------- 设置 -------------------------------- */

#[tauri::command]
fn get_settings(app: AppHandle) -> Settings {
    store::load_settings(&app)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    store::save_settings(&app, &settings)?;
    apply_settings(&app, &settings)
}

/* -------------------------------- 窗口 -------------------------------- */

#[tauri::command]
fn hide_window(app: AppHandle) -> Result<(), String> {
    window_ctl::hide(&app).map_err(|e| e.to_string())
}

/// 前端在弹出菜单/对话框期间挂起自动收起。
#[tauri::command]
fn set_hotzone_hold(app: AppHandle, hold: bool) {
    if let Some(state) = app.try_state::<HotZoneState>() {
        state.hold.store(hold, Ordering::Relaxed);
    }
}

/* -------------------------------- 拖拽 -------------------------------- */

/// 把文件从面板里拖到外部程序。
///
/// Tauri 2 内核没有这个能力（`startDragging` 是拖窗口），所以用 `drag` crate 走 OLE。
/// `DoDragDrop` 必须跑在有消息循环的 UI 线程上，因此 dispatch 到主线程执行；
/// 命令本身声明成 `async`，避免把主线程和等待结果的主线程锁死。
#[tauri::command]
async fn drag_out(app: AppHandle, paths: Vec<String>) -> Result<(), String> {
    let items: Vec<PathBuf> = paths
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.exists())
        .collect();
    if items.is_empty() {
        return Err("没有可拖出的文件：路径必须是存在的绝对路径".into());
    }
    let Some(window) = window_ctl::main_window(&app) else {
        return Err("找不到主窗口".into());
    };

    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let icon = drag::Image::Raw(include_bytes!("../icons/32x32.png").to_vec());
        let result = drag::start_drag(
            &window,
            drag::DragItem::Files(items),
            icon,
            |_result, _cursor| {},
            drag::Options::default(),
        );
        let _ = tx.send(result.map_err(|e| e.to_string()));
    })
    .map_err(|e| format!("无法切到主线程执行拖拽：{e}"))?;

    rx.recv().map_err(|e| format!("拖拽线程异常退出：{e}"))?
}

/* ------------------------------ 设置生效 ------------------------------ */

fn apply_settings(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    if let Some(state) = app.try_state::<HotZoneState>() {
        state.apply(settings);
    }

    // 快捷键可能改了，先全部注销再按新值注册
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    let shortcut: Shortcut = settings
        .hotkey
        .parse()
        .map_err(|e| format!("快捷键「{}」无法解析：{e}", settings.hotkey))?;
    shortcuts
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                window_ctl::toggle_panel(app);
            }
        })
        .map_err(|e| format!("快捷键「{}」注册失败：{e}", settings.hotkey))?;

    // 面板大小是常量，不跟用户设置挂钩（见 window_ctl::PANEL_WIDTH）
    let _ = window_ctl::apply_panel_size(app);
    Ok(())
}

/* -------------------------------- 托盘 -------------------------------- */

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "显示 / 收起面板", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出 VigilShelf", true, None::<&str>)?;
    // 面板位置是固定的，所以不再需要「贴到屏幕顶部居中」这一项
    let menu = Menu::with_items(app, &[&toggle, &separator, &quit])?;

    let mut builder = TrayIconBuilder::with_id("vigilshelf-tray")
        .tooltip("VigilShelf —— 鼠标移到屏幕顶部即滑出")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => window_ctl::toggle_panel(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window_ctl::toggle_panel(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/* -------------------------------- 启动 -------------------------------- */

pub fn run() {
    // 踩过的坑：普通的 `cargo build --release` 产出的是 **dev 构建** —— 它不内嵌 `dist/`，
    // 而是去连 `build.devUrl`（http://localhost:1420）。若那一刻正好有别的项目在 1420 上跑 Vite，
    // 面板就会渲染出**别的项目的界面**，看起来像“代码被污染”。发布只能走 `pnpm tauri build`。
    if tauri::is_dev() {
        eprintln!(
            "[vigilshelf] 警告：这是 dev 构建，界面来自 build.devUrl 而不是内嵌的 dist/。发布请用 `pnpm tauri build`。"
        );
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(HotZoneState::default())
        .invoke_handler(tauri::generate_handler![
            list_dir,
            open_path,
            reveal_in_explorer,
            trash_path,
            show_open_with_dialog,
            copy_to_clipboard,
            clipboard_files,
            paste_into,
            transfer_into,
            list_quick_locations,
            list_system_fonts,
            shell_icons,
            get_workspaces,
            save_workspaces,
            get_settings,
            save_settings,
            hide_window,
            set_hotzone_hold,
            drag_out,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = store::load_settings(&handle);

            // 启动时先摆好尺寸，免得首次呼出时先闪一下默认大小的窗口
            let _ = window_ctl::apply_panel_size(&handle);

            if let Some(state) = handle.try_state::<HotZoneState>() {
                state.apply(&settings);
            }
            // 快捷键注册失败（被别的软件占用）不应该拦住启动，托盘仍然可用
            if let Err(e) = apply_settings(&handle, &settings) {
                eprintln!("[vigilshelf] {e}");
            }
            setup_tray(&handle)?;
            hotzone::spawn(handle);

            // 预取系统字体列表。它要拉起一次 PowerShell，首次约 1–2 秒；
            // 不预热的话，用户第一次点开设置时会先看到一个空的下拉框。
            // 刻意不 join：字体列表只影响下拉框，慢了不能拖住启动。
            std::thread::spawn(|| {
                fonts::list_system_fonts();
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            // 关窗 = 收起面板，不退出应用；退出走托盘菜单。
            // 走 request_hide 而不是直接 hide，是为了让前端先把退场动画放完。
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                window_ctl::request_hide(window.app_handle());
            }
        })
        .run(tauri::generate_context!())
        .expect("VigilShelf 启动失败");
}
