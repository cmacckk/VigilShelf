import { useEffect, useLayoutEffect, useRef, useState } from "react";

export interface MenuItem {
  id: string;
  label: string;
  disabled?: boolean;
  /** 危险操作（移入回收站）用红色提示 */
  danger?: boolean;
  /** 在该项之前画一条分隔线 */
  separatorBefore?: boolean;
}

interface Props {
  x: number;
  y: number;
  items: MenuItem[];
  onPick: (id: string) => void;
  onClose: () => void;
}

export function ContextMenu({ x, y, items, onPick, onClose }: Props) {
  const boxRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });

  // 先按原始坐标渲染，量到真实尺寸后再夹回窗口内，避免菜单被裁掉
  useLayoutEffect(() => {
    const box = boxRef.current;
    if (!box) return;
    const { width, height } = box.getBoundingClientRect();
    const margin = 8;
    setPos({
      x: Math.max(margin, Math.min(x, window.innerWidth - width - margin)),
      y: Math.max(margin, Math.min(y, window.innerHeight - height - margin)),
    });
  }, [x, y, items.length]);

  useEffect(() => {
    function onPointerDown(e: MouseEvent) {
      if (!boxRef.current?.contains(e.target as Node)) onClose();
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    }
    // 捕获阶段监听：面板自身的收起逻辑不该抢在菜单前面执行
    window.addEventListener("mousedown", onPointerDown, true);
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("mousedown", onPointerDown, true);
      window.removeEventListener("keydown", onKey, true);
    };
  }, [onClose]);

  return (
    <div
      ref={boxRef}
      className="menu"
      style={{ left: pos.x, top: pos.y }}
      onContextMenu={(e) => e.preventDefault()}
    >
      {items.map((item) => (
        <div key={item.id}>
          {item.separatorBefore && <div className="menu-sep" />}
          <button
            type="button"
            className={item.danger ? "menu-item danger" : "menu-item"}
            disabled={item.disabled}
            onClick={() => {
              onClose();
              onPick(item.id);
            }}
          >
            {item.label}
          </button>
        </div>
      ))}
    </div>
  );
}
