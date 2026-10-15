//! 前后端共享的数据模型。
//!
//! 所有结构体都带 `#[serde(rename_all = "camelCase")]`，
//! 这样 Rust 的 `is_dir` 到前端就是 `isDir`，与 `src/types.ts` 一一对应。

use serde::{Deserialize, Serialize};

/// 文件列表中的一行。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    /// 字节数；目录固定为 0，避免读目录大小带来的卡顿
    pub size: u64,
    /// Unix 毫秒时间戳；取不到为 None
    pub modified: Option<i64>,
    /// 小写扩展名（不含点）；目录或无扩展名为 None
    pub ext: Option<String>,
    /// Windows 隐藏属性，或以 `.` 开头
    pub is_hidden: bool,
}

/// 一次目录列举的结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirListing {
    pub path: String,
    /// 上级目录；已在驱动器根时为 None
    pub parent: Option<String>,
    pub entries: Vec<Entry>,
}

/// 工作空间 = 一个绑定了常用目录的标签页。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: String,
}

/// 字号滑杆的两端，前端 `src/types.ts` 里的滑杆范围必须与这两个值一致。
pub const MIN_FONT_SIZE: u32 = 11;
pub const MAX_FONT_SIZE: u32 = 16;
/// 背景图模糊半径上限（px）
pub const MAX_BG_BLUR: u32 = 40;

/// 用户设置。带 `default`，所以旧版本或残缺的配置文件也能加载。
///
/// 面板尺寸**不在**这里 —— 它在 `window_ctl::PANEL_WIDTH` / `PANEL_HEIGHT`。
/// 早先版本把 `windowWidth` / `windowHeight` 存在设置里，结果磁盘上的旧值会把新默认值
/// 默默压回去。它们已被删除；旧 `settings.json` 里的这两个键会被当作未知字段忽略掉。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub hotzone_enabled: bool,
    pub hotzone_delay_ms: u64,
    pub hide_delay_ms: u64,
    pub hotkey: String,
    pub show_hidden: bool,
    /// `"dark"` 或 `"light"`；其他值一律按 `"dark"` 处理
    pub theme: String,
    /// 界面字体族；空字符串 = 用内置默认字体栈
    pub font_family: String,
    /// 界面基准字号（px），所有尺寸都由它按倍数推导
    pub font_size: u32,
    /// 背景图片的绝对路径；空 = 不使用
    pub background_image: String,
    /// 背景图的模糊半径（px）
    pub bg_blur: u32,
    /// 背景图上暗化层的不透明度（%）
    pub bg_dim: u32,
    /// 面板底色不透明度（%）
    pub bg_opacity: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotzone_enabled: true,
            hotzone_delay_ms: 150,
            hide_delay_ms: 300,
            hotkey: "Ctrl+Alt+Space".into(),
            show_hidden: false,
            theme: "dark".into(),
            font_family: String::new(),
            font_size: 13,
            background_image: String::new(),
            bg_blur: 0,
            bg_dim: 40,
            bg_opacity: 88,
        }
    }
}

impl Settings {
    /// 把可能越界的数值夹回合法区间，非法主题名回落 `"dark"`。
    ///
    /// 必要性的来源：`settings.json` 是明文 JSON，用户可能直接手改，旧版本也可能写进过
    /// 更大的值。越界的模糊半径会做出「面板糊成一团」这类用户无法自救的状态，
    /// 所以读盘时必须夹一次。
    ///
    /// 只夹数值和主题名，**不碰其余字符串** —— 否则用户自定义的字体族名会被改掉。
    pub fn sanitized(mut self) -> Self {
        if self.theme != "light" {
            self.theme = "dark".into();
        }
        self.font_size = self.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
        self.bg_blur = self.bg_blur.min(MAX_BG_BLUR);
        self.bg_dim = self.bg_dim.min(100);
        self.bg_opacity = self.bg_opacity.clamp(20, 100);
        self.hotzone_delay_ms = self.hotzone_delay_ms.min(5_000);
        self.hide_delay_ms = self.hide_delay_ms.clamp(50, 10_000);
        self
    }
}

/// 添加工作空间时可选的预设目录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickLocation {
    pub name: String,
    pub path: String,
    /// home | desktop | download | document | picture | video | drive
    pub kind: String,
}

