import { useEffect, useState } from "react";
import type { AdaptationEvent } from "@/api";
import { Icon } from "@/components/ui/Icon";

interface NoticeBannerProps {
  /** Newest adaptation for the active route, if any. */
  adaptation: AdaptationEvent | null;
  /** Stable key so the banner re-appears for a new adaptation. */
  noticeKey: string;
  onDismiss: () => void;
}

const COPY: Record<string, { title: string; body: string; icon: "review" | "quiz" }> = {
  review_inserted: {
    title: "已调整下一次复习",
    body: "针对刚才的薄弱点，再巩固一下。",
    icon: "review",
  },
  node_unlocked: {
    title: "已为你解锁下一站",
    body: "这一段稳了，继续往前走。",
    icon: "quiz",
  },
  difficulty_changed: {
    title: "已调整练习难度",
    body: "接下来的题目会跟着你的状态走。",
    icon: "quiz",
  },
  misconception_detected: {
    title: "记录了一个易错点",
    body: "它会出现在后面的复习里。",
    icon: "review",
  },
  lesson_replanned: {
    title: "已为你重排本课",
    body: "讲解顺序按你的理解调整了。",
    icon: "review",
  },
};

/**
 * The acrylic strip that tells the learner the roadmap just changed under them.
 * It slides away instead of vanishing, so the change is never silent.
 */
export function NoticeBanner({ adaptation, noticeKey, onDismiss }: NoticeBannerProps) {
  const [leaving, setLeaving] = useState(false);
  const [shown, setShown] = useState(noticeKey);

  useEffect(() => {
    setLeaving(false);
    setShown(noticeKey);
  }, [noticeKey]);

  if (!adaptation || shown !== noticeKey) return null;
  const copy = COPY[adaptation.kind];
  if (!copy) return null;

  return (
    <aside
      className={`bottom-notice acrylic${leaving ? " notice-out" : ""}`}
      id="route-notice"
      aria-label="路线更新"
    >
      <span className="notice-icon">
        <Icon name={copy.icon} />
      </span>
      <div>
        <strong>{copy.title}</strong>
        <p>{copy.body}</p>
      </div>
      <button
        aria-label="关闭路线提醒"
        onClick={() => {
          setLeaving(true);
          window.setTimeout(onDismiss, 200);
        }}
      >
        <Icon name="close" />
      </button>
    </aside>
  );
}
