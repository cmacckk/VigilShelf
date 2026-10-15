/**
 * VigilShelf 前后端共享的数据契约。
 * 所有字段名与 Rust 侧 `#[serde(rename_all = "camelCase")]` 保持一致。
 */

/** 文件列表中的一行 */
export interface Entry {
  name: string;
  path: string;
  isDir: boolean;
  /** 字节。目录固定为 0 */
  size: number;
  /** Unix 毫秒时间戳；取不到时为 null */
  modified: number | null;
  /** 小写扩展名（不含点）；目录或无扩展名为 null */
  ext: string | null;
  /** Windows 隐藏属性 */
  isHidden: boolean;
}

/** 一次目录列举的结果 */
export interface DirListing {
  /** 被列举的目录绝对路径 */
  path: string;
  /** 上级目录；已在驱动器根时为 null */
  parent: string | null;
  entries: Entry[];
}

/** 工作空间 = 一个绑定了常用目录的标签页 */
export interface Workspace {
  id: string;
  name: string;
  path: string;
}

/** 主题名。Rust 侧只认这两个值，其他值一律按 `dark` 处理 */
export type ThemeName = "dark" | "light";

/**
 * 字号滑杆两端。
 *
 * **必须与 `src-tauri/src/model.rs` 的 `MIN_FONT_SIZE` / `MAX_FONT_SIZE` 一致** ——
 * 后端读盘时会按自己的值夹一次，两边不一致就会出现「拖到头了但保存后弹回去」。
 */
export const MIN_FONT_SIZE = 11;
export const MAX_FONT_SIZE = 16;
/** 背景图模糊半径上限（px），同 Rust 侧 `MAX_BG_BLUR` */
export const MAX_BG_BLUR = 40;

/** 用户设置，存 settings.json */
export interface Settings {
  /** 是否启用顶边热区 */
  hotzoneEnabled: boolean;
  /** 光标需在顶边停留多久才呼出（毫秒） */
  hotzoneDelayMs: number;
  /** 光标离开窗口后多久收起（毫秒） */
  hideDelayMs: number;
  /** 全局快捷键，如 "Ctrl+Alt+Space" */
  hotkey: string;
  /** 是否显示隐藏文件 */
  showHidden: boolean;

  /* ------------------------------ 外观 ------------------------------ */

  theme: ThemeName;
  /** 界面字体族；空字符串 = 用内置默认字体栈 */
  fontFamily: string;
  /** 界面基准字号（px），所有尺寸都由它按倍数推导 */
  fontSize: number;
  /** 背景图片的绝对路径；空字符串 = 不使用 */
  backgroundImage: string;
  /** 背景图的模糊半径（px） */
  bgBlur: number;
  /** 背景图的暗化程度（%），只在设了背景图时生效 */
  bgDim: number;
  /** 面板底色不透明度（%），越低越透出背景 */
  bgOpacity: number;
}

/** 「快速位置」= 添加工作空间时可选的预设目录 */
export interface QuickLocation {
  name: string;
  path: string;
  /** home | desktop | download | document | picture | drive */
  kind: string;
}
