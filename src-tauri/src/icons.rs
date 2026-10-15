//! 用 Windows 外壳自己那套图标，而不是我们手画的 SVG。
//!
//! 为什么不自己调 `SHGetFileInfo`：它只能给到 16 / 32px 的小图标，在 125% / 150% 的缩放屏上
//! 放大后是糊的。`file_icon_provider` 走的是 `IShellItemImageFactory::GetImage` ——
//! 和资源管理器**同一套**取图路径，可以按需拿任意尺寸。
//!
//! 它还自带一个正合我们需要的缓存策略：**按扩展名缓存，但 `.exe` / `.lnk` / 无扩展名永不缓存**
//! （这两种文件的图标是它们自己带的，不是格式通用的）。所以「一个目录 500 个文件」
//! 实际只会有十几次真正的外壳调用，其余都是查表。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use file_icon_provider::{Icon, Provider};

/// 向外壳要的尺寸。48 是外壳的标准档位之一（`SHIL_EXTRALARGE`），
/// 而卡片上只画约 18px —— 多出来的留给高分屏，让它在 150% 缩放下依然清楚。
const ICON_SIZE: u16 = 48;

/// 一次调用最多解析多少个路径。面板一屏只有 8 张卡片，400 已经远超实际浏览深度；
/// 设上限是为了「前端误传一个上万文件的目录」不会把 IPC 和外壳调用撑爆。
const MAX_PATHS: usize = 400;

/// 把 RGBA 编成 PNG，再包成 data URL。
///
/// 直接把 RGBA 发给前端有两个问题：一是前端还得自己画 canvas，二是 Tauri 的 IPC 走 JSON，
/// 原始字节会膨胀成数字列表，比 base64 还大一圈。PNG 顺手把它压到 1–3 KB。
///
/// 这个函数会被当作 `fn` 指针交给 `Provider`，所以不能是闭包。
fn encode_png_data_url(icon: Icon) -> String {
    let mut png_bytes: Vec<u8> = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_bytes, icon.width, icon.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        // 编码失败就返回空串，前端会退回手画的 SVG —— 拿不到图标不该让整次请求失败
        if let Ok(mut writer) = encoder.write_header() {
            let _ = writer.write_image_data(&icon.pixels);
        }
    }
    if png_bytes.is_empty() {
        return String::new();
    }
    format!("data:image/png;base64,{}", STANDARD.encode(&png_bytes))
}

/// 全局唯一的图标提供者（进程内只建一次，缓存也就跟着只建一次）。
///
/// `Provider` 内部是 `RefCell`，所以它不是 `Sync`，必须放进 `Mutex`。
/// 外壳那边本来也是单线程串行取图的，锁不会成为瓶颈。
fn provider() -> Option<&'static Mutex<Provider<String>>> {
    static PROVIDER: OnceLock<Option<Mutex<Provider<String>>>> = OnceLock::new();
    PROVIDER
        .get_or_init(|| {
            Provider::new(ICON_SIZE, encode_png_data_url)
                .ok()
                .map(Mutex::new)
        })
        .as_ref()
}

/// 批量为一批路径取外壳图标。
///
/// 返回「路径 → PNG data URL」。**取不到的路径不会出现在返回值里**（而不是给一个空串），
/// 前端据此继续用手画的 SVG 兜底，也能顺带把「这个路径没有图标」缓存下来不再重问。
pub fn shell_icons(paths: &[String]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Some(provider) = provider() else {
        return out;
    };
    let Ok(provider) = provider.lock() else {
        return out;
    };
    for path in paths.iter().take(MAX_PATHS) {
        if out.contains_key(path) {
            continue;
        }
        // 取不到图标时 `get_file_icon` 返回 Err，直接跳过 —— 前端会保留 SVG 兜底
        if let Ok(url) = provider.get_file_icon(path)
            && !url.is_empty()
        {
            out.insert(path.clone(), url);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_rgba_icon_round_trips_into_a_png_data_url() {
        let icon = Icon {
            width: 2,
            height: 2,
            pixels: vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 0,
            ],
        };
        let url = encode_png_data_url(icon);
        assert!(
            url.starts_with("data:image/png;base64,iVBOR"),
            "PNG 的 base64 必须以 iVBOR 开头，实际是 {url}"
        );
    }

    #[test]
    fn missing_paths_are_skipped_instead_of_returned_empty() {
        let missing = "F:\\VigilShelf\\绝对不存在的目录\\绝对不存在的文件.zzz".to_string();
        let found = shell_icons(std::slice::from_ref(&missing));
        assert!(
            !found.contains_key(&missing),
            "取不到图标的路径不应出现在结果里，前端要据此保留 SVG 兜底"
        );
    }
}
