import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import * as api from "./api";
import type { DirListing, Entry, Settings, Workspace } from "./types";
import { Breadcrumb } from "./components/Breadcrumb";
import { FileList } from "./components/FileList";
import { FilterBar } from "./components/FilterBar";
import { ContextMenu, type MenuItem } from "./components/ContextMenu";
import { SettingsPanel } from "./components/SettingsPanel";
import { AddWorkspaceDialog } from "./components/AddWorkspaceDialog";
import { Confirm } from "./components/Confirm";
import { applyAppearance } from "./lib/appearance";
import "./App.css";

/** 取路径最后一段，用于「移入回收站」这类需要报出名字的确认文案 */
function baseName(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

/** 名字筛选：不区分大小写的子串匹配。空筛选串等于「不过滤」 */
function matchName(name: string, filter: string): boolean {
  return filter === "" || name.toLowerCase().includes(filter.toLowerCase());
}

/**
 * 「看得见」的条目：当前目录里通过筛选的那部分。
 *
 * 界面渲染和点击连选都走这一个函数，两边不会出现「显示的是 12 个、Shift 连选却圈进 31 个」
 * 这类不一致。筛选为空时直接返回原数组，不多分配一次。
 */
function visibleEntries(list: DirListing | null, filter: string): Entry[] {
  const all = list?.entries ?? [];
  return filter === "" ? all : all.filter((e) => matchName(e.name, filter));
}

function reportText(report: api.PasteReport): string {
  const parts = [`完成 ${report.copied} 项`];
  if (report.renamed.length > 0) parts.push(`重名自动改名 ${report.renamed.length} 项`);
  if (report.errors.length > 0) parts.push(`失败 ${report.errors.length} 项`);
  return parts.join("，");
}

/** 把 Tauri 拖放事件里的物理像素坐标换成 CSS 像素，再找它下面的文件夹行 */
function dirAtPoint(position?: { x: number; y: number }): string | null {
  if (!position) return null;
  const dpr = window.devicePixelRatio || 1;
  const el = document.elementFromPoint(position.x / dpr, position.y / dpr);
  return el?.closest("[data-dir]")?.getAttribute("data-dir") ?? null;
}

export default function App() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [activeId, setActiveId] = useState("");
  const [listing, setListing] = useState<DirListing | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  /** 被「剪切」的路径。粘贴时走 transfer_into 的移动模式 */
  const [cut, setCut] = useState<string[]>([]);
  const [clipboardCount, setClipboardCount] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; entry: Entry } | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [adding, setAdding] = useState(false);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [ask, setAsk] = useState<{ text: string; run: () => void } | null>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  /** Ctrl+F 的名字筛选条件。**刻意不落盘** —— 它是临时视图状态，不是用户设置 */
  const [filter, setFilter] = useState("");
  const [filterOpen, setFilterOpen] = useState(false);
  /** 路径 → Windows 外壳给的图标（PNG data URL）。缺项的继续用 SVG 兜底 */
  const [icons, setIcons] = useState<Record<string, string>>({});
  /** 已经向外壳问过图标的路径，避免每次渲染重问一遍 */
  const iconAsked = useRef<Set<string>>(new Set());
  /** true = 正在放退场动画，放完才真的 hide */
  const [closing, setClosing] = useState(false);
  /** 每弹出一次 +1，用来给 `.app` 换 key 重放进场动画 */
  const [runId, setRunId] = useState(0);

  const history = useRef<string[]>([]);
  /** 上一次点击的行，Shift 连选用它当锚点 */
  const anchor = useRef<Entry | null>(null);
  /** 拖放事件的监听只注册一次，用 ref 拿到「最新那次渲染」里的处理函数 */
  const dropHandler = useRef<(paths: string[], dest: string) => void>(() => {});

  /* ------------------------------ 数据加载 ------------------------------ */

  /**
   * 清掉筛选并收起筛选条。
   *
   * 「下次打开界面默认不做筛选」有两层：进程重启靠「不落盘」天然满足；
   * 而面板是被热区反复收起 / 弹出的（React 树不卸载），所以还要在重新弹出时主动清一次 ——
   * 见下面监听 `vigilshelf://panel-shown` 的 effect。
   *
   * 进入另一个目录也会清：筛选条件是按名字匹配的，换了目录后还留着，
   * 只会让新目录看起来「文件全没了」。
   */
  function closeFilter() {
    setFilter("");
    setFilterOpen(false);
  }

  async function navigate(path: string, remember = true) {
    setLoading(true);
    try {
      const next = await api.listDir(path, settings?.showHidden ?? false);
      if (remember && listing && listing.path !== path) history.current.push(listing.path);
      setListing(next);
      setSelected([]);
      closeFilter();
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  async function reload() {
    if (!listing) return;
    try {
      const next = await api.listDir(listing.path, settings?.showHidden ?? false);
      setListing(next);
      setSelected((prev) => prev.filter((p) => next.entries.some((e) => e.path === p)));
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  async function refreshClipboard() {
    try {
      const files = await api.clipboardFiles();
      setClipboardCount(files?.length ?? 0);
    } catch {
      setClipboardCount(0);
    }
  }

  function flash(text: string) {
    setToast(text);
  }

  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => setToast(null), 2400);
    return () => window.clearTimeout(timer);
  }, [toast]);

  // 外观只有一个入口：设置一变就重写 `:root` 的 CSS 变量。
  // 启动时 `settings` 从 null 变成磁盘上的值，这一步把用户上次选的主题/字体/背景图恢复正常。
  useEffect(() => {
    if (settings) applyAppearance(settings);
  }, [settings]);

  // 启动：拉设置与工作空间，并进入第一个空间
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [s, ws] = await Promise.all([api.getSettings(), api.getWorkspaces()]);
        if (cancelled) return;
        setSettings(s);
        setWorkspaces(ws);
        const first = ws[0];
        if (first) {
          setActiveId(first.id);
          setLoading(true);
          const next = await api.listDir(first.path, s.showHidden);
          if (!cancelled) setListing(next);
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    void refreshClipboard();
    return () => {
      cancelled = true;
    };
    // 只在挂载时跑一次：后续刷新都走 reload() / navigate()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 窗口每次被呼出都重新读一遍目录，避免显示过期的文件列表
  useEffect(() => {
    function onFocus() {
      void reload();
      void refreshClipboard();
    }
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  });

  /* ------------------------------- 导航 ------------------------------- */

  async function openEntry(entry: Entry) {
    if (entry.isDir) {
      await navigate(entry.path);
      return;
    }
    try {
      await api.openPath(entry.path);
    } catch (e) {
      setError(String(e));
    }
  }

  async function goUp() {
    if (!listing?.parent) return;
    history.current.push(listing.path);
    await navigate(listing.parent, false);
  }

  async function goBack() {
    const prev = history.current.pop();
    if (prev) await navigate(prev, false);
  }

  /* ------------------------------- 选择 ------------------------------- */

  function selectEntry(entry: Entry, mods: { ctrlKey: boolean; shiftKey: boolean }) {
    // 连选只在**看得见**的条目里算范围，否则筛选状态下 Shift 会把隐藏的文件也圈进来
    const paths = visibleEntries(listing, filter).map((e) => e.path);
    if (mods.shiftKey && anchor.current) {
      const a = paths.indexOf(anchor.current.path);
      const b = paths.indexOf(entry.path);
      if (a >= 0 && b >= 0) {
        const [lo, hi] = a <= b ? [a, b] : [b, a];
        setSelected(paths.slice(lo, hi + 1));
        return;
      }
    }
    if (mods.ctrlKey) {
      setSelected((prev) =>
        prev.includes(entry.path) ? prev.filter((p) => p !== entry.path) : [...prev, entry.path],
      );
      anchor.current = entry;
      return;
    }
    setSelected([entry.path]);
    anchor.current = entry;
  }

  /** 操作目标：右键的行若已在选区内，就对整个选区生效 */
  function targetPaths(entry: Entry): string[] {
    return selected.includes(entry.path) ? selected : [entry.path];
  }

  /* ------------------------------ 右键菜单 ------------------------------ */

  function menuItems(entry: Entry): MenuItem[] {
    const onSelf = listing?.path === entry.path;
    if (onSelf) {
      return [
        { id: "paste", label: "粘贴到此文件夹", disabled: clipboardCount === 0 },
        { id: "reveal", label: "在资源管理器中显示", separatorBefore: true },
        { id: "refresh", label: "刷新" },
      ];
    }
    const many = selected.includes(entry.path) && selected.length > 1;
    const suffix = many ? `（${selected.length} 项）` : "";
    return [
      { id: "open", label: entry.isDir ? "打开文件夹" : "打开" },
      { id: "openwith", label: "用其他程序打开" },
      { id: "reveal", label: "在资源管理器中显示" },
      { id: "copy", label: `复制${suffix}`, separatorBefore: true },
      { id: "cut", label: `剪切${suffix}` },
      { id: "pasteIn", label: "粘贴到此文件夹", disabled: !(entry.isDir && clipboardCount > 0) },
      { id: "copyPath", label: "复制路径", separatorBefore: true },
      { id: "trash", label: `移入回收站${suffix}`, danger: true },
    ];
  }

  async function doPaste(dest: string) {
    setBusy("正在粘贴…");
    try {
      const report =
        cut.length > 0
          ? await api.transferInto(cut, dest, true)
          : await api.pasteInto(dest, false);
      if (cut.length > 0) setCut([]);
      if (report.errors.length > 0) setError(report.errors.join("\n"));
      flash(reportText(report));
      await reload();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
      await refreshClipboard();
    }
  }

  async function doTrash(paths: string[]) {
    const errors: string[] = [];
    for (const path of paths) {
      try {
        await api.trashPath(path);
      } catch (e) {
        errors.push(`${path}：${e}`);
      }
    }
    if (errors.length > 0) setError(errors.join("\n"));
    else flash(`已移入回收站 ${paths.length} 项`);
    setSelected([]);
    await reload();
  }

  useEffect(() => {
    dropHandler.current = async (paths, dest) => {
      setBusy(`正在复制 ${paths.length} 项…`);
      try {
        const report = await api.transferInto(paths, dest, false);
        if (report.errors.length > 0) setError(report.errors.join("\n"));
        flash(reportText(report));
        await reload();
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(null);
      }
    };
  });

  async function handleMenu(id: string) {
    const entry = menu?.entry;
    if (!entry || !listing) return;
    const onSelf = listing.path === entry.path;
    const paths = onSelf ? [] : targetPaths(entry);
    try {
      switch (id) {
        case "open": {
          await openEntry(entry);
          break;
        }
        case "openwith": {
          // 系统对话框会把鼠标挪出面板，期间必须挂起自动收起
          await api.setHotzoneHold(true);
          try {
            await api.showOpenWithDialog(entry.path);
          } finally {
            window.setTimeout(() => void api.setHotzoneHold(false), 1000);
          }
          break;
        }
        case "reveal": {
          await api.revealInExplorer(onSelf ? listing.path : entry.path);
          break;
        }
        case "copy": {
          await api.copyToClipboard(paths);
          setCut([]);
          await refreshClipboard();
          flash(`已复制 ${paths.length} 项`);
          break;
        }
        case "cut": {
          await api.copyToClipboard(paths);
          setCut(paths);
          await refreshClipboard();
          flash(`已剪切 ${paths.length} 项，粘贴后会移走原文件`);
          break;
        }
        case "paste":
        case "pasteIn": {
          await doPaste(onSelf ? listing.path : entry.path);
          break;
        }
        case "copyPath": {
          await navigator.clipboard.writeText(paths.join("\r\n"));
          flash("路径已复制");
          break;
        }
        case "trash": {
          const label = paths.length === 1 ? `「${baseName(paths[0])}」` : `这 ${paths.length} 项`;
          setAsk({
            text: `把${label}移入回收站？之后可以随时从回收站还原。`,
            run: () => void doTrash(paths),
          });
          break;
        }
        case "refresh": {
          await reload();
          break;
        }
      }
    } catch (e) {
      setError(String(e));
    }
  }

  /* -------------------------------- 拖拽 -------------------------------- */

  async function handleDragOut(entry: Entry) {
    const paths = targetPaths(entry);
    const ok = await api.dragOut(paths);
    if (ok) return;
    // 拖拽不可用时回落成「复制路径」，至少不让这次操作白做
    try {
      await navigator.clipboard.writeText(paths.join("\r\n"));
      flash("拖拽不可用，已改为复制路径");
    } catch {
      setError("拖拽失败，且无法写入剪贴板");
    }
  }

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        // SAFETY: Tauri 的 DragDropEvent 联合类型在各小版本间不一致（较早的版本
        // 没有 `enter`，`position` / `paths` 也是可选的）。这里收窄成我们自己处理的
        // 形状，运行时先判 `type` 再取字段，不会读到不存在的属性。
        const payload = event.payload as unknown as {
          type: string;
          position?: { x: number; y: number };
          paths?: string[];
        };
        if (payload.type === "over" || payload.type === "enter") {
          setDropTarget(dirAtPoint(payload.position));
          return;
        }
        if (payload.type === "drop") {
          const dest = dirAtPoint(payload.position);
          setDropTarget(null);
          if (dest && payload.paths?.length) dropHandler.current(payload.paths, dest);
          return;
        }
        setDropTarget(null);
      })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  /* ------------------------------- 键盘 ------------------------------- */

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const target = e.target as HTMLElement | null;
      const typing =
        target?.tagName === "INPUT" || target?.tagName === "TEXTAREA" || !!target?.isContentEditable;

      if (e.key === "Escape") {
        e.preventDefault();
        if (menu) setMenu(null);
        else if (ask) setAsk(null);
        else if (adding) setAdding(false);
        else if (showSettings) setShowSettings(false);
        else if (filterOpen) closeFilter();
        else beginClose();
        return;
      }
      if (typing) return;

      if (e.ctrlKey && e.key.toLowerCase() === "f") {
        e.preventDefault();
        setFilterOpen(true);
        return;
      }

      if (e.key === "Backspace") {
        e.preventDefault();
        void goUp();
      } else if (e.key === "F5") {
        e.preventDefault();
        void reload();
      } else if (e.ctrlKey && e.key.toLowerCase() === "c") {
        if (selected.length === 0) return;
        e.preventDefault();
        void api.copyToClipboard(selected).then(() => {
          setCut([]);
          flash(`已复制 ${selected.length} 项`);
          return refreshClipboard();
        });
      } else if (e.ctrlKey && e.key.toLowerCase() === "x") {
        if (selected.length === 0) return;
        e.preventDefault();
        void api.copyToClipboard(selected).then(() => {
          setCut(selected);
          flash(`已剪切 ${selected.length} 项`);
          return refreshClipboard();
        });
      } else if (e.ctrlKey && e.key.toLowerCase() === "v") {
        if (!listing || clipboardCount === 0) return;
        e.preventDefault();
        void doPaste(listing.path);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // 面板每次重新弹出都回到「不做筛选」。筛过的列表和没筛过的长得一样，
  // 上次的条件留着会让人以为文件丢了。
  useEffect(() => {
    const registered = [
      listen("vigilshelf://panel-shown", () => {
        setFilter("");
        setFilterOpen(false);
        // 上一次的收起请求作废：否则那个退场定时器会把刚弹出来的面板又收回去
        setClosing(false);
        // 换 key 强制重建 `.app` 这一层。CSS 动画只在元素创建时跑一次，
        // 而面板是 hide/show、从不卸载 —— 不重建就没有「滑出」的效果。
        setRunId((n) => n + 1);
      }),
      listen("vigilshelf://panel-hiding", () => setClosing(true)),
    ];
    return () => {
      for (const off of registered) void off.then((fn) => fn());
    };
  }, []);

  // 退场动画放完再真正隐藏。170ms 是 `.app.closing` 的动画时长加一点余量；
  // Rust 侧 request_hide 里有 600ms 兜底，这个值必须比它小。
  useEffect(() => {
    if (!closing) return;
    const timer = window.setTimeout(() => void api.hideWindow(), 170);
    return () => window.clearTimeout(timer);
  }, [closing]);

  /**
   * 所有「收起」都走这里：先让退场动画跑起来，动画结束的定时器再去 hide。
   *
   * 分散地直接调 `api.hideWindow()` 会让「热区收起」和「Esc / ✕ 收起」看起来是两套行为 ——
   * 前者有动画、后者硬切，用户能一眼看出不一致。
   */
  function beginClose() {
    setClosing(true);
  }

  // Windows 外壳图标（和资源管理器同一套图）。取不到的路径不会重试 ——
  // 这是有意的：省掉「每次进同一个坏目录都白白再问一遍」。
  useEffect(() => {
    const wanted = visibleEntries(listing, filter)
      .map((entry) => entry.path)
      .filter((path) => !iconAsked.current.has(path));
    if (wanted.length === 0) return;
    for (const path of wanted) iconAsked.current.add(path);
    void api
      .shellIcons(wanted)
      .then((got) => setIcons((prev) => ({ ...prev, ...got })))
      // 取图标失败不报错：所有卡片都落到 SVG 兜底，界面照常可用
      .catch(() => undefined);
  }, [listing, filter]);

  // 有浮层时挂起自动收起：此时鼠标经常在面板外（菜单、系统对话框）
  const overlayOpen = showSettings || adding || !!ask || !!menu;
  useEffect(() => {
    void api.setHotzoneHold(overlayOpen);
  }, [overlayOpen]);

  /* ------------------------------ 工作空间 ------------------------------ */

  async function persist(next: Workspace[]) {
    setWorkspaces(next);
    try {
      await api.saveWorkspaces(next);
    } catch (e) {
      setError(String(e));
    }
  }

  async function switchWorkspace(ws: Workspace) {
    history.current = [];
    setActiveId(ws.id);
    await navigate(ws.path, false);
  }

  async function addWorkspace(ws: Workspace) {
    setAdding(false);
    history.current = [];
    setActiveId(ws.id);
    setLoading(true);
    try {
      const next = await api.listDir(ws.path, settings?.showHidden ?? false);
      setListing(next);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
    await persist([...workspaces, ws]);
  }

  async function renameWorkspace(ws: Workspace, name: string) {
    setRenamingId(null);
    const trimmed = name.trim();
    if (!trimmed || trimmed === ws.name) return;
    await persist(workspaces.map((w) => (w.id === ws.id ? { ...w, name: trimmed } : w)));
  }

  async function removeWorkspace(ws: Workspace) {
    const next = workspaces.filter((w) => w.id !== ws.id);
    await persist(next);
    if (activeId === ws.id && next[0]) await switchWorkspace(next[0]);
  }

  async function saveSettingsAndApply(next: Settings) {
    try {
      await api.saveSettings(next);
      setSettings(next);
      setShowSettings(false);
      flash("设置已保存");
      await reload();
    } catch (e) {
      setError(String(e));
    }
  }

  /* -------------------------------- 渲染 -------------------------------- */

  const allEntries = listing?.entries ?? [];
  const entries = visibleEntries(listing, filter);
  const selectedEntry = entries.find((e) => e.path === selected[0]);

  return (
    <div className={closing ? "app closing" : "app"} key={runId}>
      {/* 背景图层。放在最前面、用 z-index:-1 沉到所有内容之下；
          背景图必须独立成层，因为 `filter: blur()` 会连子元素一起模糊。 */}
      <div className="backdrop" aria-hidden="true" />
      <header className="topbar">
        <button
          type="button"
          className="icon-btn"
          onClick={() => void goBack()}
          disabled={history.current.length === 0}
          title="后退"
        >
          ‹
        </button>
        <button
          type="button"
          className="icon-btn"
          onClick={() => void goUp()}
          disabled={!listing?.parent}
          title="上一级（Backspace）"
        >
          ↑
        </button>
        {listing ? (
          <Breadcrumb path={listing.path} onNavigate={(p) => void navigate(p)} />
        ) : (
          <span className="crumbs" />
        )}
        <div className="topbar-actions">
          <button
            type="button"
            className="icon-btn"
            onClick={() => {
              if (!settings) return;
              void saveSettingsAndApply({
                ...settings,
                theme: settings.theme === "light" ? "dark" : "light",
              });
            }}
            title={settings?.theme === "light" ? "切换到深色" : "切换到浅色"}
          >
            {settings?.theme === "light" ? "☾" : "☀"}
          </button>
          <button
            type="button"
            className={settings?.showHidden ? "icon-btn on" : "icon-btn"}
            onClick={() => {
              if (!settings) return;
              void saveSettingsAndApply({ ...settings, showHidden: !settings.showHidden });
            }}
            title="显示隐藏文件"
          >
            ◌
          </button>
          <button
            type="button"
            className="icon-btn"
            onClick={() => setShowSettings(true)}
            title="设置"
          >
            ⚙
          </button>
          <button
            type="button"
            className="icon-btn"
            onClick={beginClose}
            title="收起面板（Esc）"
          >
            ✕
          </button>
        </div>
      </header>

      <nav className="tabs">
        {workspaces.map((ws) => (
          <div key={ws.id} className={ws.id === activeId ? "tab active" : "tab"}>
            {renamingId === ws.id ? (
              <input
                className="tab-input"
                defaultValue={ws.name}
                autoFocus
                onBlur={(e) => void renameWorkspace(ws, e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") e.currentTarget.blur();
                  if (e.key === "Escape") setRenamingId(null);
                }}
              />
            ) : (
              <>
                <button
                  type="button"
                  className="tab-name"
                  onClick={() => void switchWorkspace(ws)}
                  onDoubleClick={() => setRenamingId(ws.id)}
                  title={`${ws.name}\n${ws.path}\n双击可重命名`}
                >
                  {ws.name}
                </button>
                <button
                  type="button"
                  className="tab-close"
                  onClick={() => void removeWorkspace(ws)}
                  title="移除这个工作空间（不会删除文件夹）"
                >
                  ✕
                </button>
              </>
            )}
          </div>
        ))}
        <button type="button" className="tab-add" onClick={() => setAdding(true)} title="添加工作空间">
          ＋
        </button>
      </nav>

      {error && (
        <div className="banner">
          <pre>{error}</pre>
          <button type="button" className="icon-btn" onClick={() => setError(null)} title="关闭">
            ✕
          </button>
        </div>
      )}

      {filterOpen && <FilterBar value={filter} onChange={setFilter} onClose={closeFilter} />}

      <FileList
        entries={entries}
        selected={new Set(selected)}
        icons={icons}
        dropTarget={dropTarget}
        loading={loading}
        cwd={listing?.path ?? ""}
        onSelect={selectEntry}
        onOpen={(entry) => void openEntry(entry)}
        onContextMenu={(entry, x, y) => {
          setMenu({ x, y, entry });
          void refreshClipboard();
        }}
        onDragOut={(entry) => void handleDragOut(entry)}
        emptyText={filter === "" ? undefined : `没有匹配「${filter}」的文件`}
      />

      <footer className="statusbar">
        <span>
          {filter === "" ? `${entries.length} 项` : `${entries.length} / ${allEntries.length} 项`}
        </span>
        {selectedEntry && <span className="ellipsis">{selectedEntry.name}</span>}
        {selected.length > 1 && <span>已选 {selected.length} 项</span>}
        <span className="spacer" />
        {cut.length > 0 && <span className="mark">剪切板：{cut.length} 项待移动</span>}
        {busy && <span className="mark">{busy}</span>}
      </footer>

      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={menuItems(menu.entry)}
          onPick={(id) => void handleMenu(id)}
          onClose={() => setMenu(null)}
        />
      )}

      {showSettings && settings && (
        <SettingsPanel
          settings={settings}
          onSave={(next) => void saveSettingsAndApply(next)}
          // 实时预览：滑杆一动就写 CSS 变量，用户不用先点保存才看得到效果。
          // 这里不落盘，取消时 App 会拿 `settings` 把外观复原。
          onPreview={applyAppearance}
          onClose={() => {
            setShowSettings(false);
            applyAppearance(settings);
          }}
        />
      )}

      {adding && (
        <AddWorkspaceDialog
          existing={workspaces}
          onAdd={(ws) => void addWorkspace(ws)}
          onClose={() => setAdding(false)}
        />
      )}

      {ask && (
        <Confirm
          text={ask.text}
          danger
          confirmLabel="移入回收站"
          onConfirm={() => {
            const run = ask.run;
            setAsk(null);
            run();
          }}
          onCancel={() => setAsk(null)}
        />
      )}

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}
