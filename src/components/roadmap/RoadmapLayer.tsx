import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { LearningRoute } from "@/api";
import { Icon } from "@/components/ui/Icon";
import { routeGlyph } from "@/lib/format";
import { pauseAmbientRoadmap, resumeAmbientRoadmap } from "@/hooks/useAmbientRoadmap";
import { ConfirmDialog, useBlindSlats } from "@/components/roadmap/ConfirmDialog";

interface RoadmapLayerProps {
  open: boolean;
  routes: LearningRoute[];
  activeRouteId: string | null;
  completedCount: number;
  nodeCount: number;
  onClose: () => void;
  onSelect: (routeId: string) => void;
  onDelete: (routeId: string) => void;
  onCreate: () => void;
}

/**
 * The acrylic management layer: it unfolds from the header, and its rows arrive
 * as blind slats (`blind-open` / `blind-close`).
 */
export function RoadmapLayer({
  open,
  routes,
  activeRouteId,
  completedCount,
  nodeCount,
  onClose,
  onSelect,
  onDelete,
  onCreate,
}: RoadmapLayerProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const [pendingDelete, setPendingDelete] = useState<LearningRoute | null>(null);
  const closing = useRef(false);

  useBlindSlats(ref, open);

  useLayoutEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open) {
      closing.current = false;
      dialog.classList.remove("is-closing");
      if (!dialog.open) dialog.showModal();
      document.getElementById("root")?.classList.add("roadmap-is-open");
      pauseAmbientRoadmap();
      return;
    }
    if (dialog.open) dismiss();
  }, [open]);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    const onNativeClose = () => {
      document.getElementById("root")?.classList.remove("roadmap-is-open");
      resumeAmbientRoadmap();
      if (open) onClose();
    };
    dialog.addEventListener("close", onNativeClose);
    return () => dialog.removeEventListener("close", onNativeClose);
  }, [open, onClose]);

  const dismiss = () => {
    const dialog = ref.current;
    if (!dialog?.open || closing.current) return;
    closing.current = true;
    dialog.classList.add("is-closing");
    const duration = Number(dialog.dataset.closeDuration ?? 200);
    window.setTimeout(() => {
      closing.current = false;
      dialog.close();
    }, duration);
  };

  const progress = nodeCount > 0 ? Math.round((completedCount / nodeCount) * 100) : 0;
  const active = routes.find((route) => route.id === activeRouteId) ?? routes[0];

  return (
    <>
      <dialog
        ref={ref}
        className="dialog roadmap-layer"
        aria-label="路线管理"
        onCancel={(event) => {
          event.preventDefault();
          dismiss();
        }}
        onClick={(event) => {
          if (event.target === ref.current) dismiss();
        }}
      >
        <div className="layer-heading">
          <button
            className="subject-switch"
            onClick={dismiss}
            aria-label="收起路线管理"
          >
            <span>
              <span className="eyebrow">我的学习</span>
              <span className="subject-name">{active?.title ?? "学习路线"}</span>
            </span>
            <Icon name="chevron" />
          </button>
        </div>
        <div className="layer-scroll">
          <div className="progress-card">
            <div className="row">
              <span>完成进度</span>
              <span className="progress-count">
                <strong>{progress}%</strong>
                <span>
                  {completedCount} / {nodeCount} 个节点
                </span>
              </span>
            </div>
            <div className="bar">
              <i style={{ width: `${progress}%` }} />
            </div>
          </div>
          <div className="layer-label">
            切换路线 <span>{routes.length} 条</span>
          </div>
          <div className="layer-maps">
            {routes.map((route) => {
              const selected = route.id === active?.id;
              const done = route.completedCount;
              const total = route.nodeCount;
              return (
                <div
                  key={route.id}
                  className={`layer-map${selected ? " selected" : ""}`}
                >
                  <button
                    className="route-select"
                    onClick={() => {
                      if (!selected) onSelect(route.id);
                      dismiss();
                    }}
                    data-motion="menu"
                  >
                    <span className="route-glyph">
                      <Icon name={routeGlyph(route.kind)} />
                    </span>
                    <span>
                      <strong>{route.title}</strong>
                      <small>
                        {done} / {total} 个节点
                        {selected ? " · 当前路线" : ""}
                      </small>
                    </span>
                    {selected ? <Icon name="check" /> : <Icon name="chevron" />}
                  </button>
                  {routes.length > 1 ? (
                    <button
                      className="remove-route"
                      aria-label={`移除${route.title}`}
                      onClick={() => setPendingDelete(route)}
                    >
                      <Icon name="trash" />
                    </button>
                  ) : null}
                </div>
              );
            })}
          </div>
        </div>
        <button className="add-unknown" onClick={onCreate} data-motion="menu">
          <span className="unknown-icon">
            <Icon name="plus" />
          </span>
          <span>
            <strong>探索新的未知</strong>
            <small>添加一条新的学习路线</small>
          </span>
          <Icon name="arrow" />
        </button>
      </dialog>
      <ConfirmDialog
        open={pendingDelete !== null}
        title={`移除「${pendingDelete?.title ?? ""}」？`}
        body="这条路线下的节点、资料与学习记录都会一起移除。"
        confirmLabel="移除路线"
        onCancel={() => setPendingDelete(null)}
        onConfirm={() => {
          const route = pendingDelete;
          setPendingDelete(null);
          if (route) onDelete(route.id);
          dismiss();
        }}
      />
    </>
  );
}
