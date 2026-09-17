import { useEffect, useRef, useState } from "react";

/**
 * A native `<dialog>` in the sheet style (the small confirm sheet).
 * `open` drives `showModal()` so focus trapping and the top layer come for free.
 */
export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel = "确定",
  cancelLabel = "保留",
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  body?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      className="dialog"
      onCancel={(event) => {
        event.preventDefault();
        onCancel();
      }}
      onClick={(event) => {
        if (event.target === ref.current) onCancel();
      }}
    >
      <h2>{title}</h2>
      {body ? <p className="subtext">{body}</p> : null}
      <div className="actions">
        <button className="secondary" onClick={onCancel} data-motion="menu">
          {cancelLabel}
        </button>
        <button className="primary" onClick={onConfirm} data-motion="menu">
          {confirmLabel}
        </button>
      </div>
    </dialog>
  );
}

/** The blind-slat values the CSS reads: `--slat` in, `--close-slat` out. */
export function useBlindSlats(
  ref: React.RefObject<HTMLElement | null>,
  open: boolean,
): void {
  useEffect(() => {
    const host = ref.current;
    if (!host) return;
    const slats = Array.from(
      host.querySelectorAll<HTMLElement>(
        ".progress-card,.layer-label,.layer-map,.add-unknown",
      ),
    );
    slats.forEach((element, index) => {
      element.classList.add("blind-slat");
      element.style.setProperty("--slat", String(index));
      element.style.setProperty("--close-slat", String(slats.length - 1 - index));
    });
    host.dataset.closeDuration = String(140 + Math.max(0, slats.length - 1) * 12);
    // Re-run whenever the slat count can change (routes added / removed).
  }, [ref, open]);
}

/** Drives `#root`'s `roadmap-is-open` class and the ambient wave pause. */
export function useRoadmapLayerLifecycle(
  open: boolean,
  onClosed?: () => void,
): void {
  const [wasOpen, setWasOpen] = useState(false);
  useEffect(() => {
    if (open) setWasOpen(true);
    if (!open && wasOpen) {
      onClosed?.();
      setWasOpen(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);
}
