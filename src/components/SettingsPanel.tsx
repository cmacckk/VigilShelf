import { useEffect, useState } from "react";
import { listSystemFonts } from "../api";
import { fontStack, fileUrl, LIGHT_DIM_FLOOR } from "../lib/appearance";
import { MAX_BG_BLUR, MAX_FONT_SIZE, MIN_FONT_SIZE, type Settings } from "../types";
import { ImagePicker } from "./ImagePicker";

interface Props {
  settings: Settings;
  onSave: (next: Settings) => void;
  /** 每次改动都调一次，让外观立刻生效 —— 不然用户得先保存才能看见效果 */
  onPreview: (next: Settings) => void;
  onClose: () => void;
}

const MODIFIER_KEYS = ["Control", "Alt", "Shift", "Meta"];

/**
 * 把一次按键转成 Tauri 全局快捷键字符串（如 `Ctrl+Alt+Space`）。
 *
 * 只接受「至少一个修饰键 + 一个普通键」，也不接受方向键 / Esc 这类命名键 ——
 * 它们要么会抢走系统按键，要么插件解析不了。
 */
function toAccelerator(e: React.KeyboardEvent): string | null {
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");
  if (mods.length === 0) return null;

  const raw = e.key;
  if (MODIFIER_KEYS.includes(raw)) return null;

  let key: string;
  if (raw === " ") key = "Space";
  else if (raw.length === 1) key = raw.toUpperCase();
  else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(raw)) key = raw;
  else return null;

  return [...mods, key].join("+");
}

