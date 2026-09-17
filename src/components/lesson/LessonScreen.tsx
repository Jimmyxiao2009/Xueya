import { useEffect } from "react";
import type { LearningRoute, RouteNode } from "@/api";
import { Icon } from "@/components/ui/Icon";
import { SoftBlur } from "@/components/ui/SoftBlur";
import { GeoPlate, LessonPane } from "@/components/lesson/LessonPane";
import { depthLabel, experienceLabel, padIndex } from "@/lib/format";
import { originLabel, routeMeta } from "@/lib/nodeMeta";
import { useLessonStore } from "@/stores/lessonStore";

interface LessonScreenProps {
  route: LearningRoute;
  node: RouteNode | undefined;
  index: number;
  tab: "todo" | "materials";
  onBack: () => void;
  onTab: (tab: "todo" | "materials") => void;
  onLaunch: (trigger: HTMLElement) => void;
  reduced: boolean;
  launching: boolean;
}

/**
 * The lesson card, opened full-screen by the roadmap transition. The bottom dock
 * launches the learning session and owns its own press → launch animation.
 */
export function LessonScreen({
  route,
  node,
  index,
  tab,
  onBack,
  onTab,
  onLaunch,
  reduced,
  launching,
}: LessonScreenProps) {
  const lesson = useLessonStore((state) => (node ? state.byNode[node.id] : undefined));
  const prepare = useLessonStore((state) => state.prepare);

  useEffect(() => {
    if (node) void prepare(node.id);
  }, [node, prepare]);

  const meta = routeMeta(route.metadataJson);
  const minutes = lesson?.estimatedMinutes || node?.estimatedMinutes || 15;
  const stepCount = lesson?.steps.length ?? 4;
  const conceptCount = Math.max(
    1,
    (node?.description ? 1 : 0) + (lesson?.keyPoints.length ?? 0) || 1,
  );

  return (
    <>
      <main className="page" id="swipe-zone">
        <button className="back" onClick={onBack}>
          <Icon name="back" /> {route.title} / 学习路线
        </button>
        <div className="course-top">
          <div>
            <div className="eyebrow">LESSON {padIndex(index)}</div>
            <h1 className="page-title">{node?.title ?? route.title}</h1>
            <p className="subtext">
              {node?.objective ||
                node?.description ||
                "带着问题出发，把概念变成自己的理解。"}
            </p>
          </div>
          <GeoPlate />
        </div>
        <div className="pills">
          <span className="pill">
            <Icon name="clock" /> 约 {minutes} 分钟
          </span>
          <span className="pill">
            <Icon name="nodes" /> {conceptCount} 个知识点
          </span>
          <span className="pill">
            <Icon name="spark" /> {originLabel(node?.generatedBy)}
          </span>
        </div>
        {meta.learningObjectives.length ? (
          <div className="lesson-goals">
            <div className="section-label">
              <span>本路线的学习目标</span>
              <span className="small">
                {experienceLabel(route.experienceLevel)} · {depthLabel(route.desiredDepth)}
              </span>
            </div>
            <ul className="key-points">
              {meta.learningObjectives.slice(0, 4).map((goal) => (
                <li key={goal}>{goal}</li>
              ))}
            </ul>
          </div>
        ) : null}
        <div className="tabs" role="tablist">
          <button
            role="tab"
            aria-selected={tab === "todo"}
            className={tab === "todo" ? "active" : ""}
            onClick={() => onTab("todo")}
          >
            本课待学 <span className="small">{String(stepCount).padStart(2, "0")}</span>
          </button>
          <button
            role="tab"
            aria-selected={tab === "materials"}
            className={tab === "materials" ? "active" : ""}
            onClick={() => onTab("materials")}
          >
            课程资料 <Icon name="book" />
          </button>
        </div>
        {lesson ? (
          <LessonPane lesson={lesson} node={node} tab={tab} />
        ) : (
          <div className="lesson-content">
            <div className="section-label">
              <span>正在准备这一课</span>
              <span className="small">稍等</span>
            </div>
            <div className="empty-note">Orbit 正在整理这一课的提纲与资料…</div>
          </div>
        )}
      </main>
      <footer className="lesson-dock">
        <SoftBlur edge="bottom" />
        <button
          className={`primary floating${launching && !reduced ? " launching" : ""}`}
          onClick={(event) => onLaunch(event.currentTarget)}
          disabled={launching || !lesson}
          data-motion="own"
        >
          <Icon name="play" /> 开始学习{" "}
          <span style={{ opacity: 0.6, fontWeight: 400 }}>· {minutes} min</span>{" "}
          <Icon name="arrow" />
        </button>
      </footer>
    </>
  );
}
