import { useEffect, useRef, useState } from "react";

/** 把 `C:\work\src` 拆成 `C:\` / `work` / `src` 三段可点的面包屑 */
function segments(path: string): { label: string; path: string }[] {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts.map((label, i) => {
    const joined = parts.slice(0, i + 1).join("\\");
    // 驱动器根必须保留反斜杠，否则 `C:` 会被当成「C 盘当前目录」而不是「C 盘根」
    return { label, path: i === 0 ? `${joined}\\` : joined };
  });
}

interface Props {
  path: string;
  onNavigate: (path: string) => void;
}

export function Breadcrumb({ path, onNavigate }: Props) {
  const endRef = useRef<HTMLSpanElement>(null);
  const crumbs = segments(path);
  // 滚动裁剪会把左边被切掉的段落留成孤零零的「›」。只在真的滚动了时候加渐隐，
  // 否则短短路径（「C: > Users」）的盘符也会被平白淡掉。
  const [scrolled, setScrolled] = useState(false);

  // 路径很深时自动滚到最右端，让当前目录始终可见
  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "nearest", inline: "end" });
  }, [path]);

  return (
    <nav
      className={scrolled ? "crumbs scrolled" : "crumbs"}
      onScroll={(e) => setScrolled(e.currentTarget.scrollLeft > 4)}
      aria-label="目录路径"
    >
      {crumbs.map((crumb, i) => (
        <span className="crumb-item" key={crumb.path}>
          {i > 0 && <span className="crumb-sep">›</span>}
          <button
            type="button"
            className={i === crumbs.length - 1 ? "crumb current" : "crumb"}
            onClick={() => onNavigate(crumb.path)}
            title={crumb.path}
          >
            {crumb.label}
          </button>
        </span>
      ))}
      <span ref={endRef} />
    </nav>
  );
}
