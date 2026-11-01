import { NavigationAnchor } from "@/components/ui/NavigationAnchor";
import { useEffect, useRef, useState } from "react";
import { useJobStore, type JobPhase } from "@/stores/jobStore";

interface GeneratingScreenProps {
  topic: string;
  onCancel: () => void;
  onReady: (courseId: string | null) => void;
}

const ORDER: JobPhase[] = [
  "collecting_sources",
  "parsing",
  "building_outline",
  "building_roadmap",
  "ready",
];

/**
 * The generation page. It reads the real job's progress and message, so a slow
 * plan says why it is slow instead of animating a fake bar.
 */
export function GeneratingScreen({ topic, onCancel, onReady }: GeneratingScreenProps) {
  const job = useJobStore((state) => state.job);
  const phase = useJobStore((state) => state.phase);
  const message = useJobStore((state) => state.message);
  const progress = useJobStore((state) => state.progress);
  const error = useJobStore((state) => state.error);
  const [settled, setSettled] = useState(false);
  const readyRef = useRef(false);
  const onReadyRef = useRef(onReady);
  useEffect(() => { onReadyRef.current = onReady; }, [onReady]);

  useEffect(() => {
    if (job?.status !== "completed" || readyRef.current) return;
    // Let the final phase land before the screen leaves.
    const timer = window.setTimeout(() => {
      readyRef.current = true;
      setSettled(true);
      onReadyRef.current(readCourseId(job.resultJson));
    }, 520);
    return () => window.clearTimeout(timer);
  }, [job]);
  const percent = Math.round(Math.max(0, Math.min(1, settled ? 1 : progress)) * 100);
  const reached = ORDER.indexOf(phase);
  const labels = ["整理资料", "读取内容", "规划大纲", "铺好路线"];
  return (
    <main className="generation-page" aria-label="准备学习路线">
      <header className="generation-header"><span className="setup-brand">学涯<span>·</span></span><button onClick={onCancel}>返回路线 <span aria-hidden="true">↗</span></button></header>
      <div className="generation-layout">
        <div className={`generation-visual${error ? " is-paused" : ""}`} aria-hidden="true"><div className="generation-halo" /><NavigationAnchor size={110} /></div>
        <section className="generation-content">
          <p className="generation-caption">每一点好奇，都值得一条清晰的路</p>
          <h1 className="page-title">为「{topic}」<br />铺好第一段路</h1>
          <p id="generation-status" role={error ? "alert" : "status"}>{error ? "这次生成遇到了一点问题，请返回路线后重试。" : message}</p>
          <div className="generation-progress-row"><div className="generation-track" role="progressbar" aria-label="路线生成进度" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} aria-describedby="generation-status"><i style={{ width: `${percent}%` }} /></div><span>{percent}%</span></div>
          <ol className="generation-steps" aria-label="路线准备过程">{ORDER.slice(0,-1).map((entry,index) => <li key={entry} aria-current={index===reached ? "step" : undefined} className={index<reached ? "is-done" : index===reached ? "is-current" : "is-pending"}><span className="generation-dot" aria-hidden="true">{index<reached ? "✓" : ""}</span>{labels[index]}</li>)}</ol>
          <p className="generation-note">路线就绪后即可开始探索，课程内容会陆续准备好。</p>
        </section>
      </div>
      <footer className="generation-footer">学涯 · 让理解慢慢生长</footer>
    </main>
  );
}

function readCourseId(resultJson: string | null | undefined): string | null {
  if (!resultJson) return null;
  try {
    const parsed = JSON.parse(resultJson) as { courseId?: string };
    return parsed.courseId ?? null;
  } catch {
    return null;
  }
}
