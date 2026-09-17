import { useEffect, useRef, useState } from "react";
import type { LearningRoute, RouteNode } from "@/api";
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

/**
 * The conversation. Orbit asks, the learner answers, and the plan above lights
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

  const question = currentQuestion(store);
  const completed = completedSteps(store);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [store.messages.length, store.complete, store.answered]);

  const submitChat = async () => {
    const text = chatDraft;
    if (!text.trim()) return;
    setChatDraft("");
    await store.send(text);
  };

  const finish = async () => {
    if (finishing) return;
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
            <span className="small">Orbit · 和你一起理解</span>
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
            <Icon name="orbit" /> Orbit
          </div>
          <div className="message">
            {store.lesson ? (
              <>
                <p>{store.lesson.explanation}</p>
                {store.lesson.keyPoints.slice(0, 2).map((point) => (
                  <p key={point}>{point}</p>
                ))}
                {store.lesson.citations.length ? (
                  <div className="formula">
                    依据 {store.lesson.citations.length} 段资料
                    {store.lesson.hasEvidence ? " · 已附引用" : ""}
                  </div>
                ) : null}
              </>
            ) : (
              <p>正在准备这一课…</p>
            )}
          </div>

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
                  </>
                )}
              </div>
            ))}
          </div>

          {store.complete ? (
            <div className="completion">
              <h3>
                <Icon name="check" /> 这一站，完成了。
              </h3>
              <p>你已完成本课学习与作答。回到路线，看看下一站。</p>
              <button
                className="primary"
                onClick={finish}
                disabled={finishing}
                data-motion="own"
              >
                回到我的路线 <Icon name="arrow" />
              </button>
            </div>
          ) : (
            <>
              <div className="assistant-label question-author">
                <Icon name="spark" /> Orbit 向你提问
              </div>
              <div id="quiz">
                {question ? (
                  <QuizCard
                    question={question}
                    draft={store.draft}
                    onSelect={store.selectOption}
                    onText={store.setText}
                    onPhoto={store.setPhoto}
                    onSubmit={store.submit}
                    onAdvance={store.advance}
                    phase={phase}
                  />
                ) : (
                  <div className="empty-note">这一课暂时没有理解检查。</div>
                )}
              </div>
            </>
          )}
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
