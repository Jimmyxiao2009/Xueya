import { NavigationAnchor } from "@/components/ui/NavigationAnchor";
import { useEffect, useRef, useState } from "react";
import type { LearningRoute, RouteNode, TutorCitation } from "@/api";
import { Icon } from "@/components/ui/Icon";
import { SoftBlur } from "@/components/ui/SoftBlur";
import { LearningPlan } from "@/components/learning/LearningPlan";
import { QuizCard } from "@/components/learning/QuizCard";
import {
  completedSteps,
  currentQuestion,
  useLearningStore,
} from "@/stores/learningStore";
import { toast } from "@/stores/uiStore";

interface LearningScreenProps {
  route: LearningRoute;
  node: RouteNode | undefined;
  onBackToLesson: () => void;
  onHome: () => void;
  /** Marks the node complete and refreshes the roadmap. */
  onFinish: () => Promise<void>;
  reduced: boolean;
}

const SOURCE_KIND_LABEL: Record<string, string> = {
  file: "本地资料",
  url: "网页资料",
  search_result: "搜索资料",
};

/**
 * A light citation affordance under a grounded tutor message. It shows the
 * source title immediately, and reveals a small excerpt on demand; no internal
 * chunk id, filesystem path, or embedding score is ever rendered.
 */
function CitationChips({ citations }: { citations: TutorCitation[] }) {
  const [openId, setOpenId] = useState<string | null>(null);
  if (citations.length === 0) return null;
  return (
    <div className="mt-2 space-y-1">
      {citations.map((citation) => {
        const open = openId === citation.chunkId;
        return (
          <div key={citation.chunkId}>
            <button
              type="button"
              className="rounded-full border border-subtle bg-paper/70 px-2.5 py-1 text-[11px] text-muted transition hover:text-primary"
              aria-expanded={open}
              aria-label={`查看依据：${citation.sourceTitle}`}
              onClick={() => setOpenId(open ? null : citation.chunkId)}
            >
              依据 · {citation.sourceTitle}
            </button>
            {open ? (
              <div className="mt-1 rounded-lg border border-subtle bg-paper px-3 py-2">
                <strong className="block text-xs text-primary">
                  {citation.sourceTitle}
                </strong>
                <small className="block text-[11px] text-muted">
                  {SOURCE_KIND_LABEL[citation.sourceKind] ?? "资料"}
                  {citation.sourceFilename ? ` · ${citation.sourceFilename}` : ""}
                </small>
                <p className="mt-1 line-clamp-3 text-xs text-muted">
                  {citation.text}
                </p>
                {citation.sourceUri ? (
                  <a
                    className="mt-1 inline-block text-[11px] text-accent"
                    href={citation.sourceUri}
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    打开原来源
                  </a>
                ) : null}
              </div>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

/**
 * The conversation. 学涯 asks, the learner answers, and the plan above lights
 * up one step at a time; a wrong answer never blocks the flow, it changes the
 * roadmap behind it instead.
 */
export function LearningScreen({
  route,
  node,
  onBackToLesson,
  onHome,
  onFinish,
  reduced,
}: LearningScreenProps) {
  const store = useLearningStore();
  const endRef = useRef<HTMLDivElement>(null);
  const [chatDraft, setChatDraft] = useState("");
  const [finishing, setFinishing] = useState(false);
  const [assessmentOpen, setAssessmentOpen] = useState(false);

  const question = currentQuestion(store);
  const completed = completedSteps(store);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [store.messages.length, store.readyToComplete, store.answered]);

  const submitChat = async () => {
    const text = chatDraft;
    if (!text.trim()) return;
    setChatDraft("");
    await store.send(text);
  };

  const finish = async () => {
    if (finishing || !store.readyToComplete) return;
    setFinishing(true);
    await store.finish();
    await onFinish();
    toast("本课已完成，学习路线已更新");
  };

  const phase: "idle" | "correct" | "retry" | "pending" = !store.draft.result
    ? "idle"
    : store.draft.result.correct
      ? "correct"
      : store.draft.result.graded
        ? "retry"
        : "pending";

  return (
    <>
      <main className="chat-page">
        <header className="chat-header">
          <SoftBlur />
          <button className="back" aria-label="返回课程" onClick={onBackToLesson}>
            <Icon name="back" />
          </button>
          <div className="chat-headinfo">
            <h2>{node?.title ?? route.title}</h2>
            <span className="small">学涯 · 和你一起理解</span>
          </div>
          <button className="secondary" onClick={onHome}>
            <Icon name="nodes" /> 路线
          </button>
        </header>

        <LearningPlan
          steps={store.lesson?.steps ?? []}
          completed={completed}
          expanded={store.planExpanded}
          onToggle={store.togglePlan}
          pulse={store.planPulse}
          reduced={reduced}
        />

          <div className="chat-body scroll-pane">
            <div className="assistant-label">
              <NavigationAnchor size={24} /> 学涯
            </div>
            {store.lesson ? (
              <details className="lesson-brief">
                <summary>直接给我讲</summary>
                <p>{store.lesson.explanation}</p>
                {store.lesson.keyPoints.map((point) => (
                  <p key={point}>{point}</p>
                ))}
                {store.lesson.citations.length ? (
                  <div className="formula">
                    依据 {store.lesson.citations.length} 段资料
                    {store.lesson.hasEvidence ? " · 已附引用" : ""}
                  </div>
                ) : null}
              </details>
            ) : null}
          <div id="messages">
            {store.messages.map((message, index) => (
              <div
                key={message.id}
                className={`message ${message.role}${
                  index < store.messages.length - 2 ? " settled-message" : ""
                }`}
              >
                {message.kind === "summary" ? (
                  <div className="answered-summary">
                    <Icon name="check" /> {message.text}
                    <small>{message.note}</small>
                  </div>
                ) : (
                  <>
                    <p>{message.text}</p>
                    {message.note ? (
                      <small className="mt-1 block text-xs text-muted">
                        {message.note}
                      </small>
                    ) : null}
                    {message.citations?.length ? (
                      <CitationChips citations={message.citations} />
                    ) : null}
                  </>
                )}
              </div>
            ))}
          </div>

          {store.readyToComplete ? (
            <div className="completion">
              <h3>
                <Icon name="check" /> 这一站，可以完成了。
              </h3>
              <p>你已经在对话中建立了理解。完成这一站，回到路线看看下一站。</p>
              <div className="conversation-actions">
                <button
                  className="secondary"
                  type="button"
                  onClick={onBackToLesson}
                >
                  继续留在这一节
                </button>
                <button
                  className="primary"
                  onClick={finish}
                  disabled={finishing}
                  data-motion="own"
                >
                  完成这一站 <Icon name="arrow" />
                </button>
              </div>
            </div>
          ) : (
            <div className="conversation-actions">
              <button
                className="secondary"
                type="button"
                onClick={() => setAssessmentOpen((open) => !open)}
              >
                <Icon name="spark" /> {assessmentOpen ? "收起阶段检查" : "阶段检查"}
              </button>
              <button
                className="secondary"
                type="button"
                onClick={onBackToLesson}
              >
                离开本次学习
              </button>
            </div>
          )}
          {!store.readyToComplete && assessmentOpen ? (
            <div id="quiz" className="assessment-panel">
              <div className="assistant-label question-author">
                <Icon name="spark" /> 阶段检查 · 可选
              </div>
              {question ? (
                <QuizCard
                  question={question}
                  draft={store.draft}
                  evaluation={store.evaluation}
                  reviewing={store.reviewing}
                  onSelect={store.selectOption}
                  onText={store.setText}
                  onPhoto={store.setPhoto}
                  onSubmit={store.submit}
                  onAdvance={store.advance}
                  onReview={store.review}
                  phase={phase}
                />
              ) : (
                <div className="empty-note">这一课暂时没有理解检查。</div>
              )}
            </div>
          ) : null}
          <div id="chat-end" ref={endRef} />
        </div>
      </main>

      <div className="composer-wrap">
        <SoftBlur edge="bottom" />
        <form
          className="composer"
          onSubmit={(event) => {
            event.preventDefault();
            void submitChat();
          }}
        >
          <input
            id="chat-input"
            aria-label="发送消息"
            placeholder="有疑问，随时打断我…"
            autoComplete="off"
            value={chatDraft}
            onChange={(event) => setChatDraft(event.target.value)}
          />
          <button className="send" aria-label="发送消息" type="submit">
            <Icon name="send" />
          </button>
        </form>
        <div className="composer-note">
          {store.lesson?.generatedBy === "mock"
            ? "离线示例模型作答 · 配置模型后可获得针对性反馈"
            : "图片仅在本页预览 · 作答会记入学习记录"}
        </div>
      </div>
    </>
  );
}
