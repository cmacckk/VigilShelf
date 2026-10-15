//! 文件系统操作。
//!
//! 原则：只做用户显式点击的动作；删除一律走回收站（可还原），不做不可逆删除。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::model::{DirListing, Entry, PasteReport};

#[cfg(windows)]
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;

#[cfg(windows)]
fn has_hidden_attribute(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(windows))]
fn has_hidden_attribute(_meta: &fs::Metadata) -> bool {
    false
}

/// 规范化目录路径：去掉尾部反斜杠，但保留 `C:\` 这种驱动器根。
fn normalize_dir(path: &str) -> PathBuf {
    let raw = PathBuf::from(path);
    let trimmed = raw
        .to_string_lossy()
        .trim_end_matches(['\\', '/'])
        .to_string();
    if trimmed.is_empty() {
        raw
    } else if trimmed.ends_with(':') {
        PathBuf::from(format!("{trimmed}\\"))
    } else {
        PathBuf::from(trimmed)
    }
}

/// 列举目录内容。目录排在文件前面，各自按名称不区分大小写排序。
pub fn list_dir_impl(path: &str, show_hidden: bool) -> Result<DirListing, String> {
    let dir = normalize_dir(path);
    let meta = fs::metadata(&dir).map_err(|e| format!("无法访问 {}：{e}", dir.display()))?;
    if !meta.is_dir() {
        return Err(format!("{} 不是文件夹", dir.display()));
    }
    let reader = fs::read_dir(&dir).map_err(|e| format!("无法读取 {}：{e}", dir.display()))?;

    let mut entries = Vec::new();
    for item in reader.flatten() {
        let Ok(meta) = item.metadata() else {
            // 单个条目读不到（权限、符号链接失效）不应让整次列举失败
            continue;
        };
        let entry_path = item.path();
        let name = item.file_name().to_string_lossy().into_owned();
        let hidden = has_hidden_attribute(&meta) || name.starts_with('.');
        if hidden && !show_hidden {
            continue;
        }
        let is_dir = meta.is_dir();
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64);
        let ext = if is_dir {
            None
        } else {
            entry_path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
        };
        entries.push(Entry {
            name,
            path: entry_path.to_string_lossy().into_owned(),
            is_dir,
            size: if is_dir { 0 } else { meta.len() },
            modified,
            ext,
            is_hidden: hidden,
        });
    }

    entries.sort_by(|a, b| {
        // 目录优先；同类按名称不区分大小写；名字完全相同时按原始名，保证排序稳定
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    });

    let parent = dir.parent().map(|p| p.to_string_lossy().into_owned());
    Ok(DirListing {
        path: dir.to_string_lossy().into_owned(),
        parent,
        entries,
    })
}

/// 用系统默认程序打开。
pub fn open_path_impl(path: &str) -> Result<(), String> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|e| format!("无法打开 {path}：{e}"))
}

/// 在资源管理器中定位（打开父目录并选中）。
pub fn reveal_impl(path: &str) -> Result<(), String> {
    tauri_plugin_opener::reveal_item_in_dir(path)
        .map_err(|e| format!("无法在资源管理器中显示 {path}：{e}"))
}

/// 移入回收站。这是本应用唯一的「删除」入口，且可在资源管理器中还原。
pub fn trash_impl(path: &str) -> Result<(), String> {
    trash::delete(path).map_err(|e| format!("无法移入回收站 {path}：{e}"))
}

/// 弹出系统「打开方式」对话框，让用户挑一个程序。
pub fn show_open_with_dialog_impl(path: &str) -> Result<(), String> {
    std::process::Command::new("rundll32.exe")
        .arg("shell32.dll,OpenAs_RunDLL")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法打开「打开方式」对话框：{e}"))
}

/// 目标目录下若已存在同名文件，则追加 ` (2)`、` (3)`…… 直到不冲突。
fn unique_dest(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        // 保留 `.gitignore` 这类无扩展名的点开头文件
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
        _ => (name.to_string(), String::new()),
    };
    for n in 2..10_000 {
        let candidate = dir.join(format!("{stem} ({n}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{stem} (副本){ext}"))
}

