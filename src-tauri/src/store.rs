//! 配置文件读写。
//!
//! 放在 `%APPDATA%\com.mechrevo.vigilshelf\` 下：
//! - `settings.json`   —— 用户设置
//! - `workspaces.json` —— 工作空间列表
//!
//! 读取策略一律「失败即回落默认值」：配置坏掉时应用要能用，而不是打不开。
//!
//! 目录名是从 `identifier` 推出来的，所以项目改过一次名字就得跟着迁移 —— 见 `copy_legacy_config`。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tauri::{AppHandle, Manager};

use crate::model::{QuickLocation, Settings, Workspace};

const SETTINGS_FILE: &str = "settings.json";
const WORKSPACES_FILE: &str = "workspaces.json";

/// 旧版本（还叫 ReachFile 时）的 identifier，也就是旧的配置目录名。
const LEGACY_DIR_NAME: &str = "com.mechrevo.reachfile";

/// 整个进程只尝试迁移一次（`config_dir` 会被反复调用）。
static MIGRATION_DONE: OnceLock<()> = OnceLock::new();

/// 配置目录，不存在时创建；顺便把改名前的老配置搬过来。
pub fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("无法定位配置目录：{e}"))?;
    fs::create_dir_all(&dir).map_err(|e| format!("无法创建配置目录 {}：{e}", dir.display()))?;
    MIGRATION_DONE.get_or_init(|| {
        let copied = dir
            .parent()
            .map(|parent| copy_legacy_config(&dir, &parent.join(LEGACY_DIR_NAME)))
            .unwrap_or(0);
        if copied > 0 {
            eprintln!("[vigilshelf] 已从旧配置目录迁移 {copied} 个文件");
        }
    });
    Ok(dir)
}

/// 把改名前的老配置文件复制到新目录，返回复制成功的文件数。
///
/// **只读旧目录、只写新目录**：老文件原地不动，迁移出问题也能手工回退。
/// 新目录里已经有任一配置文件就不迁 —— 用户可能已经在新版里改过设置，
/// 再拿旧文件盖回去会让设置自己变回去，比丢失更难排查。
fn copy_legacy_config(new_dir: &Path, old_dir: &Path) -> usize {
    if !old_dir.is_dir() {
        return 0;
    }
    if [SETTINGS_FILE, WORKSPACES_FILE]
        .iter()
        .any(|name| new_dir.join(name).is_file())
    {
        return 0;
    }
    [SETTINGS_FILE, WORKSPACES_FILE]
        .iter()
        .filter(|name| {
            let from = old_dir.join(*name);
            from.is_file() && fs::copy(&from, new_dir.join(name)).is_ok()
        })
        .count()
}

/// 读 JSON；文件不存在、无权限、内容损坏时一律返回 None。
fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str::<T>(&text).ok()
}

fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| format!("序列化失败：{e}"))?;
    fs::write(path, text).map_err(|e| format!("无法写入 {}：{e}", path.display()))
}

pub fn load_settings_from(dir: &Path) -> Settings {
    // 先回落默认值、再夹一次区间：手改过的 settings.json 不能把窗口尺寸之类的值带越界
    read_json::<Settings>(&dir.join(SETTINGS_FILE))
        .unwrap_or_default()
        .sanitized()
}

pub fn load_workspaces_from(dir: &Path) -> Vec<Workspace> {
    match read_json::<Vec<Workspace>>(&dir.join(WORKSPACES_FILE)) {
        Some(list) if !list.is_empty() => list,
        // 文件缺失、损坏，或存的是空数组 —— 都回落到默认工作空间
        _ => default_workspaces(),
    }
}

pub fn load_settings(app: &AppHandle) -> Settings {
    config_dir(app)
        .map(|d| load_settings_from(&d))
        .unwrap_or_default()
}

pub fn save_settings(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let dir = config_dir(app)?;
    write_json(&dir.join(SETTINGS_FILE), settings)
}

pub fn load_workspaces(app: &AppHandle) -> Vec<Workspace> {
    match config_dir(app) {
        Ok(d) => load_workspaces_from(&d),
        Err(_) => default_workspaces(),
    }
}

pub fn save_workspaces(app: &AppHandle, workspaces: &[Workspace]) -> Result<(), String> {
    let dir = config_dir(app)?;
    write_json(&dir.join(WORKSPACES_FILE), workspaces)
}

/// 首次启动（或配置损坏）时给的工作空间。
pub fn default_workspaces() -> Vec<Workspace> {
    let candidates: [(&str, Option<PathBuf>); 2] = [
        ("桌面", dirs::desktop_dir()),
        ("下载", dirs::download_dir()),
    ];
    candidates
        .into_iter()
        .filter_map(|(name, path)| path.filter(|p| p.is_dir()).map(|path| (name, path)))
        .map(|(name, path)| Workspace {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            path: path.to_string_lossy().into_owned(),
        })
        .collect()
}

