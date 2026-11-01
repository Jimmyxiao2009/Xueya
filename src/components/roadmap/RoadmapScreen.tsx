import { contentStatusLabel } from "@/lib/nodeMeta";
import { useMemo, type CSSProperties, type Ref } from "react";
import type { LearningRoute, RouteDetail } from "@/api";
import { Icon } from "@/components/ui/Icon";
import { chapterTitle, nodeFace } from "@/lib/format";
import { nodeMeta } from "@/lib/nodeMeta";
import { completedPath, roadGeometry } from "@/lib/roadPath";
import { useThemeStore } from "@/stores/themeStore";
import { LighthouseRoadmapScreen } from "./LighthouseRoadmapScreen";
import { RoadmapNodeIcon } from "@/components/ui/RoadmapNodeIcon";

interface RoadmapScreenProps {
  route: LearningRoute;
  detail?: RouteDetail;
  /** Rendered inside a detached swipe stage instead of the live screen. */
  preview?: boolean;
  /** True when this roadmap arrived by swipe / return: entrance animations are skipped. */
  settled?: boolean;
  style?: CSSProperties;
  scrollRef?: Ref<HTMLElement>;
  onOpenNode?: (nodeId: string, trigger: HTMLElement) => void;
  onScroll?: (top: number) => void;
}

/**
 * The roadmap. This is the product's home: a hand-drawn dotted path with
 * floating nodes, not a dashboard or a list.
 */
export function RoadmapScreen(props: RoadmapScreenProps) {
  const motif = useThemeStore((state) => state.visualMotif);
  return motif === "lighthouse" ? <LighthouseRoadmapScreen {...props} /> : <BuoyRoadmapScreen {...props} />;
}

function BuoyRoadmapScreen({
  route,
  detail,
  preview = false,
  settled = false,
  style,
  scrollRef,
  onOpenNode,
  onScroll,
}: RoadmapScreenProps) {
  const nodes = detail?.nodes ?? [];
  const geometry = useMemo(() => roadGeometry(nodes.length || route.nodeCount), [nodes.length, route.nodeCount]);
  const completedCount = nodes.filter((node) => node.status === "completed").length;
  const currentNodeId =
    route.currentNodeId ?? nodes.find((node) => node.status !== "completed")?.id ?? null;

  return (
    <main
      className={`home-page scroll-pane${settled || preview ? " swipe-settled" : ""}`}
      id={preview ? undefined : "swipe-zone"}
      style={style}
      ref={scrollRef}
      aria-label="学习路线"
      onScroll={
        onScroll
          ? (event) => onScroll((event.currentTarget as HTMLElement).scrollTop)
          : undefined
      }
    >
      <section>
        <div className="roadhead">
          <h2>{chapterTitle(route.kind, 0)}</h2>
          <span>{nodes.length || route.nodeCount} 个节点</span>
        </div>
        <div className="road" style={{ height: geometry.roadHeight }}>
          <svg
            className="path"
            viewBox={geometry.viewBox}
            preserveAspectRatio="none"
            style={{ height: geometry.pathHeight }}
            aria-hidden="true"
          >
            <path d={geometry.full} />
            <path
              className="completed-path"
              d={completedPath(geometry, completedCount)}
              style={{ opacity: completedCount > 0 ? 1 : 0 }}
            />
          </svg>
          {nodes.map((node, index) => {
            const done = node.status === "completed";
            const current = !done && node.id === currentNodeId;
            const meta = nodeMeta(node);
            const face = nodeFace({
              index,
              kind: node.kind,
              done,
              current,
              estimatedMinutes: node.estimatedMinutes,
              conceptCount: meta.concepts.length || 1,
            });
            const variant = done ? "done" : current ? "current" : node.kind === "review" ? "review" : node.kind === "quiz" ? "quiz" : "";
            return (
              <div
                key={node.id}
                className={`stop${index % 2 ? " on-right" : " on-left"}${current ? " active-stop" : ""}`}
                style={{ top: geometry.stopTop(index), "--delay": `${0.08 + index * 0.133}s` } as CSSProperties}
                data-node-id={node.id}
              >
                <button
                  className="node-hit"
                  aria-label={`打开${node.title}`}
                  onClick={(event) => onOpenNode?.(node.id, event.currentTarget)}
                >
                  <RoadmapNodeIcon state={done ? "lit" : current ? "current" : node.kind === "review" ? "review" : "unlit"} variant={variant} icon={face.icon} />
                </button>
                <button
                  className="stop-info"
                  onClick={(event) => onOpenNode?.(node.id, event.currentTarget)}
                >
                  <span className="label-buoy">
                    <span className={`info-face${current ? " now-card" : ""}`}>
                      <span className={`tag ${face.tone}`}>{face.tag}</span>
                      <h3>{node.title}</h3>
                      <p>{contentStatusLabel(node) ?? face.meta}</p>
                      {current ? (
                        <span className="next-arrow">
                          <Icon name="arrow" />
                        </span>
                      ) : null}
                    </span>
                  </span>
                </button>
              </div>
            );
          })}
          <div className="endnote" style={{ top: geometry.endnoteTop }}>
            <Icon name="spark" /> 路线会随你的理解，继续生长
          </div>
        </div>
      </section>
      <div className="home-footer">
        <span>
          学涯
        </span>
        <span>左右滑动，切换学习路线</span>
      </div>
    </main>
  );
}
