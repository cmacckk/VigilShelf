import { useRef } from "react";
import type { CSSProperties } from "react";
import type { Entry } from "../types";
import { FileIcon } from "./FileIcon";
import { formatSize, formatTime, iconKind } from "../lib/format";

interface Props {
  entries: Entry[];
  selected: Set<string>;
  /** 路径 → Windows 外壳给的 PNG data URL。缺项的那些继续用 SVG 兜底 */
  icons: Record<string, string>;
  /** 外部文件正悬停在这个路径的文件夹上 */
  dropTarget: string | null;
  loading: boolean;
  /** 面板里正在显示的目录，用于「拖到自己身上」的兜底判断 */
  cwd: string;
  onSelect: (entry: Entry, e: { ctrlKey: boolean; shiftKey: boolean }) => void;
  onOpen: (entry: Entry) => void;
  onContextMenu: (entry: Entry, x: number, y: number) => void;
  /** 按住鼠标并拖出超过阈值时触发，交给 Rust 走 OLE 拖拽 */
  onDragOut: (entry: Entry) => void;
  /** entries 为空时显示的文案。筛选无结果和「文件夹是空的」是两回事 */
  emptyText?: string;
}

/** 超过这个像素距离才算「拖」，避免点一下就误触发拖拽 */
const DRAG_THRESHOLD = 6;

export function FileList({
  entries,
  selected,
  icons,
  dropTarget,
  loading,
  cwd,
  onSelect,
  onOpen,
  onContextMenu,
  onDragOut,
  emptyText,
}: Props) {
  const press = useRef<{ x: number; y: number; entry: Entry } | null>(null);

  function handleMouseDown(entry: Entry, e: React.MouseEvent) {
    // 阻止文本选中：面板里点选文件不该出现蓝色选区
    e.preventDefault();
    onSelect(entry, { ctrlKey: e.ctrlKey, shiftKey: e.shiftKey });
    if (e.button === 0) press.current = { x: e.clientX, y: e.clientY, entry };
  }

  function handleMouseMove(e: React.MouseEvent) {
    const p = press.current;
    if (!p) return;
    if (e.buttons !== 1) {
      press.current = null;
      return;
    }
    const moved = Math.hypot(e.clientX - p.x, e.clientY - p.y);
    if (moved < DRAG_THRESHOLD) return;
    press.current = null;
    onDragOut(p.entry);
  }

  return (
    <div
      className="filelist"
      onMouseMove={handleMouseMove}
      onMouseUp={() => (press.current = null)}
      onMouseLeave={() => (press.current = null)}
      onContextMenu={(e) => {
        // 空白处右键 = 对当前目录操作
        e.preventDefault();
        onContextMenu(
          { name: cwd, path: cwd, isDir: true, size: 0, modified: null, ext: null, isHidden: false },
          e.clientX,
          e.clientY,
        );
      }}
    >
      {entries.length === 0 && !loading && (
        <p className="empty">
          {emptyText ?? "这个文件夹是空的 —— 把文件拖进来即可复制到这里"}
        </p>
      )}

      {entries.map((entry, index) => {
        const isSelected = selected.has(entry.path);
        const isDropTarget = entry.isDir && dropTarget === entry.path;
        // 卡片只放得下一行小字，所以正文显示大小（文件夹显示「文件夹」）——
        // 修改时间挤不下，挪到悬停提示里，不为了腾地方把信息丢掉
        const size = entry.isDir ? "文件夹" : formatSize(entry.size);
        const time = formatTime(entry.modified);
        return (
          <div
            key={entry.path}
            data-dir={entry.isDir ? entry.path : undefined}
            // SAFETY: CSS 自定义属性不在 CSSProperties 的已知键里，csstype 又不带
            // 索引签名，所以这里只能断言。`--card-index` 会被 App.css 的卡片渐入动画读走，
            // 封顶 11 避免大目录里第 8 个卡片要等一秒才出来。
            style={{ "--card-index": Math.min(index, 11) } as CSSProperties}
            className={[
              "card",
              isSelected ? "card-selected" : "",
              isDropTarget ? "card-drop" : "",
              entry.isHidden ? "card-hidden" : "",
            ]
              .filter(Boolean)
              .join(" ")}
            title={time ? `${entry.path}\n${size}  ·  ${time}` : `${entry.path}\n${size}`}
            onMouseDown={(e) => handleMouseDown(entry, e)}
            onDoubleClick={() => onOpen(entry)}
            onContextMenu={(e) => {
              e.preventDefault();
              e.stopPropagation();
              if (!isSelected) onSelect(entry, { ctrlKey: false, shiftKey: false });
              onContextMenu(entry, e.clientX, e.clientY);
            }}
          >
            <FileIcon kind={iconKind(entry)} src={icons[entry.path]} />
            <span className="card-name">{entry.name}</span>
            <span className="card-meta">{size}</span>
          </div>
        );
      })}
    </div>
  );
}
