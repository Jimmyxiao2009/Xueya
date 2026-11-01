import type { CSSProperties } from "react";
import type { Lesson, RouteNode } from "@/api";
import { Icon } from "@/components/ui/Icon";
import { hostnameOf } from "@/lib/format";


/** The decorative geometry plate in the lesson header. */
export function GeoPlate() {
  return (
    <svg className="geometry" viewBox="0 0 180 150" aria-hidden="true">
      <path
        d="M15 120 90 23 163 120ZM15 120h148M90 23v110M33 98h114M52 72h76"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.2"
        opacity=".6"
      />
      <ellipse
        cx="90"
        cy="108"
        rx="66"
        ry="23"
        fill="none"
        stroke="currentColor"
        strokeDasharray="4 5"
        opacity=".5"
      />
      <circle cx="90" cy="23" r="5" fill="currentColor" />
      <circle cx="33" cy="98" r="4" fill="currentColor" />
      <circle cx="147" cy="98" r="4" fill="currentColor" />
    </svg>
  );
}

export interface LessonPaneProps {
  lesson: Lesson;
  node: RouteNode | undefined;
  tab: "todo" | "materials";
  style?: CSSProperties;
  className?: string;
}

/**
 * The swipeable lesson body. Both tabs render through this component, and the
 * drag engine mounts a second instance as the incoming page during a swipe.
 */
export function LessonPane({ lesson, node, tab, style, className }: LessonPaneProps) {
  return (
    <div className={className ?? "lesson-content"} style={style}>
      {tab === "todo" ? (
        <TodoPane lesson={lesson} />
      ) : (
        <MaterialsPane lesson={lesson} node={node} />
      )}
    </div>
  );
}

function TodoPane({ lesson }: { lesson: Lesson }) {
  return (
    <>
      <div className="section-label">
        <span>从这几个小问题开始</span>
        <span className="small">学习时逐步点亮</span>
      </div>
      <div className="todos">
        {lesson.steps.map((step, index) => (
          <div
            className="todo"
            key={step.id}
            style={{ "--delay": `${index * 0.08}s` } as CSSProperties}
          >
            <span className="number">{index + 1}</span>
            <div>
              <strong>{step.title}</strong>
              <p>{step.detail}</p>
            </div>
            <span className="duration">{step.durationLabel}</span>
          </div>
        ))}
      </div>
      <p className="prototype-note">
        讲解会在对话中逐步展开；开始学习后，你可以随时请学涯直接讲解。
      </p>
      <p className="swipe-tip">
        向左滑动，翻开本课资料 <Icon name="arrow" />
      </p>
    </>
  );
}

function MaterialsPane({ lesson, node }: { lesson: Lesson; node: RouteNode | undefined }) {
  const citations = lesson.citations;
  return (
    <>
      <div className="section-label">
        <span>本课资料来源</span>
        <span className="small">{citations.length} 项参考</span>
      </div>
      <p className="source-explainer">
        {citations.length
          ? "这些片段来自你添加的资料，讲解与题目都以它们为依据。"
          : "查看原始讲义、课程与参考资料。"}
      </p>
      {citations.length ? (
        citations.map((citation, index) => (
          <article className="source-card" key={`${citation.chunkId}-${index}`}>
            <div className="source-top">
              <span className="source-mark">
                <Icon name={citation.uri ? "book" : "upload"} />
              </span>
              <div>
                <small>
                  {citation.sourceTitle ??
                    (citation.uri ? hostnameOf(citation.uri) : "本地资料")}
                </small>
                <h3>{citation.quote || `片段 ${index + 1}`}</h3>
              </div>
              <span className="source-type">
                {citation.pageNumber
                  ? `第 ${citation.pageNumber} 页`
                  : citation.uri
                    ? "链接"
                    : "片段"}
              </span>
            </div>
            <p>{citation.text}</p>
            {citation.uri ? (
              <div className="source-actions">
                <a href={citation.uri} target="_blank" rel="noopener noreferrer">
                  查看原文 <Icon name="arrow" />
                </a>
              </div>
            ) : null}
          </article>
        ))
      ) : (
        <div className="empty-note">
          还没有参考资料。
          <br />
          可在新增路线时上传文件或添加来源链接。
        </div>
      )}
      {lesson.diagnosticQuestion ? (
        <div className="lesson-brief">
          <div className="section-label">
            <span>带着这个问题开始</span>
          </div>
          <p>{lesson.diagnosticQuestion}</p>
        </div>
      ) : null}
      <p className="prototype-note">
        {node?.generatedBy === "mock" || lesson.generatedBy === "mock"
          ? "资料片段由本地离线检索给出；接入模型后会附带引用编号。"
          : "资料片段来自已索引的来源，可作为讲解与题目的依据。"}
      </p>
    </>
  );
}
