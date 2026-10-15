import type { IconKind } from "../lib/format";

/** 页形底图：pdf / doc / sheet / slide / text 共用，靠颜色区分 */
const PAGE = ["M6.5 3.5h6.8l4.2 4.2v12.8h-11z", "M13 3.5v4.5h4.5"];
/** 未知类型的兜底图标：带内容线的页 */
const FILE = [...PAGE, "M9 13h6", "M9 16.5h6"];

/** 有专属形状的类别；其余类别走 PAGE / FILE 兜底 */
const GLYPHS: Partial<Record<IconKind, string[]>> = {
  folder: [
    "M3 7.5A2 2 0 0 1 5 5.5h3.4l1.8 2H19a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  ],
  image: [
    "M4 5.5h16v13H4z",
    "M8.5 11a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3",
    "M4 16.5l5-4 4 3 3-2 4 3",
  ],
  video: ["M3.5 6.5h12v11h-12z", "M15.5 11.5l5-3v7l-5-3z"],
  audio: [
    "M9.5 17.5V6l8.5-2v11",
    "M7 20.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5",
    "M16 18.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5",
  ],
  archive: ["M4 4.5h16v4H4z", "M6 8.5v11h12v-11", "M10 12.5h4"],
  code: ["M9 8.5 5 12l4 3.5", "M15 8.5l4 3.5-4 3.5"],
  exe: ["M12 3.5l8.5 4.7v9.6L12 22.5 3.5 17.8V8.2z", "M12 11.5v5"],
  file: FILE,
};

const PAGE_KINDS = new Set<IconKind>(["pdf", "doc", "sheet", "slide", "text"]);

/**
 * 单个文件/文件夹的图标。
 *
 * `src` 是 Windows 外壳给的 PNG data URL（资源管理器同一套图）；拿不到时才退回手画的 SVG。
 * 兜底是必要的：外壳取图会失败（路径太长、网络盘掉线、权限），而一个空位比一个通用图标难看得多。
 */
export function FileIcon({ kind, src }: { kind: IconKind; src?: string }) {
  if (src) {
    return (
      <img className={`icon icon-${kind}`} src={src} alt="" aria-hidden="true" draggable={false} />
    );
  }
  const paths = GLYPHS[kind] ?? (PAGE_KINDS.has(kind) ? PAGE : FILE);
  return (
    <svg
      className={`icon icon-${kind}`}
      viewBox="0 0 24 24"
      width="18"
      height="18"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {paths.map((d) => (
        <path key={d} d={d} />
      ))}
    </svg>
  );
}
