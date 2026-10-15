/**
 * 把设置里的外观项写进 `:root` 的 CSS 变量。
 *
 * 集中在这里做，而不是让各组件自己算：字号一变，行高、顶栏、标签栏、状态栏、
 * 列宽都得跟着变，分散写必然对不齐。
 */
import { convertFileSrc } from "@tauri-apps/api/core";
import type { Settings } from "../types";

/**
 * 字体栈的兜底尾巴。
 *
 * 中文字体必须留在栈里：用户选了 `Times New Roman` 这类纯西文字体时，
 * 后面的微软雅黑会接手渲染汉字，否则汉字会变成一个个方框。
 */
const FALLBACK_STACK = '"Microsoft YaHei UI", "Segoe UI", system-ui, sans-serif';

/** 把字体族名拼成可用的 CSS 字体栈 */
export function fontStack(family: string | undefined | null): string {
  // 容忍 undefined：旧版本写下的 settings.json 可能没有这个字段，
  // 一个缺失的字符串不该把整个面板渲染炸掉
  const trimmed = (family ?? "").trim().replace(/"/g, "");
  if (!trimmed) return FALLBACK_STACK;
  // 必须加引号：`Microsoft YaHei UI` 带空格，不加引号 CSS 会把它当成三个族名依次查找
  return `"${trimmed}", ${FALLBACK_STACK}`;
}

/**
 * 由基准字号推导各处尺寸。
 *
 * 用比例而不是写死 px：字号滑到 16 时，若各处尺寸还是按 13px 算出来的值，
 * 文字会被上下裁掉。
 */
function sizeVars(fontSize: number): Record<string, string> {
  const px = (n: number) => `${Math.round(n)}px`;
  return {
    "--fs": `${fontSize}px`,
    "--bar-h": px(fontSize * 3.25),
    "--tab-h": px(fontSize * 2.15),
    "--status-h": px(fontSize * 2),
    // 卡片的行高由 App.css 的 .filelist 从容器高度反推，这里只需行间距与图标尺寸。
    // 字号 13px 时算出 6 / 18px，与 App.css 里 `:root` 的初值一致。
    //
    // 图标倍率（1.35）不能跟着字号线增长：卡片行高几乎不随字号变（它是被容器高度除出来的），
    // 而文字与图标会。倍率取大了，字号调到 16 时图标 + 两行字就会溢出卡片而被裁掉。
    "--card-gap": px(fontSize * 0.46),
    "--icon-size": px(fontSize * 1.35),
  };
}

/** 本地绝对路径 -> 能在 `<img src>` / CSS `url()` 里用的 asset 协议 URL */
export function fileUrl(path: string): string {
  if (!path) return "";
  try {
    return convertFileSrc(path);
  } catch {
    // 路径不合法或 asset 协议没启用时退化成空串，调用方按「没有背景图」处理，
    // 不要把整块面板搞成一张看不到内容的空白
    return "";
  }
}

/** 当前背景图的 asset URL；没设背景图时为空串 */
function backgroundUrl(settings: Settings): string {
  return fileUrl(settings.backgroundImage);
}

/**
 * 浅色主题下背景图遮罩的最低强度（百分比）。
 *
 * 深色主题是白字，背景图再亮也只是对比弱一点；浅色主题是黑字，背景图稍微亮/花
 * 就会把字吃干净。不给地板的话，用户把「提亮」拉到 0 时第一眼看到的是一片读不出来的字，
 * 得先猜到要往右拉才看得见 —— 这是把可用性押在用户会调参上。
 */
export const LIGHT_DIM_FLOOR = 55;

/** 立即把外观写进 CSS 变量。设置面板改滑杆时会直接调它做实时预览 */
export function applyAppearance(settings: Settings): void {
  const root = document.documentElement;
  const url = backgroundUrl(settings);
  // 数字字段一个个兑底值。踩过的坑：draft 里某一项是 undefined 时
  // `fontStack(undefined)` 会直接抛错，而它在 useEffect 里 —— 整个 App 白屏，
  // 报的却是 React 的「An error occurred in the <App> component」，看不出是字段缺失。
  const fontSize = settings.fontSize ?? 13;
  const theme = settings.theme === "light" ? "light" : "dark";
  const dimPercent = settings.backgroundImage
    ? Math.max(settings.bgDim ?? 0, theme === "light" ? LIGHT_DIM_FLOOR : 0)
    : 0;
  const vars: Record<string, string> = {
    ...sizeVars(fontSize),
    "--ui-font": fontStack(settings.fontFamily),
    "--bg-alpha": String((settings.bgOpacity ?? 88) / 100),
    "--bg-blur": `${settings.bgBlur ?? 0}px`,
    "--bg-dim": String(dimPercent / 100),
    "--bg-image": url ? `url("${url}")` : "none",
  };
  for (const [key, value] of Object.entries(vars)) {
    root.style.setProperty(key, value);
  }
  root.dataset.theme = theme;
}
