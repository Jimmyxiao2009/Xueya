import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { useCloudStore } from "@/stores/cloudStore";
import { useRouteStore } from "@/stores/routeStore";
import { useNavStore } from "@/stores/uiStore";
import { useThemeStore } from "@/stores/themeStore";
import { isManagedBrowser } from "@/lib/runtime";
import { NavigationAnchor } from "@/components/ui/NavigationAnchor";
import { LocalConversation } from "./LocalConversation";
import { Atmosphere } from "@/components/ui/Atmosphere";

export function ThemeChoices({ showIconMotif = true }: { showIconMotif?: boolean } = {}) {
  const { colorScheme, visualMotif, iconMotif, choose, chooseMotif, chooseIcons } = useThemeStore();
  return <><p className="appearance-label">背景风格</p><div className="motif-choices" role="group" aria-label="背景风格">
    {(["lighthouse", "buoy"] as const).map((motif) => <button type="button" key={motif} aria-pressed={visualMotif === motif} onClick={() => chooseMotif(motif)}>{motif === "lighthouse" ? "灯塔雾海" : "浮标暖纸"}</button>)}
    </div>{showIconMotif ? <><p className="appearance-label">节点图标</p><div className="motif-choices" role="group" aria-label="节点图标">
    {(["lighthouse", "buoy"] as const).map((motif) => <button type="button" key={motif} aria-pressed={iconMotif === motif} onClick={() => chooseIcons(motif)}>{motif === "lighthouse" ? "灯塔图标" : "浮标图标"}</button>)}
    </div></> : null}<p className="appearance-label">背景明暗</p><div className="theme-choices" role="group" aria-label="背景明暗">
    {(["light", "dark"] as const).map((value) => <button type="button" key={value} className={`theme-choice theme-choice-${value}`} aria-pressed={colorScheme === value} onClick={() => choose(value)}>
      <span className="theme-sample" aria-hidden="true"><i /><i /><i /></span>
      <span>{value === "light" ? "日光纸面" : "夜色书房"}</span><small>{value === "light" ? "温暖、明亮" : "安静、柔和"}</small>
    </button>)}
    <button className="theme-system" type="button" aria-pressed={colorScheme === "system"} onClick={() => choose("system")}>跟随系统</button>
  </div></>;
}

function Frame({ children }: { step: number; children: ReactNode }) {
  const motif = useThemeStore((state) => state.visualMotif);
  return <main className="onboarding">
    {motif === "lighthouse" ? <Atmosphere density="dense" /> : null}
    <header className="onboarding-brand">学涯<span>·</span></header>
    <div className="conversation-world"><NavigationAnchor size={130} /><div className="navigation-horizon" /></div>
    <div className="onboarding-content">{children}</div>
    <footer className="onboarding-footer">每一点好奇，都有下一步。</footer>
  </main>;
}

function FirstRoute({ children }: { children: ReactNode }) {
  const load = useRouteStore((s) => s.load);
  const routes = useRouteStore((s) => s.routes);
  const error = useRouteStore((s) => s.error);
  const logout = useCloudStore((s) => s.logout);
  const busy = useCloudStore((s) => s.busy);
  const [ready, setReady] = useState(false);
  const [timedOut, setTimedOut] = useState(false);
  useEffect(() => { let alive = true; const timer = window.setTimeout(() => { if (alive) setTimedOut(true); }, 15000); void load().finally(() => { window.clearTimeout(timer); if (alive) { const current = useRouteStore.getState(); if (!current.error && !current.routes.length) useNavStore.getState().setView("create"); setReady(true); } }); return () => { alive = false; window.clearTimeout(timer); }; }, [load]);
  if (!ready && timedOut) return <Frame step={3}><h1>连接需要再试一次</h1><p role="alert">读取学习路径超时，请检查网络与服务器连接。</p><button className="primary" onClick={() => window.location.reload()}>重新连接</button></Frame>;
  if (!ready) return <Frame step={3}><h1>找回你的学习路径</h1><p role="status">正在读取账号中的学习记录…</p></Frame>;
  if (routes.length || !error) return children;
  return <Frame step={3}>
    <h1>暂时无法读取学习路径</h1><p role="alert" className="onboarding-error">{error}</p>
    <button className="primary" onClick={() => { setReady(false); void load().finally(() => { const current = useRouteStore.getState(); if (!current.error && !current.routes.length) useNavStore.getState().setView("create"); setReady(true); }); }}>重新读取路径</button>
    <button className="onboarding-text-button" type="button" disabled={busy} onClick={() => void logout()}>{busy ? "正在退出…" : "退出登录 / 切换账号"}</button>
  </Frame>;
}

