//! 窗口定位与显隐。
//!
//! 面板位置是**固定**的：永远贴在「光标所在那块屏」的顶边居中，且窗口本身不可被拖动
//! （顶栏没有 `data-tauri-drag-region`）。早期版本支持在光标旁弹出，已移除 ——
//! 位置一旦能变，下次热区呼出时前后跳位反而更像 bug。

use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindow};

use crate::hotzone::HotZoneState;

pub const MAIN_WINDOW: &str = "main";

/// 面板每次重新弹出时发给前端的事件。
///
/// 面板是被热区反复收起 / 弹出的，React 树并不卸载，组件里的临时状态会一直留着。
/// 前端靠它把「筛选中」这类一次性状态清掉，**并且把进场动画重放一道**
/// （CSS 动画只在元素创建时跑一次，不重放就没有「滑出」效果）。
pub const PANEL_SHOWN_EVENT: &str = "vigilshelf://panel-shown";

/// 请求前端收起面板：前端放完退场动画后自己调 `hide_window`。
///
/// 为什么要绕一圈：退场动画只能在 webview 里放，而窗口一旦被 `hide()` 就什么也看不见了。
pub const PANEL_HIDING_EVENT: &str = "vigilshelf://panel-hiding";

/// 退场动画的安全阀。前端若在这个时间内没有回来调 `hide_window`（卡住、白屏、事件没送到），
/// 就直接隐藏 —— 否则面板会永远留在屏幕上，只能杀进程。必须大于前端动画时长。
const HIDE_FALLBACK_DELAY: Duration = Duration::from_millis(600);

/// 面板的逻辑尺寸。
///
/// 这里（而不是 `Settings`）是面板尺寸的唯一来源。旧版本把它放在用户设置里，造成三处都能决定
/// 面板大小：`tauri.conf.json`、`Settings::default()`、以及磁盘上已经存在的 `settings.json`。
/// 面板不支持拖边调整大小，所以它本来就不是「用户设置」，还是常量更诚实 —— 改一改就能生效，
/// 不会被旧配置默默压回去。
///
/// 高度是按「列表区正好放得下 4 列 × 2 行卡片」定的。各项都源自基准字号 13px:
/// 顶栏 `42.25 + 1` + 标签栏 `6 + 27.95 + 6 + 1` + 列表 `6 + 两行卡片 + 行距 + 8` +
/// 状态栏 `26 + 1` + `.app` 上下边框 `2`。这些倍数写在 `src/lib/appearance.ts`
/// 的 `sizeVars` 里，改那边必须回头改这里。
///
/// 列表区两行的行高不是写死的，而是 `grid-auto-rows: calc((100% - var(--card-gap)) / 2)`
/// 由容器高度反推 —— 所以面板高度一变，卡片自动跟着变高变矮，第 3 行永远落在可视区外。
/// 270 这个总分是拿截图量出来的。
///
/// **只在默认字号 13px 下刚好放得下两行卡片的全部内容**：字号调到上限 16px 时行高基本不变、
/// 而文字和图标会变大，卡片会更挤（`.card` 的 `overflow: hidden` 只是兜底，不是设计）。
/// Ctrl+F 的筛选条会在列表区上方多占一行，此时两行卡片再各压矮约一半的筛选条高度，
/// 同理更挤。想要「不管字号和筛选条都在不在，卡片内容都恰恰好」，得让前端算完高度再回传
/// （注意 `anchor_to_top` 会把高度重置回这里，所以要存成共享状态），目前认为不值得。
pub const PANEL_WIDTH: u32 = 560;
pub const PANEL_HEIGHT: u32 = 270;

pub fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MAIN_WINDOW)
}

/// 一块显示器的物理矩形。只需要「左右居中」和「顶边对齐」两个信息，所以不含高度。
struct MonitorRect {
    x: i32,
    y: i32,
    w: i32,
    scale: f64,
}

/// 取「光标所在那块屏」；取不到时退回窗口当前所在屏。
fn monitor_rect(
    window: &WebviewWindow,
    point: Option<(f64, f64)>,
) -> tauri::Result<Option<MonitorRect>> {
    let by_point = match point {
        Some((x, y)) => window.monitor_from_point(x, y)?,
        None => None,
    };
    let monitor = match by_point {
        Some(m) => Some(m),
        None => window.current_monitor()?,
    };
    Ok(monitor.map(|m| {
        let pos = m.position();
        let size = m.size();
        MonitorRect {
            x: pos.x,
            y: pos.y,
            w: size.width as i32,
            scale: m.scale_factor(),
        }
    }))
}

/// 把面板设成 `PANEL_WIDTH` × `PANEL_HEIGHT`。
///
/// 用**逻辑**尺寸而不是物理尺寸，保证 125%/150% 缩放的屏幕上视觉大小一致 ——
/// 否则同样一行代码，在 150% 屏上做出来的面板会比其他屏大一半。
pub fn apply_panel_size(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = main_window(app) else {
        return Ok(());
    };
    window.set_size(LogicalSize::new(PANEL_WIDTH as f64, PANEL_HEIGHT as f64))
}

