import type { Entry } from "../types";

/** 文件类别，决定图标形状与配色 */
export type IconKind =
  | "folder"
  | "image"
  | "video"
  | "audio"
  | "archive"
  | "code"
  | "exe"
  | "pdf"
  | "doc"
  | "sheet"
  | "slide"
  | "text"
  | "file";

const EXT_KIND: Record<string, IconKind> = {};

function register(kind: IconKind, exts: string) {
  for (const ext of exts.split(" ")) EXT_KIND[ext] = kind;
}

register("image", "png jpg jpeg gif bmp webp svg ico heic avif psd ai");
register("video", "mp4 mkv avi mov wmv flv webm m4v mpg mpeg ts");
register("audio", "mp3 wav flac aac ogg m4a wma ape mid");
register(
  "archive",
  "zip rar 7z tar gz bz2 xz iso cab jar war apk dmg",
);
register(
  "code",
  "js jsx ts tsx mjs cjs json html htm css scss less vue svelte py rs go java c h cpp hpp cs php rb kt swift sh ps1 bat cmd toml yaml yml ini xml sql lua dart",
);
register("exe", "exe msi dll sys com scr bat_c md");
register("pdf", "pdf");
register("doc", "doc docx rtf odt wps");
register("sheet", "xls xlsx csv ods et");
register("slide", "ppt pptx odp dps key");
register("text", "txt md log nfo conf cfg");

/** 判断一个文件行该用哪种图标 */
export function iconKind(entry: Entry): IconKind {
  if (entry.isDir) return "folder";
  if (!entry.ext) return "file";
  return EXT_KIND[entry.ext] ?? "file";
}

const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** 人类可读的体积；目录与 0 字节一律显示「—」 */
export function formatSize(bytes: number): string {
  if (bytes <= 0) return "—";
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 ? 0 : value < 10 ? 1 : 0;
  return `${value.toFixed(digits)} ${UNITS[unit]}`;
}

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

/**
 * 当天只显示时分，其余显示日期。
 * 时间戳缺失（权限不足等）时返回空串，不显示「Invalid Date」。
 */
export function formatTime(ms: number | null): string {
  if (!ms) return "";
  const d = new Date(ms);
  if (Number.isNaN(d.getTime())) return "";
  const now = new Date();
  const sameDay =
    d.getFullYear() === now.getFullYear() &&
    d.getMonth() === now.getMonth() &&
    d.getDate() === now.getDate();
  if (sameDay) return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

