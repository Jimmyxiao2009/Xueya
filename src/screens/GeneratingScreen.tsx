import { useEffect, useRef, useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { jobPhaseCopy, useJobStore, type JobPhase } from "@/stores/jobStore";

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

  useEffect(() => {
    if (job?.status !== "completed" || readyRef.current) return;
    readyRef.current = true;
    // Let the final phase land before the screen leaves.
    const timer = window.setTimeout(() => {
      setSettled(true);
      onReady(readCourseId(job.resultJson));
    }, 520);
    return () => window.clearTimeout(timer);
  }, [job, onReady]);

  const percent = Math.round(Math.max(0, Math.min(1, settled ? 1 : progress)) * 100);
  const reached = ORDER.indexOf(phase);

  return (
    <main className="generation-page">
      <span className="generation-orbit">
        <Icon name="orbit" />
      </span>
      <h1 className="page-title">
        为「{topic}」
        <br />
        铺好第一段路
      </h1>
      <p id="generation-status">{error ?? message}</p>
      <div className="bar">
        <i style={{ width: `${Math.max(6, percent)}%` }} />
      </div>
      <ol className="generation-steps">
        {ORDER.slice(0, -1).map((entry, index) => (
          <li
            key={entry}
            className={
              index < reached ? "is-done" : index === reached ? "is-current" : "is-pending"
            }
          >
            <span className="generation-dot" aria-hidden="true" />
            {jobPhaseCopy(entry)}
          </li>
        ))}
      </ol>
      {job ? (
        <p className="prototype-note">
          任务 {job.kind} · {percent}%
          {job.message && job.message !== message ? ` · ${job.message}` : ""}
        </p>
      ) : null}
      <button className="secondary" onClick={onCancel}>
        返回路线
      </button>
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