pub fn anchor_to_top(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = main_window(app) else {
        return Ok(());
    };
    let cursor = window.cursor_position().ok();
    let rect = monitor_rect(&window, cursor.map(|c| (c.x, c.y)))?;
    apply_panel_size(app)?;
    if let Some(r) = rect {
        let w = (PANEL_WIDTH as f64 * r.scale).round() as i32;
        let x = r.x + (r.w - w).max(0) / 2;
        window.set_position(PhysicalPosition::new(x, r.y))?;
    }
    Ok(())
}

/// 面板是不是真的「在外面」。
///
/// 只看 `is_visible()` 不够：被最小化（Win+D、Win+M、或别的东西）的窗口在 Windows 上
/// `IsWindowVisible` 仍是 true，窗口只是被挪到屏幕外（约 -32000）并缩小。
/// 那种状态下热区会以为自己已经弹出、永不再触发，于是面板彻底叫不回来 —— 必须把
/// 「最小化」也算成没弹出，并且唤起时先 `unminimize`。
pub fn panel_is_up(window: &WebviewWindow) -> bool {
    window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false)
}

/// 显示并置顶，同时抢到焦点，让 Esc / 空格预览这类键盘操作立即可用。
pub fn show(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = main_window(app) else {
        return Ok(());
    };
    // 先还原：面板可能被 Win+D 之类的操作最小化过，不还原的话 show 出来也是个小窗口
    let _ = window.unminimize();
    window.show()?;
    window.set_always_on_top(true)?;
    window.set_focus()?;
    if let Some(state) = app.try_state::<HotZoneState>() {
        state.begin_auto_hide();
    }
    // 通知前端「面板刚重新弹出」。前端在这里清掉 Ctrl+F 的筛选条件 ——
    // 条件留着会让列表看起来像文件丢了。发射失败不影响显示，所以吞掉错误。
    let _ = app.emit(PANEL_SHOWN_EVENT, ());
    Ok(())
}

pub fn hide(app: &AppHandle) -> tauri::Result<()> {
    let Some(window) = main_window(app) else {
        return Ok(());
    };
    if let Some(state) = app.try_state::<HotZoneState>() {
        state.reset();
    }
    window.hide()
}

/// 请求收起面板：让前端先把退场动画放完，再由前端调 `hide_window`。
///
/// 用一个原子标志保证「一次收起只发一次事件」：热区轮询每 60ms 就会问一次要不要收起，
/// 而前端从收到事件到真正隐藏有 170ms，不加这个标志会连发好几次。
/// 同一个标志也兼作安全阀的判据 —— 前端隐藏完（或面板被重新弹出）时会被清掉。
pub fn request_hide(app: &AppHandle) {
    let Some(window) = main_window(app) else {
        return;
    };
    if !panel_is_up(&window) {
        return;
    }
    let Some(state) = app.try_state::<HotZoneState>() else {
        // 拿不到状态就直接收，不折腾动画
        let _ = hide(app);
        return;
    };
    if state
        .hide_pending
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
        .is_err()
    {
        return; // 已经有一次收起在飞了
    }

    let _ = app.emit(PANEL_HIDING_EVENT, ());

    // 安全阀。只在前端一直没回来的情况下生效：
    // 前端正常隐藏 → `reset()` 清掉标志；
    // 用户抢先又把面板唤出来 → `begin_auto_hide()` 清掉标志。
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(HIDE_FALLBACK_DELAY);
        let Some(state) = app.try_state::<HotZoneState>() else {
            return;
        };
        if state.hide_pending.load(Ordering::Acquire) {
            let _ = hide(&app);
        }
    });
}

/// 全局快捷键与托盘：可见就收起（带退场动画），不可见就在顶部居中弹出。
pub fn toggle_panel(app: &AppHandle) {
    let Some(window) = main_window(app) else {
        return;
    };
    if panel_is_up(&window) {
        request_hide(app);
        return;
    }
    let _ = anchor_to_top(app);
    let _ = show(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `tauri.conf.json` 里的窗口尺寸只在「窗口被创建」的那一帧用得上，随后立刻被
    /// `apply_panel_size` 覆盖。两处不一致时表现为首次显示前面板闪一下别的尺寸 ——
    /// 肉眼几乎抓不到，但确实是 bug，所以用测试钉住。
    #[test]
    fn tauri_conf_agrees_with_panel_size() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .expect("tauri.conf.json 必须是合法 JSON");
        let window = &conf["app"]["windows"][0];
        assert_eq!(
            window["width"].as_u64(),
            Some(PANEL_WIDTH as u64),
            "tauri.conf.json 的窗口宽度必须与 PANEL_WIDTH 一致，否则首帧会闪一下别的尺寸"
        );
        assert_eq!(
            window["height"].as_u64(),
            Some(PANEL_HEIGHT as u64),
            "tauri.conf.json 的窗口高度必须与 PANEL_HEIGHT 一致，否则首帧会闪一下别的尺寸"
        );
    }
}
