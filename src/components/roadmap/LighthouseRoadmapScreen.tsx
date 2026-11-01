import { contentStatusLabel } from "@/lib/nodeMeta";
import { useMemo, type CSSProperties, type Ref } from "react";
import type { LearningRoute, RouteDetail } from "@/api";
import { Icon } from "@/components/ui/Icon";
import { LighthouseBeacon, type BeaconState } from "@/components/ui/Lighthouse";
import { Atmosphere, FocusGlow } from "@/components/ui/Atmosphere";
import { chapterTitle, nodeFace } from "@/lib/format";
import { nodeMeta } from "@/lib/nodeMeta";
import { roadGeometry } from "@/lib/roadPath";
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
export function LighthouseRoadmapScreen({
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
      className={`home-page lighthouse-home scroll-pane${settled || preview ? " swipe-settled" : ""}`}
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
      <section className="knowledge-sea">
        <Atmosphere density="dense" />
        <div className="roadhead">
          <h2>{chapterTitle(route.kind, 0)}</h2>
          <span>{nodes.length || route.nodeCount} 个节点</span>
        </div>
        <div className="lighthouse-origin" role="img" aria-label="主灯塔，引导知识探索">
          <FocusGlow intensity="strong" />
          <LighthouseBeacon state="lit" primary />
          <div className="origin-caption" aria-hidden="true">
            <span>以理解，点亮未知</span>
                    <small>{completedCount} / {nodes.length || route.nodeCount} 个节点已掌握</small>
          </div>
        </div>
        <div className="road" style={{ height: geometry.roadHeight }}>
          <div className="sea-water" aria-hidden="true" />
          <svg className="origin-path" viewBox="0 0 400 90" preserveAspectRatio="none" aria-hidden="true">
            <path d="M200 0C200 28 68 20 68 90" />
          </svg>
          <svg
            className="path"
            viewBox={geometry.viewBox}
            preserveAspectRatio="none"
            style={{ height: geometry.pathHeight }}
            aria-hidden="true"
          >
            <path d={geometry.full} />
            {geometry.legs.map((d, index) => {
              const from = nodes[index];
              const to = nodes[index + 1];
              const travelled = from?.status === "completed" && to?.status === "completed";
              const approaching = from?.status === "completed" && to?.id === currentNodeId && !travelled;
              return travelled || approaching ? (
                <path key={index} className={travelled ? "completed-path" : "approaching-path"} d={d} />
              ) : null;
            })}
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
            const beaconState: BeaconState = done ? "lit" : current ? "current" : node.kind === "review" ? "review" : "unlit";
            return (
              <div
                key={node.id}
                className={`stop beacon-stop${index % 2 ? " on-right" : " on-left"}${current ? " active-stop" : ""}`}
                style={{ top: geometry.stopTop(index), "--delay": `${0.08 + index * 0.133}s` } as CSSProperties}
                data-node-id={node.id}
                data-beacon-state={beaconState}
              >
                <div className="node-atmosphere" aria-hidden="true" />
                {current || done ? <FocusGlow intensity={current ? "strong" : "soft"} /> : null}
                <button
                  className="node-hit"
                  aria-label={`打开${node.title}`}
                  aria-current={current ? "step" : undefined}
                  onClick={(event) => onOpenNode?.(node.id, event.currentTarget)}
                >
                  <RoadmapNodeIcon state={beaconState} variant={done ? "done" : current ? "current" : node.kind === "review" ? "review" : node.kind === "quiz" ? "quiz" : ""} icon={face.icon} />
                </button>
                <button
                  className="stop-info"
                  onClick={(event) => onOpenNode?.(node.id, event.currentTarget)}
                >
                  <span className="beacon-label">
                    <span className={`info-face${current ? " now-card" : ""}`}>
                      <span className={`tag ${face.tone}`}>
                        <Icon name={done ? "check" : face.icon} />
                        {done ? "已点亮 · 已掌握" : current ? "正在探索" : face.tag}
                      </span>
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
            <span className="horizon-mark" aria-hidden="true" /> 迷雾之外，还有新的理解
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
