import { useRef } from "react";
import type { LearningEvaluation, QuizQuestion, QuizResult } from "@/api";
import { Icon } from "@/components/ui/Icon";
import type { QuizDraft } from "@/stores/learningStore";

const KIND_LABEL: Record<string, string> = {
  single: "单选",
  multi: "多选",
  fill: "填空",
  short: "简答",
  image: "拍照作答",
};

const KIND_HINT: Record<string, string> = {
  single: "选一个你认为对的",
  multi: "可选择多个答案",
  fill: "不用着急，想一想",
  short: "用自己的话写下来",
  image: "在纸上画一画，再拍下来",
};

export interface QuizCardProps {
  question: QuizQuestion;
  draft: QuizDraft;
  /** Set once the learner has asked the model to grade a free-text answer. */
  evaluation: LearningEvaluation | null;
  reviewing: boolean;
  onSelect: (option: string) => void;
  onText: (text: string) => void;
  onPhoto: (dataUrl: string | null) => void;
  onSubmit: () => void;
  onAdvance: () => void;
  onReview: () => void;
  /** Wrong answers are retried, so the card keeps its shake animation state. */
  phase: "idle" | "correct" | "retry" | "pending";
}

/**
 * The understanding check. Single / multi / fill are graded locally by Rust;
 * short and photo answers are recorded and left for the model, so they are
 * labelled "已提交 · 待评阅" instead of being scored by string matching.
 */
export function QuizCard({
  question,
  draft,
  evaluation,
  reviewing,
  onSelect,
  onText,
  onPhoto,
  onSubmit,
  onAdvance,
  onReview,
  phase,
}: QuizCardProps) {
  const fileInput = useRef<HTMLInputElement>(null);
  const result: QuizResult | null = draft.result;
  const objective = question.kind === "single" || question.kind === "multi";
  const answered = Boolean(result);
  const canAdvance = Boolean(result && (result.correct || !result.graded));

  const empty =
    objective ? draft.selected.length === 0 : question.kind === "image" ? !draft.photo : !draft.text.trim();

  const readPhoto = (file: File | undefined) => {
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => onPhoto(typeof reader.result === "string" ? reader.result : null);
    reader.readAsDataURL(file);
  };

  return (
    <section
      className={`quiz-card${phase === "correct" ? " answer-correct" : ""}${
        phase === "retry" ? " answer-retry" : ""
      }`}
    >
      <div className="quiz-tag">
        <Icon name="spark" /> 理解检查 · {KIND_LABEL[question.kind] ?? "练习"}
      </div>
      <h3>{question.prompt}</h3>

      {objective ? (
        <div className="options">
          {question.options.map((option, index) => {
            const selected = draft.selected.includes(option);
            return (
              <button
                key={option}
                className={`option${selected ? " selected" : ""}`}
                onClick={() => onSelect(option)}
                aria-pressed={selected}
              >
                <span>
                  {selected ? "✓" : String.fromCharCode(65 + index)}
                </span>
                {option}
              </button>
            );
          })}
        </div>
      ) : question.kind === "image" ? (
        <label className="upload" htmlFor="answer-photo">
          {draft.photo ? (
            <img src={draft.photo} alt="你上传的作答照片" />
          ) : (
            <Icon name="upload" />
          )}
          {draft.photo ? "点击重新选择图片" : "拍摄或上传你的作答"}
          <input
            hidden
            type="file"
            id="answer-photo"
            accept="image/*"
            capture="environment"
            ref={fileInput}
            onChange={(event) => readPhoto(event.target.files?.[0])}
          />
        </label>
      ) : question.kind === "short" ? (
        <textarea
          id="written-answer"
          className="text-input"
          rows={3}
          placeholder="写下你的理解…"
          value={draft.text}
          disabled={answered}
          onChange={(event) => onText(event.target.value)}
        />
      ) : (
        <input
          id="written-answer"
          className="text-input"
          placeholder="在这里填写答案"
          value={draft.text}
          disabled={answered}
          onChange={(event) => onText(event.target.value)}
        />
      )}

      {result ? (
        <div
          className={`feedback${result.correct ? "" : result.graded ? " wrong" : " pending"}`}
          role="status"
        >
          <span className="feedback-symbol">
            <Icon name={result.correct ? "check" : "review"} />
          </span>
          <span>
            {result.feedback}
            {!result.graded ? (
              <span className="feedback-detail">
                这次作答已经记录，回到路线后可以继续下一站。
              </span>
            ) : null}
            {question.kind === "short" ? (
              evaluation ? (
                <span className="feedback-detail">
                  已评阅 · 正确度 {Math.round(evaluation.correctness * 100)}%
                  {evaluation.misconceptions.length
                    ? ` · 需要留意：${evaluation.misconceptions.join("；")}`
                    : ""}
                </span>
              ) : (
                <button
                  type="button"
                  className="mt-2 block text-xs text-accent"
                  onClick={onReview}
                  disabled={reviewing}
                  data-motion="menu"
                >
                  {reviewing ? "正在评阅这段作答…" : "让 学涯 评阅这段作答"}
                </button>
              )
            ) : null}
            {result.misconceptions.length && !result.correct ? (
              <span className="feedback-detail">
                需要留意：{result.misconceptions.join("；")}
              </span>
            ) : null}
            {result.adaptation && !result.correct ? (
              <span className="feedback-detail">
                已调整路线中的针对性复习。
              </span>
            ) : null}
          </span>
        </div>
      ) : null}

      <div className="quiz-actions">
        <span className="small">
          {answered && !canAdvance
            ? "改一改答案，再提交一次"
            : (KIND_HINT[question.kind] ?? "不用着急，想一想")}
        </span>
        <button
          className="primary"
          disabled={draft.submitting || (!answered && empty)}
          onClick={canAdvance ? onAdvance : onSubmit}
          data-motion={canAdvance ? "own" : undefined}
        >
          {draft.submitting
            ? "正在检查…"
            : canAdvance
              ? "继续学习"
              : "提交答案"}{" "}
          <Icon name="arrow" />
        </button>
      </div>
    </section>
  );
}