/** 文件名，用于「已选背景图」那行的标题 */
function baseName(path: string): string {
  const parts = path.split(/[\\/]+/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function SettingsPanel({ settings, onSave, onPreview, onClose }: Props) {
  const [draft, setDraft] = useState<Settings>(settings);
  const [hint, setHint] = useState<string | null>(null);
  const [fonts, setFonts] = useState<string[]>([]);
  const [fontsLoading, setFontsLoading] = useState(true);
  const [picking, setPicking] = useState(false);

  useEffect(() => setDraft(settings), [settings]);

  // 字体列表要读注册表（后端拉起一次 PowerShell），首次约 1–2 秒。
  // 后端启动时就预热过，这里通常已经命缓存；但仍要区分「还在读」和「真的读不到」，
  // 否则面板刚打开的那一瞬间会误报「读不到系统字体」
  useEffect(() => {
    listSystemFonts()
      .then(setFonts)
      .catch(() => setFonts([]))
      .finally(() => setFontsLoading(false));
  }, []);

  // 任何字段一变就实时预览
  useEffect(() => {
    onPreview(draft);
    // onPreview 是每次渲染新建的函数，放进依赖里会无限循环
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draft]);

  /** 改一个字段 */
  function patch(next: Partial<Settings>) {
    setDraft((prev) => ({ ...prev, ...next }));
  }

  function capture(e: React.KeyboardEvent<HTMLInputElement>) {
    e.preventDefault();
    if (e.key === "Escape") {
      onClose();
      return;
    }
    const acc = toAccelerator(e);
    if (!acc) {
      setHint("请按住至少一个修饰键（Ctrl / Alt / Shift）再按一个字母或数字");
      return;
    }
    setHint(null);
    patch({ hotkey: acc });
  }

  const hasImage = draft.backgroundImage !== "";
  // 背景图上的遮罩层在深色主题盖黑、浅色主题盖白，所以这个滑杆的名字也得跟着变
  const isLight = draft.theme === "light";

  return (
    <>
      <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
        <section className="panel settings">
          <header className="panel-head">
            <h2>设置</h2>
            <button type="button" className="icon-btn" onClick={onClose} title="关闭">
              ✕
            </button>
          </header>

          <h3 className="section">外观</h3>

          <div className="field">
            <span>主题</span>
            <div className="segmented">
              {(["dark", "light"] as const).map((t) => (
                <button
                  key={t}
                  type="button"
                  className={draft.theme === t ? "seg active" : "seg"}
                  onClick={() => patch({ theme: t })}
                >
                  {t === "dark" ? "深色" : "浅色"}
                </button>
              ))}
            </div>
          </div>

          <label className="field">
            <span>字体</span>
            <select
              className="font-select"
              value={draft.fontFamily}
              onChange={(e) => patch({ fontFamily: e.target.value })}
            >
              <option value="">默认（微软雅黑 / Segoe UI）</option>
              {fonts.map((f) => (
                // 每个选项用它自己的字体渲染，挑字体时能直接看出差别
                <option key={f} value={f} style={{ fontFamily: fontStack(f) }}>
                  {f}
                </option>
              ))}
            </select>
            <p className="font-preview" style={{ fontFamily: fontStack(draft.fontFamily) }}>
              VigilShelf 文件夹预览 abcdefg 0123456789
            </p>
            {fontsLoading && <small>正在读取系统字体…</small>}
            {!fontsLoading && fonts.length === 0 && (
              <small>读不到系统字体列表（注册表查询被拦或超时），下拉框里只有默认项</small>
            )}
          </label>

          <label className="field">
            <span>字号：{draft.fontSize} px</span>
            <input
              type="range"
              min={MIN_FONT_SIZE}
              max={MAX_FONT_SIZE}
              step={1}
              value={draft.fontSize}
              onChange={(e) => patch({ fontSize: Number(e.target.value) })}
            />
            <small>行高、顶栏、标签栏都由它按比例推导，改一个数整块界面一起缩放</small>
          </label>

          <h3 className="section">背景图</h3>

          <div className="field">
            <span>图片</span>
            <div className="bg-row">
              <div className="bg-preview">
                {hasImage ? (
                  <img src={fileUrl(draft.backgroundImage)} alt="背景图预览" />
                ) : (
                  <span className="empty small">未设置</span>
                )}
              </div>
              <div className="bg-actions">
                <p className="ellipsis small" title={draft.backgroundImage}>
                  {hasImage ? baseName(draft.backgroundImage) : "没有背景图，面板使用纯色底"}
                </p>
                <div className="row-btns">
                  <button
                    type="button"
                    className="btn ghost tiny"
                    onClick={() => setPicking(true)}
                  >
                    选择图片…
                  </button>
                  {hasImage && (
                    <button
                      type="button"
                      className="btn ghost tiny"
                      onClick={() =>
                        patch({ backgroundImage: "", bgBlur: 0, bgDim: 0 })
                      }
                    >
                      清除
                    </button>
                  )}
                </div>
              </div>
            </div>
          </div>

          {hasImage && (
            <>
              <label className="field">
                <span>模糊：{draft.bgBlur} px</span>
                <input
                  type="range"
                  min={0}
                  max={MAX_BG_BLUR}
                  step={1}
                  value={draft.bgBlur}
                  onChange={(e) => patch({ bgBlur: Number(e.target.value) })}
                />
                <small>模糊掉照片细节，文件名的可读性会明显更好</small>
              </label>

              <label className="field">
                <span>
                  {isLight ? "提亮" : "暗化"}：{draft.bgDim}%
                </span>
                <input
                  type="range"
                  min={0}
                  max={100}
                  step={1}
                  value={draft.bgDim}
                  onChange={(e) => patch({ bgDim: Number(e.target.value) })}
                />
                <small>
                  在背景图上盖一层{isLight ? "白" : "黑"}。图片太花、字看不清时就调大它
                  {isLight && `（浅色主题最低 ${LIGHT_DIM_FLOOR}%，再低黑字会被图片吃掉）`}
                </small>
              </label>
            </>
          )}

          <label className="field">
            <span>面板不透明度：{draft.bgOpacity}%</span>
            <input
              type="range"
              min={20}
              max={100}
              step={1}
              value={draft.bgOpacity}
              onChange={(e) => patch({ bgOpacity: Number(e.target.value) })}
            />
            <small>面板底色本身的浓淡。调低会透出后面的桌面和背景图</small>
          </label>

          <h3 className="section">行为</h3>

          <label className="field switch">
            <input
              type="checkbox"
              checked={draft.hotzoneEnabled}
              onChange={(e) => patch({ hotzoneEnabled: e.target.checked })}
            />
            <span>启用顶边呼出</span>
            <small>鼠标撞到屏幕最上沿并停住时滑出面板；全屏游戏里会自动失效</small>
          </label>

          <label className="field">
            <span>停留多久才呼出：{draft.hotzoneDelayMs} ms</span>
            <input
              type="range"
              min={0}
              max={1000}
              step={50}
              value={draft.hotzoneDelayMs}
              onChange={(e) => patch({ hotzoneDelayMs: Number(e.target.value) })}
            />
            <small>数值越小越灵敏，也越容易在够顶部按钮时误触发</small>
          </label>

          <label className="field">
            <span>鼠标移开后多久收起：{draft.hideDelayMs} ms</span>
            <input
              type="range"
              min={100}
              max={2000}
              step={50}
              value={draft.hideDelayMs}
              onChange={(e) => patch({ hideDelayMs: Number(e.target.value) })}
            />
          </label>

          <label className="field">
            <span>全局快捷键（点击后按下组合键）</span>
            <input
              className="hotkey"
              readOnly
              value={draft.hotkey}
              onKeyDown={capture}
              placeholder="Ctrl+Alt+Space"
            />
            <small>{hint ?? "全屏程序盖住顶边时，用它在屏幕顶部呼出面板"}</small>
          </label>

          <label className="field switch">
            <input
              type="checkbox"
              checked={draft.showHidden}
              onChange={(e) => patch({ showHidden: e.target.checked })}
            />
            <span>显示隐藏文件</span>
            <small>以点开头的文件，以及带 Windows「隐藏」属性的文件</small>
          </label>

          <footer className="panel-foot">
            <button type="button" className="btn ghost" onClick={onClose}>
              取消
            </button>
            <button type="button" className="btn primary" onClick={() => onSave(draft)}>
              保存
            </button>
          </footer>
        </section>
      </div>

      {picking && (
        <ImagePicker
          onPick={(path) => {
            patch({ backgroundImage: path });
            setPicking(false);
          }}
          onClose={() => setPicking(false)}
        />
      )}
    </>
  );
}
