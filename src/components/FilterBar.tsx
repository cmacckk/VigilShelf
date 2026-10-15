import { useEffect, useRef } from "react";

interface Props {
  value: string;
  onChange: (next: string) => void;
  onClose: () => void;
}

/**
 * Ctrl+F 的筛选条。只做「按名字子串过滤」，不做通配符 / 正则 ——
 * 面板只有 560px 宽、一屏 8 个卡片，语法越少越不会误用。
 *
 * Esc 不在这里处理：面板的全局 keydown 会按「菜单 → 确认框 → 添加 → 设置面板 → 筛选」
 * 的顺序逐层关闭，放在这里会和那一套抢事件。
 */
export function FilterBar({ value, onChange, onClose }: Props) {
  const input = useRef<HTMLInputElement>(null);

  // 打开即聚焦并全选：Ctrl+F 之后可以直接接着打字，不用再点一下输入框
  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);

  return (
    <div className="filterbar">
      <span className="filter-icon" aria-hidden="true">
        ⌕
      </span>
      <input
        ref={input}
        className="filter-input"
        value={value}
        placeholder="按名字筛选（不区分大小写）"
        spellCheck={false}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
        }}
      />
      <button type="button" className="icon-btn" onClick={onClose} title="关闭筛选（Esc）">
        ✕
      </button>
    </div>
  );
}
