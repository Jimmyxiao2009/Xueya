import { useLayoutEffect, useRef } from "react";
import type { LessonStep } from "@/api";
import { Icon } from "@/components/ui/Icon";
import { SoftBlur } from "@/components/ui/SoftBlur";
import { CompletionMark } from "@/components/learning/CompletionMark";

interface LearningPlanProps {
  steps: LessonStep[];
  completed: number;
  expanded: boolean;
  onToggle: () => void;
  /** Bumped when a step completes, so the just-finished row animates. */
  pulse: number;
  reduced: boolean;
}

/**
 * The serial plan above the conversation. It lights up step by step as the
 * learner answers, and collapses into a one-line "正在理解 · …" summary.
 */
export function LearningPlan({
  steps,
  completed,
  expanded,
  onToggle,
  pulse,
  reduced,
}: LearningPlanProps) {
  const panel = useRef<HTMLElement>(null);
  const previous = useRef<{ height: number; rendered: number } | null>(null);

  // Remember the height *before* the toggle so we can animate between the two.
  useLayoutEffect(() => {
    const element = panel.current;
    if (!element) return;
    const height = element.getBoundingClientRect().height;
    const before = previous.current;
    previous.current = { height, rendered: pulse };
    if (!before || reduced || !element.animate) return;
    if (Math.abs(before.height - height) < 1) return;
    element.animate(
      [{ height: `${before.height}px` }, { height: `${height}px` }],
      { duration: 260, easing: "cubic-bezier(.2,.8,.2,1)" },
    );
  }, [expanded, pulse, reduced]);

  const total = steps.length;
  const current = Math.min(completed, Math.max(0, total - 1));

  return (
    <section
      className="chat-progress learning-plan"
      id="learning-plan"
      aria-label="串行学习计划"
      ref={panel}
    >
      <SoftBlur />
      <button
        className="plan-toggle"
        onClick={onToggle}
        aria-expanded={expanded}
        aria-controls="learning-plan-steps"
      >
        <span className="plan-heading">
          <Icon name="nodes" /> 学习计划
        </span>
        <span className="plan-summary">
          已建立理解 {completed} · 继续 {Math.max(0, total - completed)} <Icon name="chevron" />
        </span>
      </button>
      {expanded ? (
        <ol className="plan-steps" id="learning-plan-steps">
          {steps.map((step, index) => {
            const done = index < completed;
            const active = !done && index === current;
            return (
              <li
                key={step.id}
                className={`plan-step ${done ? "is-done" : active ? "is-current" : "is-pending"}`}
                aria-current={active ? "step" : undefined}
              >
                <span className="plan-marker" aria-hidden="true">
                  {done ? (
                    <CompletionMark />
                  ) : active ? (
                    <span className="plan-working" />
                  ) : (
                    <span className="plan-empty" />
                  )}
                </span>
                <span className="plan-task">{step.title}</span>
                <span className="plan-state">
                  {done ? "已建立理解" : active ? "正在理解" : "接下来"}
                </span>
              </li>
            );
          })}
        </ol>
      ) : (
        <div className="plan-collapsed">
          {completed >= total && total > 0 ? (
            "本课计划已完成"
          ) : (
            <>
              <span className="plan-working" />
              <span>正在理解 · {steps[current]?.title ?? "这一课"}</span>
            </>
          )}
        </div>
      )}
    </section>
  );
}
