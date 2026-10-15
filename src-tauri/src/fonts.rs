//! 枚举系统已装字体族，供设置面板的字体下拉使用。
//!
//! 不引任何字体库：直接问注册表要「字体全名」列表，再按 Windows 的命名惯例还原成族名
//! （注册表里存的是 `Arial Bold (TrueType)` 这种全名，而 CSS 要的是 `Arial`）。
//!
//! 走 PowerShell 而不是 `reg.exe`，原因是**编码**：`reg.exe` 按控制台代码页输出，
//! 中文系统上读回 Rust 会得到乱码，而 PowerShell 可以先把 OutputEncoding 顶成 UTF-8。
//!
//! 拿不到结果时回落到一份内置列表，而不是返回空 —— 空下拉框比字体不全更难用。

use std::collections::HashSet;
use std::sync::OnceLock;

/// 查询失败时的兜底字体族，覆盖简中 Windows 的常见预装字体。
const FALLBACK: &[&str] = &[
    "Microsoft YaHei UI",
    "Microsoft YaHei",
    "Segoe UI",
    "等线",
    "宋体",
    "黑体",
    "楷体",
    "Consolas",
    "Arial",
    "Times New Roman",
];

/// 注册表里 `Fonts` 键下的值名就是字体全名，两处（机器级 / 用户级）都要看。
const SCRIPT: &str = "[Console]::OutputEncoding=[Text.Encoding]::UTF8; \
@('HKLM:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts', \
'HKCU:\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts') | \
ForEach-Object { Get-ItemProperty $_ -ErrorAction SilentlyContinue } | \
ForEach-Object { $_.PSObject.Properties | Where-Object { $_.Name -notlike 'PS*' } | \
ForEach-Object { $_.Name } }";

/// 字重 / 字形字样，出现在族名末尾时要剥掉。
///
/// 两条硬约束，违反了都会**静默失败**：
/// 1. **必须全小写 ASCII**。匹配前把族名转成小写再 `ends_with`，写 `"Bold"` 这种带大写的
///    条目永远匹配不上，结果是下拉框里塞满 `Arial Bold`、`Segoe UI Semibold`。
/// 2. **长词在前**。逐个试、命中即停，所以 `bold italic` 必须排在 `bold` 前面。
///
/// 下面两条测试分别守这两个约束。
///
/// 有意**不**收录的词（看着像字重，实际是独立字体族，剥掉会把它并进别的族里）：
/// `roman`（`Times New Roman` 会被切成 `Times New`）、`narrow`（`Arial Narrow`）、
/// `condensed`（`Roboto Condensed`）、`black` / `heavy`（`Arial Black`）。
const STYLE_WORDS: &[&str] = &[
    "bold italic",
    "bold oblique",
    "italic bold",
    "extra light",
    "extra bold",
    "ultra light",
    "ultra bold",
    "demi light",
    "semi light",
    "semi bold",
    "demi bold",
    "extralight",
    "extrabold",
    "ultralight",
    "ultrabold",
    "demilight",
    "semilight",
    "semibold",
    "demibold",
    "regular",
    "oblique",
    "italic",
    "medium",
    "light",
    "bold",
    "thin",
    "book",
];

/// 注册表值名末尾的类型后缀。
const TYPE_SUFFIXES: &[&str] = &[" (TrueType)", " (OpenType)", " (TrueType Collection)"];

fn strip_type_suffix(name: &str) -> &str {
    for tag in TYPE_SUFFIXES {
        if let Some(rest) = name.strip_suffix(tag) {
            return rest;
        }
    }
    name
}

/// 反复剥掉末尾的字重字样，返回族名。
///
/// 会一直剥到剥不动为止，因为注册表里存在 `Arial Bold Italic` 这种叠了两层的写法。
/// `head.is_empty()` 那道判断防止把整个名字剥空（存在就叫 `Light` 的字体）。
fn strip_style_words(name: &str) -> &str {
    let mut current = name;
    loop {
        let lower = current.to_ascii_lowercase();
        let mut stripped: Option<&str> = None;
        for &word in STYLE_WORDS {
            // `to_ascii_lowercase` 不改字节长度，所以能用小写串去判、用原串去切
            if !lower.ends_with(word) {
                continue;
            }
            // 命中的是纯 ASCII 后缀，所以这个切点一定落在字符边界上
            let head = current[..current.len() - word.len()].trim_end();
            if !head.is_empty() {
                stripped = Some(head);
                break;
            }
        }
        match stripped {
            Some(head) => current = head,
            None => return current,
        }
    }
}

/// 把注册表值名列表还原成去重、排序后的字体族名列表。
///
/// 单独抽成纯函数是为了能脱机测试 —— 这段命名解析是全模块最容易出错的地方。
fn families_from_names(names: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut families: Vec<String> = Vec::new();

    for raw in names {
        let name = strip_type_suffix(raw.trim());
        // `微软雅黑 & Microsoft YaHei UI` 这种一条记录里塞两个族名的情况
        for part in name.split(" & ") {
            let family = strip_style_words(part.trim());
            // 以 @ 开头的是竖排变体，不是独立字体族
            if family.is_empty() || family.starts_with('@') {
                continue;
            }
            if seen.insert(family.to_lowercase()) {
                families.push(family.to_string());
            }
        }
    }

    families.sort_by_key(|f| f.to_lowercase());
    families
}