fn copy_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_dir() {
        fs::create_dir_all(dst).map_err(|e| format!("无法创建 {}：{e}", dst.display()))?;
        let reader = fs::read_dir(src).map_err(|e| format!("无法读取 {}：{e}", src.display()))?;
        for item in reader.flatten() {
            let child_dst = dst.join(item.file_name());
            copy_recursive(&item.path(), &child_dst)?;
        }
        Ok(())
    } else {
        fs::copy(src, dst)
            .map(|_| ())
            .map_err(|e| format!("无法复制 {}：{e}", src.display()))
    }
}

fn file_name_of(path: &Path) -> Result<String, String> {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| format!("无法识别文件名：{}", path.display()))
}

/// 把一批路径复制或移动到目标目录。
///
/// `move_mode` 为真时，移动 = 复制成功 + 把源移入回收站。
/// 这样「移动」也是可还原的，不会出现复制到一半源就没了的情况。
pub fn transfer_impl(
    sources: &[String],
    dest_dir: &str,
    move_mode: bool,
) -> Result<PasteReport, String> {
    let dest = normalize_dir(dest_dir);
    if !dest.is_dir() {
        return Err(format!("目标不是文件夹：{}", dest.display()));
    }
    let mut report = PasteReport::default();

    for source in sources {
        let src = PathBuf::from(source);
        if !src.exists() {
            report.errors.push(format!("{source}：文件不存在"));
            continue;
        }
        let Ok(name) = file_name_of(&src) else {
            report.errors.push(format!("{source}：无法识别文件名"));
            continue;
        };
        // 防止把目录复制到它自己里面（会造成无限递归）
        if src.is_dir() && dest.starts_with(&src) {
            report
                .errors
                .push(format!("{source}：不能把文件夹移动或复制到它自己内部"));
            continue;
        }

        let target = unique_dest(&dest, &name);
        if target != dest.join(&name) {
            report.renamed.push(name.clone());
        }
        match copy_recursive(&src, &target) {
            Ok(()) => {
                if move_mode && let Err(e) = trash::delete(&src) {
                    report
                        .errors
                        .push(format!("{source}：已复制，但源文件移入回收站失败（{e}）"));
                }
                report.copied += 1;
            }
            Err(e) => report.errors.push(e),
        }
    }
    Ok(report)
}

/// 把一批路径写入系统剪贴板（CF_HDROP），使其能被资源管理器 Ctrl+V 粘贴。
pub fn copy_to_clipboard_impl(paths: &[String]) -> Result<(), String> {
    use clipboard_win::{Clipboard, Setter, formats};
    let _clip = Clipboard::new_attempts(20).map_err(|e| format!("无法打开剪贴板：{e}"))?;
    formats::FileList
        .write_clipboard(paths)
        .map_err(|e| format!("无法写入剪贴板：{e}"))
}

