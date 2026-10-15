//! 屏幕顶边热区。
//!
//! Windows 没有 Mac 刘海，所以用「屏幕最上沿 1px + 停留一段时间」等价替代：
//! 触发带做得极窄是刻意的 —— 避免用户把鼠标推到顶部时误触发。
//!
//! 独立线程每 60ms 轮询一次光标位置，不做全局鼠标钩子（钩子更容易被杀软拦）。
//! 全屏应用盖住顶边时热区会失效，这是预期行为，由全局快捷键兜底。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, PhysicalPosition, WebviewWindow};

use crate::model::Settings;
use crate::window_ctl;

const POLL_INTERVAL: Duration = Duration::from_millis(60);
/// 顶边触发带高度（物理像素）。
const HOT_ZONE_HEIGHT: f64 = 1.0;
/// 判定「鼠标还在窗口里」时向外放宽的缓冲，防止边缘抖动导致反复收起
const LEAVE_MARGIN: f64 = 8.0;

/// 热区运行态。用原子量而不是 Mutex：轮询线程每 60ms 都要读，别互相等。
pub struct HotZoneState {
    pub enabled: AtomicBool,
    pub hotzone_delay_ms: AtomicU64,
    pub hide_delay_ms: AtomicU64,
    /// true 表示窗口会随鼠标离开自动收起
    pub auto_hide: AtomicBool,
    /// 光标是否已经进过窗口。没进过就不自动收起，避免刚弹出就消失。
    pub armed: AtomicBool,
    /// 前端临时挂起自动收起（例如系统「打开方式」对话框弹出期间）
    pub hold: AtomicBool,
    /// true 表示「已经通知前端放退场动画，正等它回来隐藏」。
    /// 兼作两件事的判据：不让热区每 60ms 重发一次事件；兜底线程据此判断前端是不是卡住了。
    pub hide_pending: AtomicBool,
}

impl Default for HotZoneState {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            hotzone_delay_ms: AtomicU64::new(150),
            hide_delay_ms: AtomicU64::new(300),
            auto_hide: AtomicBool::new(false),
            armed: AtomicBool::new(false),
            hold: AtomicBool::new(false),
            hide_pending: AtomicBool::new(false),
        }
    }
}

impl HotZoneState {
    pub fn apply(&self, settings: &Settings) {
        self.enabled
            .store(settings.hotzone_enabled, Ordering::Relaxed);
        self.hotzone_delay_ms
            .store(settings.hotzone_delay_ms, Ordering::Relaxed);
        self.hide_delay_ms
            .store(settings.hide_delay_ms, Ordering::Relaxed);
    }

    /// 窗口刚刚显示：开启自动收起，但等光标真的进过窗口才开始计时。
    pub fn begin_auto_hide(&self) {
        self.auto_hide.store(true, Ordering::Relaxed);
        self.armed.store(false, Ordering::Relaxed);
        // 面板又被唤出来了，上一次的收起请求作废 ——
        // 否则前端那边正在跑的退场动画会把刚弹出来的面板又收回去。
        self.hide_pending.store(false, Ordering::Release);
    }

    pub fn reset(&self) {
        self.auto_hide.store(false, Ordering::Relaxed);
        self.armed.store(false, Ordering::Relaxed);
        self.hold.store(false, Ordering::Relaxed);
        self.hide_pending.store(false, Ordering::Release);
    }
}

fn point_in_window(window: &WebviewWindow, p: PhysicalPosition<f64>) -> bool {
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        // 拿不到窗口矩形时按「在里面」处理，宁可留着也不要误收起
        return true;
    };
    let left = pos.x as f64 - LEAVE_MARGIN;
    let top = pos.y as f64 - LEAVE_MARGIN;
    let right = (pos.x + size.width as i32) as f64 + LEAVE_MARGIN;
    let bottom = (pos.y + size.height as i32) as f64 + LEAVE_MARGIN;
    p.x >= left && p.x <= right && p.y >= top && p.y <= bottom
}

pub fn spawn(app: AppHandle) {
    let _ = thread::Builder::new()
        .name("vigilshelf-hotzone".into())
        .spawn(move || run(app));
}

fn run(app: AppHandle) {
    // 光标在顶边停留的起点
    let mut in_zone_since: Option<Instant> = None;
    // 光标离开窗口的起点
    let mut out_since: Option<Instant> = None;

    loop {
        thread::sleep(POLL_INTERVAL);

        let Some(window) = window_ctl::main_window(&app) else {
            continue;
        };
        let Some(state) = app.try_state::<HotZoneState>() else {
            continue;
        };
        let Ok(cursor) = window.cursor_position() else {
            continue;
        };

        // 「在外面」的判据走 panel_is_up：被最小化的窗口 is_visible() 仍是 true，
        // 只看它会让面板被最小化后再也唤不回来。
        if window_ctl::panel_is_up(&window) {
            in_zone_since = None;

            let inside = point_in_window(&window, cursor);
            if inside {
                state.armed.store(true, Ordering::Relaxed);
            }
            let should_hide = state.auto_hide.load(Ordering::Relaxed)
                && !state.hold.load(Ordering::Relaxed)
                && state.armed.load(Ordering::Relaxed)
                && !inside;

            if !should_hide {
                out_since = None;
                continue;
            }
            let started = *out_since.get_or_insert_with(Instant::now);
            if started.elapsed()
                >= Duration::from_millis(state.hide_delay_ms.load(Ordering::Relaxed))
            {
                // 不直接 hide：让前端先把退场动画放完
                window_ctl::request_hide(&app);
                out_since = None;
            }
        } else {
            out_since = None;
            if !state.enabled.load(Ordering::Relaxed) {
                in_zone_since = None;
                continue;
            }
            // 只在「光标所在那块显示器」的顶边触发，多屏下不会误弹到主屏
            let Ok(Some(monitor)) = window.monitor_from_point(cursor.x, cursor.y) else {
                in_zone_since = None;
                continue;
            };
            let top = monitor.position().y as f64;
            if cursor.y > top + HOT_ZONE_HEIGHT {
                in_zone_since = None;
                continue;
            }
            let started = *in_zone_since.get_or_insert_with(Instant::now);
            if started.elapsed()
                < Duration::from_millis(state.hotzone_delay_ms.load(Ordering::Relaxed))
            {
                continue;
            }
            in_zone_since = None;

            if window_ctl::anchor_to_top(&app).is_err() {
                continue;
            }
            let _ = window_ctl::show(&app);
        }
    }
}
