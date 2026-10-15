interface Props {
  text: string;
  danger?: boolean;
  confirmLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}

/** 面板内的确认框。不用 window.confirm —— 它在 WebView2 里可能被静默忽略 */
export function Confirm({ text, danger, confirmLabel = "确定", onConfirm, onCancel }: Props) {
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onCancel()}>
      <section className="panel confirm">
        <p className="confirm-text">{text}</p>
        <footer className="panel-foot">
          <button type="button" className="btn ghost" onClick={onCancel}>
            取消
          </button>
          <button
            type="button"
            className={danger ? "btn danger" : "btn primary"}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </footer>
      </section>
    </div>
  );
}