/// 「添加工作空间」对话框里可选的预设位置。
pub fn quick_locations() -> Vec<QuickLocation> {
    let candidates: [(&str, Option<PathBuf>, &str); 6] = [
        ("主目录", dirs::home_dir(), "home"),
        ("桌面", dirs::desktop_dir(), "desktop"),
        ("下载", dirs::download_dir(), "download"),
        ("文档", dirs::document_dir(), "document"),
        ("图片", dirs::picture_dir(), "picture"),
        ("视频", dirs::video_dir(), "video"),
    ];
    let mut out: Vec<QuickLocation> = candidates
        .into_iter()
        .filter_map(|(name, path, kind)| {
            let path = path.filter(|p| p.is_dir())?;
            Some(QuickLocation {
                name: name.into(),
                path: path.to_string_lossy().into_owned(),
                kind: kind.into(),
            })
        })
        .collect();

    // 盘符：A-Z 逐个探测，只有真实存在的根目录才列出来
    for letter in b'A'..=b'Z' {
        let root = format!("{}:\\", letter as char);
        if Path::new(&root).is_dir() {
            out.push(QuickLocation {
                name: root.clone(),
                path: root,
                kind: "drive".into(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vigilshelf-test-{}-{tag}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("测试目录应能创建");
        dir
    }

    #[test]
    fn legacy_config_is_copied_into_the_new_directory_once() {
        let root = temp_dir("migrate");
        let old = root.join(LEGACY_DIR_NAME);
        let new = root.join("com.mechrevo.vigilshelf");
        fs::create_dir_all(&old).expect("旧配置目录应能创建");
        fs::create_dir_all(&new).expect("新配置目录应能创建");
        fs::write(old.join(SETTINGS_FILE), r#"{"theme":"light"}"#).expect("写旧 settings");
        fs::write(old.join(WORKSPACES_FILE), "[]").expect("写旧 workspaces");

        assert_eq!(
            copy_legacy_config(&new, &old),
            2,
            "两个配置文件都该被复制过来"
        );
        assert!(
            new.join(SETTINGS_FILE).is_file(),
            "新的 settings.json 必须出现，否则用户升级后设置就没了"
        );
        assert!(
            old.join(SETTINGS_FILE).is_file(),
            "旧文件必须原地保留：迁移是复制，不能是移动"
        );

        // 第二次不能再迁：用户可能已经在新版里改过设置，被旧文件盖回去比丢失更难排查
        fs::write(new.join(SETTINGS_FILE), r#"{"theme":"dark"}"#).expect("改写新 settings");
        assert_eq!(
            copy_legacy_config(&new, &old),
            0,
            "新目录已有配置文件时不应再迁移"
        );
        let kept = fs::read_to_string(new.join(SETTINGS_FILE)).expect("读新 settings");
        assert!(
            kept.contains("dark"),
            "已有配置不能被旧文件盖掉，实际内容：{kept}"
        );

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn migration_without_a_legacy_directory_is_a_no_op() {
        let root = temp_dir("migrate-none");
        let new = root.join("com.mechrevo.vigilshelf");
        fs::create_dir_all(&new).expect("新配置目录应能创建");
        assert_eq!(
            copy_legacy_config(&new, &root.join(LEGACY_DIR_NAME)),
            0,
            "老目录不存在时应安静地返回 0，不能报错也不能建目录"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_settings_file_falls_back_to_default() {
        let dir = temp_dir("missing-settings");
        assert_eq!(load_settings_from(&dir), Settings::default());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn corrupted_settings_file_falls_back_to_default() {
        let dir = temp_dir("broken-settings");
        fs::write(dir.join(SETTINGS_FILE), "{ this is not json").unwrap();
        assert_eq!(
            load_settings_from(&dir),
            Settings::default(),
            "损坏的 settings.json 必须回落默认值，而不是让应用启动失败"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn empty_workspace_list_falls_back_to_defaults() {
        let dir = temp_dir("empty-ws");
        fs::write(dir.join(WORKSPACES_FILE), "[]").unwrap();
        let loaded = load_workspaces_from(&dir);
        assert!(
            loaded
                .iter()
                .all(|w| !w.name.is_empty() && !w.path.is_empty()),
            "空数组应回落到默认工作空间，而不是得到空列表：{loaded:?}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn saved_workspaces_survive_a_round_trip() {
        let dir = temp_dir("ws-roundtrip");
        let ws = vec![Workspace {
            id: "id-1".into(),
            name: "项目".into(),
            path: dir.to_string_lossy().into_owned(),
        }];
        write_json(&dir.join(WORKSPACES_FILE), &ws).unwrap();
        assert_eq!(load_workspaces_from(&dir), ws);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_settings_file_is_upgraded_not_reset() {
        // 0.1.0 写下的 settings.json 里没有外观字段。升级后这些值必须还在，
        // 只把新字段补成默认值 —— 否则用户的自定义配置会被静默清空。
        let dir = temp_dir("legacy-settings");
        fs::write(
            dir.join(SETTINGS_FILE),
            r#"{"hotkey":"Ctrl+Shift+F","windowWidth":900,"showHidden":true}"#,
        )
        .unwrap();
        let loaded = load_settings_from(&dir);
        assert_eq!(
            loaded.hotkey, "Ctrl+Shift+F",
            "加字段不能让旧配置被冲回默认值：{loaded:?}"
        );
        assert!(loaded.show_hidden, "旧配置里的隐藏文件开关必须保留");
        assert_eq!(loaded.theme, "dark", "新字段应补上默认值");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn settings_loaded_from_disk_are_always_in_range() {
        let dir = temp_dir("clamp-settings");
        fs::write(dir.join(SETTINGS_FILE), r#"{"fontSize":200,"bgOpacity":1}"#).unwrap();
        let loaded = load_settings_from(&dir);
        assert_eq!(
            loaded.font_size,
            crate::model::MAX_FONT_SIZE,
            "读盘路径必须夹区间，而不是只依赖调用方自觉：{loaded:?}"
        );
        assert_eq!(loaded.bg_opacity, 20, "面板不透明度不能低到看不见内容");
        fs::remove_dir_all(&dir).ok();
    }
}
