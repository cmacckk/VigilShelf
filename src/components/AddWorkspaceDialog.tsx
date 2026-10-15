import { useEffect, useState } from "react";
import type { QuickLocation, Workspace } from "../types";
import { listDir, listQuickLocations } from "../api";

interface Props {
  existing: Workspace[];
  onAdd: (ws: Workspace) => void;
  onClose: () => void;
}

/** 取路径最后一段作为默认空间名；驱动器根（`C:\`）直接用盘符 */
function baseName(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function AddWorkspaceDialog({ existing, onAdd, onClose }: Props) {
  const [quick, setQuick] = useState<QuickLocation[]>([]);
  const [path, setPath] = useState("");
  const [name, setName] = useState("");
  const [children, setChildren] = useState<{ name: string; path: string }[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listQuickLocations().then(setQuick).catch(() => setQuick([]));
  }, []);

  // 路径一变就列出它的子目录，做出一个「就地往下钻」的迷你浏览器
  useEffect(() => {
    let cancelled = false;
    if (!path) {
      setChildren([]);
      return;
    }
    listDir(path, false)
      .then((listing) => {
        if (cancelled) return;
        setError(null);
        setChildren(
          listing.entries.filter((e) => e.isDir).map((e) => ({ name: e.name, path: e.path })),
        );
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setChildren([]);
        setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [path]);

  function pick(next: string) {
    setPath(next);
    setName(baseName(next));
  }

  function submit() {
    const target = path.trim();
    if (!target) {
      setError("请先选择或输入一个目录");
      return;
    }
    const duplicate = existing.some(
      (w) => w.path.toLowerCase() === target.toLowerCase(),
    );
    if (duplicate) {
      setError("这个目录已经是一个工作空间了");
      return;
    }
    onAdd({ id: crypto.randomUUID(), name: name.trim() || baseName(target), path: target });
  }

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section className="panel add-ws">
        <header className="panel-head">
          <h2>添加工作空间</h2>
          <button type="button" className="icon-btn" onClick={onClose} title="关闭">
            ✕
          </button>
        </header>

        <div className="field">
          <span>常用位置</span>
          <div className="chips">
            {quick.map((q) => (
              <button
                key={q.path}
                type="button"
                className={path === q.path ? "chip active" : "chip"}
                onClick={() => pick(q.path)}
                title={q.path}
              >
                {q.name}
              </button>
            ))}
          </div>
        </div>

        <label className="field">
          <span>目录</span>
          <input
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder="粘贴一个绝对路径，或从上方选一个"
            spellCheck={false}
          />
        </label>

        <label className="field">
          <span>名称</span>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="显示在标签上的名字"
          />
        </label>

        <div className="field">
          <span>子文件夹</span>
          <div className="browse">
            {children.length === 0 && <p className="empty small">没有可进入的子文件夹</p>}
            {children.map((c) => (
              <button
                key={c.path}
                type="button"
                className="browse-item"
                onClick={() => pick(c.path)}
                title={c.path}
              >
                {c.name}
              </button>
            ))}
          </div>
        </div>

        {error && <p className="error small">{error}</p>}

        <footer className="panel-foot">
          <button type="button" className="btn ghost" onClick={onClose}>
            取消
          </button>
          <button type="button" className="btn primary" onClick={submit}>
            添加
          </button>
        </footer>
      </section>
    </div>
  );
}
