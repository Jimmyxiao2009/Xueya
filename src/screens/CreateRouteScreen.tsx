import { useEffect, useRef, useState } from "react";
import { api, errorOf, type RouteCreateInput } from "@/api";
import { Icon } from "@/components/ui/Icon";
import { hostnameOf, sizeLabel } from "@/lib/format";
import { useSourceStore } from "@/stores/sourceStore";

export interface RouteCreateFormInput extends RouteCreateInput { collectWeb?: boolean }
interface Props { onBack: () => void; onCreate: (input: RouteCreateFormInput) => void | Promise<void>; submitting: boolean; searchConfigured: boolean }
const LEVELS = [
  { value: "beginner", label: "从未接触", detail: "从最基本的概念开始，慢慢建立直觉。" },
  { value: "some_experience", label: "接触过一些", detail: "补齐知识之间的连接，把零散理解串起来。" },
  { value: "advanced", label: "已有基础", detail: "略过熟悉的基础，直接走向更有挑战的问题。" },
];
const DEPTHS = [
  { value: "overview", label: "了解全貌", detail: "先看清整体，掌握重要概念与基本联系。" },
  { value: "practical", label: "能够应用", detail: "通过例子和练习，把理解变成能用的知识。" },
  { value: "deep", label: "深入掌握", detail: "追问原理与边界，建立更完整的知识体系。" },
];
const IDEAS = ["线性代数", "Python 入门", "摄影构图", "概率与统计", "音乐理论", "世界历史"];
type Reference = { id: string; title: string; url?: string; size?: number; sourceId?: string; state: "reading" | "ready" | "failed" };
type Hit = { title: string; url: string; snippet: string };