/// 读取剪贴板里的文件列表；剪贴板里不是文件时为 None。
pub fn clipboard_files_impl() -> Result<Option<Vec<String>>, String> {
    use clipboard_win::{Clipboard, Getter, formats};
    let _clip = Clipboard::new_attempts(20).map_err(|e| format!("无法打开剪贴板：{e}"))?;
    if !clipboard_win::is_format_avail(formats::FileList.into()) {
        return Ok(None);
    }
    let mut files: Vec<String> = Vec::new();
    match formats::FileList.read_clipboard(&mut files) {
        Ok(_) => Ok(Some(files)),
        Err(_) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("vigilshelf-fs-{}-{tag}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("测试目录应能创建");
        dir
    }

    fn touch(path: &Path, content: &str) {
        fs::write(path, content).expect("测试文件应能写入");
    }

    #[test]
    fn list_dir_puts_directories_first_then_sorts_case_insensitively() {
        let dir = temp_dir("sort");
        fs::create_dir_all(dir.join("zeta")).unwrap();
        fs::create_dir_all(dir.join("Alpha")).unwrap();
        touch(&dir.join("banana.txt"), "b");
        touch(&dir.join("Apple.txt"), "a");

        let listing = list_dir_impl(&dir.to_string_lossy(), false).unwrap();
        let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["Alpha", "zeta", "Apple.txt", "banana.txt"],
            "目录必须排在文件前面，同类按不区分大小写的名称排序"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_dir_hides_dotfiles_unless_asked() {
        let dir = temp_dir("hidden");
        touch(&dir.join(".env"), "secret");
        touch(&dir.join("visible.txt"), "hi");

        let hidden = list_dir_impl(&dir.to_string_lossy(), false).unwrap();
        let names: Vec<&str> = hidden.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["visible.txt"], "默认不应列出隐藏文件");

        let shown = list_dir_impl(&dir.to_string_lossy(), true).unwrap();
        assert_eq!(shown.entries.len(), 2, "开启显示隐藏文件后应看到 2 项");
        assert!(
            shown
                .entries
                .iter()
                .any(|e| e.is_hidden && e.name == ".env")
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_dir_fills_metadata_for_files() {
        let dir = temp_dir("meta");
        touch(&dir.join("Note.TXT"), "hello");

        let listing = list_dir_impl(&dir.to_string_lossy(), false).unwrap();
        let entry = &listing.entries[0];
        assert_eq!(entry.size, 5, "文件大小应为 5 字节");
        assert_eq!(entry.ext.as_deref(), Some("txt"), "扩展名应统一转成小写");
        assert!(!entry.is_dir, "Note.TXT 是文件，不应被当成目录");
        assert!(entry.modified.is_some(), "修改时间应能取到");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_dir_rejects_a_file_with_an_actionable_message() {
        let dir = temp_dir("not-a-dir");
        let file = dir.join("a.txt");
        touch(&file, "x");

        let err = list_dir_impl(&file.to_string_lossy(), false).unwrap_err();
        assert!(
            err.contains("不是文件夹"),
            "错误信息要能让人看懂，实际得到：{err}"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_dir_reports_missing_path_instead_of_panicking() {
        let missing = format!("C:\\vigilshelf-does-not-exist-{}", uuid::Uuid::new_v4());
        let err = list_dir_impl(&missing, false).unwrap_err();
        assert!(err.contains("无法访问"), "实际得到：{err}");
    }

    #[test]
    fn normalize_dir_keeps_drive_root_and_strips_trailing_slash() {
        assert_eq!(
            normalize_dir("C:\\Users\\").to_string_lossy(),
            "C:\\Users",
            "普通目录要去掉尾部反斜杠"
        );
        let root = normalize_dir("C:\\").to_string_lossy().to_string();
        assert!(
            root.ends_with('\\'),
            "驱动器根必须保留反斜杠，实际得到：{root}"
        );
    }

    #[test]
    fn unique_dest_adds_a_numeric_suffix_on_collision() {
        let dir = temp_dir("unique");
        touch(&dir.join("a.txt"), "1");
        let candidate = unique_dest(&dir, "a.txt");
        assert_eq!(
            candidate.file_name().unwrap().to_string_lossy(),
            "a (2).txt",
            "同名时应追加 (2)，而不是覆盖已有文件"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn transfer_refuses_to_copy_a_directory_into_itself() {
        let dir = temp_dir("self-copy");
        let inner = dir.join("inner");
        fs::create_dir_all(&inner).unwrap();

        let report = transfer_impl(
            &[dir.to_string_lossy().into_owned()],
            &inner.to_string_lossy(),
            false,
        )
        .unwrap();
        assert_eq!(report.copied, 0, "自我复制必须被拒绝");
        assert!(
            report.errors[0].contains("不能把文件夹移动或复制到它自己内部"),
            "实际得到：{:?}",
            report.errors
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn transfer_copies_files_and_reports_missing_sources() {
        let src_dir = temp_dir("copy-src");
        let dst_dir = temp_dir("copy-dst");
        touch(&src_dir.join("one.txt"), "1");
        touch(&src_dir.join("two.txt"), "2");

        let report = transfer_impl(
            &[
                src_dir.join("one.txt").to_string_lossy().into_owned(),
                src_dir.join("two.txt").to_string_lossy().into_owned(),
                src_dir.join("gone.txt").to_string_lossy().into_owned(),
            ],
            &dst_dir.to_string_lossy(),
            false,
        )
        .unwrap();

        assert_eq!(report.copied, 2, "两个存在的文件应被复制");
        assert_eq!(report.errors.len(), 1, "缺失的源文件应被记为一条错误");
        assert!(
            dst_dir.join("one.txt").exists(),
            "存在的源文件应被复制到目标目录"
        );
        assert!(src_dir.join("one.txt").exists(), "复制不应删除源文件");
        fs::remove_dir_all(&src_dir).ok();
        fs::remove_dir_all(&dst_dir).ok();
    }
}