export function OnboardingGate({ children }: { children: ReactNode }) {
  const [local, setLocal] = useState(() => {
    try { return !isManagedBrowser() && localStorage.getItem("verruckt.localStart") === "1"; } catch { return false; }
  });
  const [newLocal, setNewLocal] = useState(false);
  const { colorScheme, chosen, choose } = useThemeStore();
  const cloud = useCloudStore();
  const [themeConfirmed, setThemeConfirmed] = useState(() => {
    try { return chosen || localStorage.getItem("verruckt.onboarding.confirmed") === "1"; }
    catch { return chosen; }
  });
  const [mode, setMode] = useState<"register" | "login">("register");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [formError, setFormError] = useState("");
  const [restored, setRestored] = useState(false);
  const [restoreTimedOut, setRestoreTimedOut] = useState(false);
  const load = cloud.load;
  useEffect(() => { let alive = true; const timer = window.setTimeout(() => { if (alive) setRestoreTimedOut(true); }, 15000); void load().finally(() => { window.clearTimeout(timer); if (alive) setRestored(true); }); return () => { alive = false; window.clearTimeout(timer); }; }, [load]);
  async function submit(event: FormEvent) {
    event.preventDefault();
    if (cloud.busy) return;
    if (mode === "register" && password !== confirmation) { setFormError("两次输入的密码不一致"); return; }
    setFormError("");
    const ok = await cloud[mode](email.trim(), password);
    if (ok) { setPassword(""); setConfirmation(""); }
  }
  if (local) return newLocal ? <LocalConversation>{children}</LocalConversation> : children;
  const localStart = !isManagedBrowser() && <button className="secondary" onClick={() => {
    try { localStorage.setItem("verruckt.localStart", "1"); } catch { /* Session remains usable. */ }
    setNewLocal(true); setLocal(true);
  }}>仅在本设备开始 · 跳过登录</button>;
  if (!themeConfirmed) return <Frame step={1}>
    <div className="eyebrow">A LIGHT FOR YOUR CURIOSITY</div><h1>让好奇心，<br />有路可循。</h1><p>从一个想弄懂的问题出发，点亮你的知识地图。</p>
      <button className="primary" onClick={() => {
        choose(colorScheme);
        try { localStorage.setItem("verruckt.onboarding.confirmed", "1"); } catch { /* Storage is optional. */ }
        setThemeConfirmed(true);
      }}>{isManagedBrowser() ? "就用这个，继续" : "使用学涯账号继续"} <span aria-hidden="true">→</span></button>
      {localStart}
    <details className="onboarding-appearance"><summary>调整探索空间</summary><ThemeChoices showIconMotif={false} /></details>
  </Frame>;
  if (!restored) return <Frame step={2}><h1>欢迎来到 学涯</h1>{localStart}{restoreTimedOut ? <><p role="alert">连接暂时没有响应。请检查网络与服务器后重试。</p><button className="primary" onClick={() => window.location.reload()}>重新连接</button></> : <p role="status">正在连接你的学习空间…</p>}</Frame>;
  if (cloud.sessionState === "authenticated" || cloud.sessionState === "degradedOffline") return <FirstRoute key={cloud.account?.userId ?? "offline"}>{children}</FirstRoute>;
  return <Frame step={2}>
    <div className="eyebrow">YOUR OWN LITTLE UNIVERSE</div><h1>{mode === "register" ? <>让每次出发<br />都有迹可循。</> : <>欢迎回来，<br />继续你的好奇。</>}</h1><p>用自己的账号保存路径与学习进度。</p>
    <div className="onboarding-tabs" role="group" aria-label="账号操作">{(["register", "login"] as const).map((m) => <button key={m} aria-pressed={mode === m} onClick={() => { setMode(m); setFormError(""); setPassword(""); setConfirmation(""); }}>{m === "register" ? "注册新账号" : "登录已有账号"}</button>)}</div>
    <form className="onboarding-form" onSubmit={(event) => void submit(event)}>
      <label>邮箱<input type="email" required autoComplete="email" value={email} onChange={(event) => setEmail(event.target.value)} placeholder="you@example.com" disabled={cloud.busy} /></label>
      <label>密码<input type="password" required minLength={mode === "register" ? 8 : undefined} autoComplete={mode === "register" ? "new-password" : "current-password"} value={password} onChange={(event) => setPassword(event.target.value)} placeholder={mode === "register" ? "至少 8 个字符" : "输入你的密码"} disabled={cloud.busy} /></label>
      {mode === "register" && <label>确认密码<input type="password" required autoComplete="new-password" value={confirmation} onChange={(event) => setConfirmation(event.target.value)} placeholder="再输入一次密码" disabled={cloud.busy} /></label>}
      {(formError || cloud.error) && <p className="onboarding-error" role="alert">{formError || cloud.error}</p>}
      <button className="primary" type="submit" disabled={cloud.busy}>{cloud.busy ? "正在连接…" : mode === "register" ? "创建账号，开始探索" : "登录并继续"}</button>
    </form>
    <button className="onboarding-text-button" onClick={() => setThemeConfirmed(false)}>← 调整背景</button>
    {localStart}
  </Frame>;
}