/// 一次复制/移动的结果汇总。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteReport {
    pub copied: usize,
    /// 因为已经存在同名文件而跳过的名字
    pub renamed: Vec<String>,
    /// 失败项：`路径：原因`
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_defaults_are_filled_from_empty_json() {
        // 旧版本配置文件里没有新字段时，必须能补上默认值而不是报错
        let s: Settings = serde_json::from_str("{}").expect("空对象应能用默认值填充");
        assert_eq!(s, Settings::default(), "空对象应完全等同于默认设置");
        assert!(s.hotzone_enabled, "默认应开启顶部热区");
        assert_eq!(s.hotkey, "Ctrl+Alt+Space", "默认快捷键应是 Ctrl+Alt+Space");
    }

    #[test]
    fn settings_ignore_unknown_fields() {
        // 未来版本加字段后降级回旧版本，仍要能读。
        // 被删掉的 `windowWidth` 也走这条路：旧 settings.json 里有它，不该导致加载失败
        let s: Settings = serde_json::from_str(
            r#"{"hotkey":"Ctrl+Shift+F","unknown_field":1,"windowWidth":900}"#,
        )
        .expect("未知字段应被忽略");
        assert_eq!(s.hotkey, "Ctrl+Shift+F", "配置文件里的快捷键应被读出来");
        assert_eq!(
            s.hide_delay_ms,
            Settings::default().hide_delay_ms,
            "未提供的字段应回落默认值"
        );
    }

    #[test]
    fn workspace_round_trips_through_camel_case() {
        let ws = Workspace {
            id: "a".into(),
            name: "项目".into(),
            path: r"C:\work".into(),
        };
        let json = serde_json::to_string(&ws).unwrap();
        assert!(json.contains("\"name\""), "实际输出：{json}");
        let back: Workspace = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ws, "序列化再反序列化后应完全一致");
    }

    #[test]
    fn legacy_settings_without_appearance_fields_keep_their_values() {
        // 真实场景：用户已经在用 0.1.0，settings.json 里没有外观字段。
        // 加字段绝不能把这些已有配置冲掉；`windowWidth` 是已删字段，必须被忽略而不是报错。
        let s: Settings = serde_json::from_str(
            r#"{"hotzoneEnabled":false,"hotkey":"Ctrl+Shift+F","windowWidth":900,"showHidden":true}"#,
        )
        .expect("旧格式配置应能加载");
        assert!(!s.hotzone_enabled, "旧配置里的开关必须保留");
        assert_eq!(s.hotkey, "Ctrl+Shift+F", "旧配置里的快捷键必须保留");
        assert!(s.show_hidden, "旧配置里的隐藏文件开关必须保留");
        assert_eq!(s.theme, "dark", "缺失的主题字段应回落 dark");
        assert_eq!(s.font_size, 13, "缺失的字号字段应回落 13");
        assert_eq!(s.background_image, "", "缺失的背景图字段应回落空字符串");
    }

    #[test]
    fn out_of_range_appearance_values_are_clamped() {
        // 拆成两句：`let s: Settings = serde_json::from_str(..).expect(..).sanitized()`
        // 编译不过 —— rustc 推不出中间接收者的类型，会对 `.sanitized()` 报 E0282。
        let raw: Settings =
            serde_json::from_str(r#"{"fontSize":999,"bgOpacity":0,"bgBlur":500,"theme":"neon"}"#)
                .expect("越界数值本身不该导致读取失败");
        let s = raw.sanitized();
        assert_eq!(
            s.font_size, MAX_FONT_SIZE,
            "字号越界必须夹到上限，否则行高会撑爆面板"
        );
        assert_eq!(s.bg_opacity, 20, "面板不透明度不能低到看不见内容");
        assert_eq!(s.bg_blur, MAX_BG_BLUR, "模糊半径越界必须夹住");
        assert_eq!(s.theme, "dark", "未知主题名应回落 dark");
    }

    #[test]
    fn sanitizing_defaults_is_a_no_op() {
        // 上面那条测试要能成立，默认值必须在合法区间内 ——
        // 否则「缺失字段回落默认值」和「越界值被夹住」会互相打架。
        assert_eq!(
            Settings::default().sanitized(),
            Settings::default(),
            "默认值必须已经是合法值，否则读盘时会把它改掉"
        );
    }
}
