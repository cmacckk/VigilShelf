import { useEffect, useState } from "react";
import type { QuickLocation } from "../types";
import { listDir, listQuickLocations } from "../api";
import { fileUrl } from "../lib/appearance";

interface Props {
  onPick: (path: string) => void;
  onClose: () => void;
}

/**
 * 能当背景图用的后缀。
 *
 * **必须与 `src-tauri/tauri.conf.json` 的 `assetProtocol.scope` 保持一致** ——
 * scope 里没有的后缀即使在这里能选中，前端也加载不出来，只会得到一张空白背景。
 */
const IMAGE_EXTS = ["jpg", "jpeg", "png", "webp", "bmp", "gif", "avif"];

/** 挑图时的位置排序：图库类目录排前面，视频/文档目录没意义 */
const KIND_ORDER = ["picture", "desktop", "download", "home", "document", "video"];

/** 一次最多渲染多少个文件夹，避免打开一个几万项的目录时卡住 */
const MAX_DIRS = 300;

interface Item {
  name: string;
  path: string;
}

/**
 * 选背景图。
 *
 * 为什么不调系统文件对话框：那需要引 `tauri-plugin-dialog`，会新增一个要联网下载的依赖。
 * 这里复用工作空间对话框那套「快速位置 + 就地往下钻」的迷你浏览器，代价是一致性好、无新依赖。
 */
export function ImagePicker({ onPick, onClose }: Props) {
  const [quick, setQuick] = useState<QuickLocation[]>([]);
  const [cwd, setCwd] = useState("");
  const [parent, setParent] = useState<string | null>(null);
  const [dirs, setDirs] = useState<Item[]>([]);
  const [images, setImages] = useState<Item[]>([]);
  const [picked, setPicked] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    listQuickLocations()
      .then((all) => {
        const ordered = [...all].sort(
          (a, b) => KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind),
        );
        setQuick(ordered);
        setCwd(ordered[0]?.path ?? "");
      })
      .catch(() => setQuick([]));
  }, []);

  useEffect(() => {
    if (!cwd) return;
    let cancelled = false;
    setLoading(true);
    listDir(cwd, false)
      .then((listing) => {
        if (cancelled) return;
        setError(null);
        setParent(listing.parent);
        setDirs(
          listing.entries
            .filter((e) => e.isDir && !e.isHidden)
            .slice(0, MAX_DIRS)
            .map((e) => ({ name: e.name, path: e.path })),
        );
        setImages(
          listing.entries
            .filter((e) => !e.isDir && IMAGE_EXTS.includes((e.ext ?? "").toLowerCase()))
            .map((e) => ({ name: e.name, path: e.path })),
        );
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setDirs([]);
        setImages([]);
        setError(String(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [cwd]);

  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section className="panel image-picker">
        <header className="panel-head">
          <h2>选择背景图片</h2>
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
                className={cwd === q.path ? "chip active" : "chip"}
                onClick={() => setCwd(q.path)}
                title={q.path}
              >
                {q.name}
              </button>
            ))}
          </div>
        </div>

        <div className="field">
          <span>当前文件夹</span>
          <div className="path-bar">
            <button
              type="button"
              className="btn ghost tiny"
              disabled={!parent}
              onClick={() => parent && setCwd(parent)}
            >
              ↑ 上一级
            </button>
            <input
              value={cwd}
              onChange={(e) => setCwd(e.target.value)}
              spellCheck={false}
              placeholder="粘贴一个绝对路径"
            />
          </div>
        </div>

        <div className="field">
          <span>子文件夹{dirs.length > 0 && `（${dirs.length}）`}</span>
          <div className="browse">
            {dirs.length === 0 && <p className="empty small">没有可进入的子文件夹</p>}
            {dirs.map((d) => (
              <button
                key={d.path}
                type="button"
                className="browse-item"
                onClick={() => setCwd(d.path)}
                title={d.path}
              >
                {d.name}
              </button>
            ))}
          </div>
        </div>

        <div className="field">
          <span>图片{images.length > 0 && `（${images.length}）`}</span>
          <div className="browse thumbs">
            {images.length === 0 && !loading && (
              <p className="empty small">这个文件夹里没有支持的图片</p>
            )}
            {images.map((img) => (
              <button
                key={img.path}
                type="button"
                className={picked === img.path ? "thumb active" : "thumb"}
                onClick={() => setPicked(img.path)}
                title={img.path}
              >
                <img src={fileUrl(img.path)} alt="" loading="lazy" />
                <span>{img.name}</span>
              </button>
            ))}
          </div>
        </div>

        {error && <p className="error small">{error}</p>}

        <footer className="panel-foot">
          <button type="button" className="btn ghost" onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className="btn primary"
            disabled={!picked}
            onClick={() => onPick(picked)}
          >
            使用这张图
          </button>
        </footer>
      </section>
    </div>
  );
}