export function CreateRouteScreen({ onBack, onCreate, submitting, searchConfigured }: Props) {
  const [step, setStep] = useState(0);
  const [topic, setTopic] = useState("");
  const [level, setLevel] = useState(0);
  const [depth, setDepth] = useState(1);
  const [references, setReferences] = useState<Reference[]>([]);
  const [url, setUrl] = useState("");
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<Hit[]>([]);
  const [searching, setSearching] = useState(false);
  const [searched, setSearched] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const heading = useRef<HTMLHeadingElement>(null);
  const topicInput = useRef<HTMLInputElement>(null);
  const claimed = useRef(new Set<string>());
  const importFile = useSourceStore(s => s.importFile);
  const importUrl = useSourceStore(s => s.importUrl);
  useEffect(() => { heading.current?.focus(); }, [step]);
  const importing = references.some(r => r.state === "reading");
  const finish = (id: string, sourceId: string | null) => setReferences(rs => rs.map(r => r.id === id ? { ...r, sourceId: sourceId ?? undefined, state: sourceId ? "ready" : "failed" } : r));
  async function addLink(value: string, title?: string) {
    let normalized: string;
    try { const parsed = new URL(value.trim()); if (!["https:", "http:"].includes(parsed.protocol)) throw new Error(); normalized = parsed.href; }
    catch { setError("请输入完整的 http 或 https 网页链接。"); return; }
    if (claimed.current.has(normalized)) return;
    claimed.current.add(normalized);
    const id = crypto.randomUUID();
    setReferences(rs => [...rs, { id, title: title || hostnameOf(normalized), url: normalized, state: "reading" }]);
    setError(""); finish(id, await importUrl(normalized, title));
  }
  async function addFiles(files: FileList | null) {
    for (const file of Array.from(files ?? [])) {
      if (file.size > 20 * 1024 * 1024) { setError(`${file.name} 超过 20 MB，请选择较小的文件。`); continue; }
      const id = crypto.randomUUID();
      setReferences(rs => [...rs, { id, title: file.name, size: file.size, state: "reading" }]);
      finish(id, await importFile(file));
    }
  }
  async function search(ai: boolean) {
    if (searching) return;
    setSearching(true); setError(""); setSearched(false);
    try {
      const found = await api.searchWeb({ query: ai ? `${topic}；${LEVELS[level].label}；${DEPTHS[depth].label}` : query.trim() || topic, count: 6, ai });
      setHits(found); setSearched(true);
      if (ai) for (const hit of found.slice(0, 3)) await addLink(hit.url, hit.title);
    } catch (e) { setError(errorOf(e).message); }
    finally { setSearching(false); }
  }
  async function submit() {
    if (!topic.trim() || busy || submitting) return;
    if (step < 2) { setStep(step + 1); return; }
    if (importing || searching) return;
    setBusy(true); setError("");
    try { await onCreate({ topic: topic.trim(), experienceLevel: LEVELS[level].value, desiredDepth: DEPTHS[depth].value,
      sourceIds: [...new Set(references.flatMap(r => r.sourceId ? [r.sourceId] : []))], collectWeb: false }); }
    catch (e) { setError(errorOf(e).message); }
    finally { setBusy(false); }
  }
  return <section className="route-setup" aria-label="创建学习路线">
    <div className="setup-progress" role="progressbar" aria-label="学习引导进度" aria-valuemin={0} aria-valuemax={3} aria-valuenow={step + 1}><i style={{ width: `${(step + 1) / 3 * 100}%` }} /></div>
    <header className="setup-header"><span className="setup-brand">学涯<span>·</span></span><button type="button" onClick={() => step ? setStep(step - 1) : onBack()} aria-label={step ? "返回上一步" : "返回路线管理"}><Icon name="back" /><span>返回</span></button></header>
    <div className={`setup-scroll ${step === 2 ? "setup-materials" : ""}`}>
      <main className="setup-body">
        <h1 ref={heading} tabIndex={-1}>{["你想学什么？", "找到你的学习节奏。", "为好奇心，带一点线索。 "][step]}</h1>
        {step > 0 && <p className="setup-intro">{step === 1 ? `关于「${topic}」，按你的感觉选择就好。` : "交给 AI 寻找，或带上自己的资料。没有资料，也可以出发。"}</p>}
        <form id="route-form" onSubmit={e => { e.preventDefault(); void submit(); }}>
          {step === 0 && <div className="setup-topic">
            <label className="sr-only" htmlFor="setup-topic">想学的主题</label>
            <input ref={topicInput} id="setup-topic" autoComplete="off" maxLength={100} value={topic} onChange={e => setTopic(e.target.value)} placeholder="一个主题，或一个想弄懂的问题" />
            <div className="setup-suggestions" aria-label="快速候选">{IDEAS.map(idea => <button type="button" key={idea} onClick={() => { setTopic(idea); topicInput.current?.focus(); }}>{idea}<span aria-hidden="true">↗</span></button>)}</div>
          </div>}
          {step === 1 && <div className="setup-sliders">
            {([{ title: "接触程度", items: LEVELS, value: level, change: setLevel }, { title: "学习深度", items: DEPTHS, value: depth, change: setDepth }]).map(control => <div className="setup-slider" key={control.title}>
              <label htmlFor={control.title}>{control.title}<strong>{control.items[control.value].label}</strong></label>
              <input id={control.title} type="range" min={0} max={2} step={1} value={control.value} aria-valuetext={control.items[control.value].label} onChange={e => control.change(Number(e.target.value))} style={{ background: `linear-gradient(to right, var(--accent) ${control.value * 50}%, var(--line) ${control.value * 50}%)` }} />
              <div className="setup-scale">{control.items.map((item, index) => <button type="button" key={item.value} aria-pressed={index === control.value} onClick={() => control.change(index)}>{item.label}</button>)}</div>
              <p>{control.items[control.value].detail}</p>
            </div>)}
          </div>}
          {step === 2 && <>
            <div className="setup-source-grid">
              <section className="setup-search"><div className="setup-source-title"><Icon name="spark" /><h2>让 AI 帮你找</h2></div><p>整理检索方向，搜索真实网页，读取适合当前程度的资料。</p>
                <button className="primary" type="button" onClick={() => void search(true)} disabled={searching || !searchConfigured}>{searching ? "正在搜索并读取…" : "帮我找资料"}<Icon name="arrow" /></button>
                <div className="setup-link"><input aria-label="搜索关键词" value={query} onChange={e => setQuery(e.target.value)} placeholder={`搜索「${topic}」相关资料`} onKeyDown={e => { if (e.key === "Enter") { e.preventDefault(); void search(false); } }} /><button type="button" aria-label="搜索网页" disabled={searching || !searchConfigured} onClick={() => void search(false)}><Icon name="arrow" /></button></div>
                {!searchConfigured && <p role="status">暂时无法在线寻找资料，你仍可以带上自己的文件或网页。</p>}
              </section>
              <section className="setup-own"><div className="setup-source-title"><Icon name="book" /><h2>带上自己的资料</h2></div>
                <label className="setup-drop" htmlFor="setup-files"><Icon name="upload" /><strong>选择文件</strong><span>PDF、Word、文本或 Markdown · 20 MB 以内</span></label><input id="setup-files" className="sr-only" type="file" accept=".pdf,.docx,.txt,.md" multiple onChange={e => { void addFiles(e.target.files); e.target.value = ""; }} />
                <div className="setup-link"><input aria-label="网页链接" type="url" value={url} onChange={e => setUrl(e.target.value)} placeholder="或粘贴一个网页链接" onKeyDown={e => { if (e.key === "Enter") { e.preventDefault(); void addLink(url); setUrl(""); } }} /><button aria-label="添加来源链接" type="button" disabled={!url.trim()} onClick={() => { void addLink(url); setUrl(""); }}><Icon name="plus" /></button></div>
              </section>
            </div>
            {searching && <p className="setup-status" role="status">正在为「{topic}」寻找资料，已找到的内容会陆续出现在下方。</p>}
            {references.length > 0 && <section className="setup-references"><h2>这次带上的资料 <span>{references.filter(r => r.state === "ready").length}</span></h2>{references.map(ref => <div className="setup-reference" key={ref.id}><Icon name="book" /><div><strong>{ref.title}</strong><small>{ref.url ? hostnameOf(ref.url) : sizeLabel(ref.size ?? 0)} · {ref.state === "reading" ? "正在读取…" : ref.state === "ready" ? "已读取，会用于生成路线" : "读取失败，可移除后重试"}</small></div><button type="button" aria-label={`移除${ref.title}`} onClick={() => { if (ref.url) claimed.current.delete(ref.url); setReferences(rs => rs.filter(r => r.id !== ref.id)); }}><Icon name="close" /></button></div>)}</section>}
            {hits.length > 0 && <section className="setup-results"><h2>搜索结果</h2>{hits.map(hit => <article key={hit.url}><div><a href={hit.url} target="_blank" rel="noreferrer">{hit.title}</a><small>{hostnameOf(hit.url)}</small><p>{hit.snippet}</p></div><button type="button" disabled={references.some(r => r.url === hit.url)} onClick={() => void addLink(hit.url, hit.title)}>{references.some(r => r.url === hit.url) ? "已添加" : "添加"}</button></article>)}</section>}
            {searched && !hits.length && <p className="setup-status" role="status">暂时没有找到结果，换一个关键词再试试。</p>}
          </>}
          {error && <p className="setup-error" role="alert">{error}</p>}
        </form>
        <footer className="setup-actions"><button className="primary" type="submit" form="route-form" disabled={!topic.trim() || busy || submitting || importing || searching}>{busy || submitting ? "正在准备路线…" : step === 2 ? "生成我的路线" : "继续"}<Icon name="arrow" /></button>{step === 1 && <button className="setup-skip" type="button" onClick={() => { setLevel(0); setDepth(1); setStep(2); }}>跳过</button>}</footer>
      </main>
      <footer className="setup-footnote">每一点好奇，都有下一步。</footer>
    </div>
  </section>;
}
