/**
 * Rust 命令的 TypeScript 封装。
 * 命名规则：JS 侧 camelCase 参数 -> Rust 侧 snake_case 同名参数。
 */
import { invoke } from "@tauri-apps/api/core";
import type { DirListing, QuickLocation, Settings, Workspace } from "./types";

/* ---------- 文件系统 ---------- */

export const listDir = (path: string, showHidden: boolean) =>
  invoke<DirListing>("list_dir", { path, showHidden });

export const openPath = (path: string) => invoke<void>("open_path", { path });

export const revealInExplorer = (path: string) =>
  invoke<void>("reveal_in_explorer", { path });

export const trashPath = (path: string) => invoke<void>("trash_path", { path });

export const showOpenWithDialog = (path: string) =>
  invoke<void>("show_open_with_dialog", { path });

export const copyToClipboard = (paths: string[]) =>
  invoke<void>("copy_to_clipboard", { paths });

/** 剪贴板里不是文件时返回 null */
export const clipboardFiles = () => invoke<string[] | null>("clipboard_files");

export const pasteInto = (destDir: string, move: boolean) =>
  invoke<PasteReport>("paste_into", { destDir, moveMode: move });

export const transferInto = (sources: string[], destDir: string, move: boolean) =>
  invoke<PasteReport>("transfer_into", { sources, destDir, moveMode: move });

export interface PasteReport {
  copied: number;
  /** 因为目标目录已有同名文件而被自动改名的文件 */
  renamed: string[];
  /** 失败项，格式为「路径：原因」 */
  errors: string[];
}

export const listQuickLocations = () =>
  invoke<QuickLocation[]>("list_quick_locations");

/** 系统已装字体族，供设置面板的字体下拉使用。首次调用约几百毫秒（要读注册表），之后走缓存 */
export const listSystemFonts = () => invoke<string[]>("list_system_fonts");

/* ---------- 工作空间 ---------- */

export const getWorkspaces = () => invoke<Workspace[]>("get_workspaces");

export const saveWorkspaces = (workspaces: Workspace[]) =>
  invoke<void>("save_workspaces", { workspaces });

/* ---------- 设置 ---------- */

export const getSettings = () => invoke<Settings>("get_settings");

export const saveSettings = (settings: Settings) =>
  invoke<void>("save_settings", { settings });

/* ---------- 窗口 ---------- */

export const hideWindow = () => invoke<void>("hide_window");

/**
 * 挂起 / 恢复「鼠标移开自动收起」。
 * 弹出系统对话框（如「打开方式」）时，鼠标会跑到面板外，不挂起就会当场收起。
 */
export const setHotzoneHold = (hold: boolean) =>
  invoke<void>("set_hotzone_hold", { hold });

/* -------------------------------- 图标 -------------------------------- */

/**
 * 批量取 Windows 外壳图标（资源管理器同一套），返回「路径 → PNG data URL」。
 *
 * 取不到的路径**不会出现在返回值里**（而不是给空串），调用方据此继续用手画的 SVG 兜底，
 * 也能顺带把「这个路径没有图标」缓存下来不再重问。
 */
export const shellIcons = (paths: string[]) =>
  invoke<Record<string, string>>("shell_icons", { paths });

/* ---------- 拖拽 ---------- */

/**
 * 把文件从面板里拖到外部程序（记事本、微信、资源管理器）。
 * 失败时不抛错，返回 false，由调用方回落为「复制路径」。
 */
export async function dragOut(paths: string[]): Promise<boolean> {
  try {
    await invoke<void>("drag_out", { paths });
    return true;
  } catch {
    return false;
  }
}