/// 调 PowerShell 读注册表，返回原始字体全名。
fn query_registry() -> Option<Vec<String>> {
    use std::os::windows::process::CommandExt;
    /// 不加这个标志会闪一个黑色控制台窗口 —— 面板里点一下设置就闪窗，很刺眼。
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", SCRIPT])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(
        text.lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect(),
    )
}

/// 系统字体族列表。首次调用会拉起一次 PowerShell（约几百毫秒），之后走缓存。
pub fn list_system_fonts() -> Vec<String> {
    static CACHE: OnceLock<Vec<String>> = OnceLock::new();

    CACHE
        .get_or_init(|| {
            let families = query_registry()
                .map(|raw| families_from_names(&raw))
                .unwrap_or_default();
            if families.is_empty() {
                FALLBACK.iter().map(|f| (*f).to_string()).collect()
            } else {
                families
            }
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(raw: &[&str]) -> Vec<String> {
        raw.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn strips_the_type_suffix() {
        let got = families_from_names(&names(&["Arial (TrueType)", "SimSun (OpenType)"]));
        assert_eq!(got, vec!["Arial", "SimSun"], "类型后缀必须被剥掉：{got:?}");
    }

    #[test]
    fn folds_weight_variants_into_one_family() {
        let got = families_from_names(&names(&[
            "Arial (TrueType)",
            "Arial Bold (TrueType)",
            "Arial Italic (TrueType)",
            "Arial Bold Italic (TrueType)",
        ]));
        assert_eq!(
            got,
            vec!["Arial"],
            "同一个族的各个字重必须归并成一条，否则下拉框里会塞满 Arial Bold 这类条目：{got:?}"
        );
    }

    #[test]
    fn splits_the_ampersand_form() {
        let got = families_from_names(&names(&["Microsoft YaHei & Microsoft YaHei UI (TrueType)"]));
        assert_eq!(
            got,
            vec!["Microsoft YaHei", "Microsoft YaHei UI"],
            "一条注册表记录里的两个族名要拆开：{got:?}"
        );
    }

    #[test]
    fn strips_stacked_style_words() {
        let got = families_from_names(&names(&[
            "DengXian Light (TrueType)",
            "Segoe UI Semibold (TrueType)",
        ]));
        assert_eq!(
            got,
            vec!["DengXian", "Segoe UI"],
            "叠加的字重字样要剥干净：{got:?}"
        );
    }

    #[test]
    fn keeps_names_that_would_be_stripped_to_nothing() {
        // 真存在叫 `Light` / `Book` 的字体。剥到空字符串时必须收手，否则这个族会凭空消失。
        let got = families_from_names(&names(&["Light (TrueType)", "Book (TrueType)"]));
        assert_eq!(
            got,
            vec!["Book", "Light"],
            "整个名字本身就是字重词时不能被剥空：{got:?}"
        );
    }

    #[test]
    fn skips_vertical_variants_and_blanks() {
        let got = families_from_names(&names(&["@SimSun (TrueType)", "   ", "SimSun (TrueType)"]));
        assert_eq!(
            got,
            vec!["SimSun"],
            "竖排变体 @ 前缀与空行都应跳过：{got:?}"
        );
    }

    #[test]
    fn deduplicates_case_insensitively() {
        let got = families_from_names(&names(&["Arial (TrueType)", "ARIAL (TrueType)"]));
        assert_eq!(got, vec!["Arial"], "大小写不同的同一族只保留一条：{got:?}");
    }

    #[test]
    fn keeps_distinct_families_that_look_like_weights() {
        // `Times New Roman` / `Arial Narrow` 是独立字体族，不是字重。
        // 把 `roman` / `narrow` 放进 STYLE_WORDS 会把它们切成 `Times New` / `Arial`。
        let got = families_from_names(&names(&[
            "Times New Roman (TrueType)",
            "Arial Narrow (TrueType)",
        ]));
        assert_eq!(
            got,
            vec!["Arial Narrow", "Times New Roman"],
            "独立字体族不能被当成字重剥掉：{got:?}"
        );
    }

    #[test]
    fn style_words_must_be_lowercase() {
        // 这条测试是有原因的：最初这些条目写成了 `"Bold"` / `"SemiBold"`，而匹配用的是
        // 小写化的族名，于是所有条目都静默失配 —— 下拉框里会冒出 `Arial Bold`。
        for &word in STYLE_WORDS {
            assert_eq!(
                word,
                word.to_ascii_lowercase(),
                "STYLE_WORDS 里的 `{word}` 不是全小写，它将永远匹配不上"
            );
        }
    }

    #[test]
    fn longer_style_words_come_first() {
        // 命中即停，所以任何「以更短条目结尾」的条目都必须排在那个短条目之前。
        // 反例：`bold` 排在 `bold italic` 前面时，`Arial Bold Italic` 会先被切成 `Arial Italic`。
        for (i, &word) in STYLE_WORDS.iter().enumerate() {
            for &later in STYLE_WORDS.iter().skip(i + 1) {
                assert!(
                    !later.ends_with(word),
                    "`{later}` 排在 `{word}` 后面，但以它结尾 —— `{word}` 会先命中，留下残尾"
                );
            }
        }
    }

    #[test]
    fn fallback_list_is_never_empty() {
        assert!(!FALLBACK.is_empty(), "兜底字体列表不能为空");
        assert!(
            families_from_names(&names(&[])).is_empty(),
            "空输入应得到空列表，由调用方决定是否回落兜底"
        );
    }
}
