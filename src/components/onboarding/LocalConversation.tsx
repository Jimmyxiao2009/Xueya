import { useEffect, useState, type ReactNode } from "react";
import { api, errorOf } from "@/api";
import { useRouteStore } from "@/stores/routeStore";
import { CreateRouteScreen, type RouteCreateFormInput } from "@/screens/CreateRouteScreen";
import { GeneratingScreen } from "@/screens/GeneratingScreen";
import { useJobStore } from "@/stores/jobStore";
import { useAppStore } from "@/stores/appStore";

/** The local core owns generation and persistence, even when the model is remote. */
export function LocalConversation({ children }: { children: ReactNode }) {
  const searchConfigured = useAppStore(s => s.info?.searchConfigured ?? false);
  const refreshInfo = useAppStore(s => s.refresh);
  useEffect(() => { void refreshInfo(); }, [refreshInfo]);
  const [scene, setScene] = useState<"topic" | "context" | "generating" | "ready">("topic");
  const [error, setError] = useState("");
  const [jobId, setJobId] = useState<string | null>(null);
  const [topic, setTopic] = useState("");
  const load = useRouteStore((s) => s.load);
  useEffect(() => {
    let alive = true;
    void load().then(() => { if (alive && useRouteStore.getState().routes.length) setScene("ready"); });
    return () => { alive = false; };
  }, [load]);
  useEffect(() => {
    if (!jobId) return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try {
        const job = await api.jobGet(jobId!);
        if (!alive) return;
        useJobStore.getState().apply(job);
        if (job.status === "failed" || job.status === "cancelled") throw new Error(job.message || "生成未完成，请重试");
        if (job.status === "completed") {
          await load();
          if (alive) { setScene("ready"); setJobId(null); }
          return;
        }
        timer = setTimeout(() => void poll(), 650);
      } catch (failure) {
        if (alive) { setError(errorOf(failure).message); setJobId(null); setScene("context"); }
      }
    }
    void poll();
    return () => { alive = false; clearTimeout(timer); };
  }, [jobId, load]);
  async function generate(input: RouteCreateFormInput) {
    setError(""); setTopic(input.topic); useJobStore.getState().reset(); setScene("generating");
    try {
      const job = await api.routeCreate(input);
      useJobStore.getState().apply(job);
      setJobId(job.id);
    } catch (failure) { setError(errorOf(failure).message); setScene("context"); }
  }
  if (scene === "ready") return children;
  return <>
    {error && <p role="alert">{error}</p>}
    <div style={{ display: scene === "generating" ? "none" : "contents" }}><CreateRouteScreen onBack={() => setScene("ready")} onCreate={generate} submitting={scene === "generating"} searchConfigured={searchConfigured} /></div>
    {scene === "generating" && <GeneratingScreen topic={topic} onCancel={() => setScene("ready")} onReady={() => setScene("ready")} />}
  </>;
}
