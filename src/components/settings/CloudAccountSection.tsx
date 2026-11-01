import { useEffect, useState } from "react";
import { isManagedBrowser } from "@/lib/runtime";
import { Icon } from "@/components/ui/Icon";
import { useCloudStore } from "@/stores/cloudStore";
import { useSettingsStore } from "@/stores/settingsStore";

const AI_SOURCES = [
  {
    value: "cloud",
    label: "学涯云",
    detail: "官方模型服务，由账号方案和可用额度决定。",
  },
  {
    value: "byok",
    label: "自定义 API / BYOK",
    detail: "使用你自己的 OpenAI-compatible 服务，密钥保存在系统钥匙串。",
  },
  {
    value: "offline",
    label: "离线模式",
    detail: "学习保存在本机，使用明确标记的离线内容，不调用远端模型。",
  },
];

function planLabel(plan?: string | null): string {
  if (plan === "pro") return "Pro";
  if (plan === "free") return "Free";
  return plan || "Free";
}

export function CloudAccountSection() {
  const managed = isManagedBrowser();
  const account = useCloudStore((state) => state.account);
  const sessionState = useCloudStore((state) => state.sessionState);
  const devices = useCloudStore((state) => state.devices);
  const usage = useCloudStore((state) => state.usage);
  const syncStatus = useCloudStore((state) => state.syncStatus);
  const conflicts = useCloudStore((state) => state.conflicts);
  const syncing = useCloudStore((state) => state.syncing);
  const busy = useCloudStore((state) => state.busy);
  const error = useCloudStore((state) => state.error);
  const load = useCloudStore((state) => state.load);
  const login = useCloudStore((state) => state.login);
  const register = useCloudStore((state) => state.register);
  const logout = useCloudStore((state) => state.logout);
  const removeDevice = useCloudStore((state) => state.removeDevice);
  const syncNow = useCloudStore((state) => state.syncNow);
  const syncFiles = useCloudStore((state) => state.syncFiles);
  const setSyncEnabled = useCloudStore((state) => state.setSyncEnabled);
  const setBaseUrl = useCloudStore((state) => state.setBaseUrl);

  const aiMode = useSettingsStore((state) => state.aiMode);
  const setAiMode = useSettingsStore((state) => state.setAiMode);

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [baseUrl, setBaseUrlDraft] = useState(account?.baseUrl ?? "http://127.0.0.1:8080");

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (account?.baseUrl) setBaseUrlDraft(account.baseUrl);
  }, [account?.baseUrl]);

  const accountState = account?.state ?? sessionState;
  const authenticated = accountState === "authenticated";
  const degraded = accountState === "degradedOffline";
  const restoring = accountState === "restoring";
  const signedOut = accountState === "signedOut";
  const officialAi = account?.entitlements?.officialAi ?? false;
  const creditsLimit = account?.entitlements?.monthlyAiCredits ?? 0;
  const creditsUsed =
    usage?.creditsUsed ?? account?.entitlements?.monthlyAiCreditsUsed ?? 0;
  const exhausted = officialAi && creditsUsed >= creditsLimit;
  const cloudSelectable = Boolean(authenticated && officialAi && !exhausted);
  const creditPercent =
    creditsLimit > 0 ? Math.min(100, Math.round((creditsUsed / creditsLimit) * 100)) : 0;

  const selectAiMode = async (mode: string) => {
    if (mode === "cloud" && !cloudSelectable) return;
    await setAiMode(mode);
  };

  return (
    <>
      {!managed && <div className="form-section">
        <div className="row">
          <h2>AI 来源</h2>
          <span className="small">
            {authenticated
              ? "已登录"
              : degraded
                ? "离线 · 等待重新连接"
                : restoring
                  ? "正在恢复…"
                  : "本地优先"}
          </span>
        </div>
        <div className="mt-3 space-y-2">
          {AI_SOURCES.map((source) => {
            const disabled = source.value === "cloud" && !cloudSelectable;
            const selected = aiMode === source.value;
            return (
              <button
                key={source.value}
                type="button"
                disabled={disabled}
                aria-pressed={selected}
                onClick={() => void selectAiMode(source.value)}
                className="settings-status"
                data-motion="menu"
              >
                <span className="flex items-center justify-between gap-3">
                  <strong className="text-sm text-primary">{source.label}</strong>
                  {selected ? <Icon name="check" /> : null}
                </span>
                <small className="mt-1 block text-xs text-muted">
                  {source.detail}
                  {source.value === "cloud" && degraded
                    ? " 官方 AI 暂不可用；BYOK 与离线模式仍可用。"
                    : source.value === "cloud" && exhausted
                      ? " 本月额度已用完，请切换到自定义 API 或离线模式。"
                    : source.value === "cloud" && !cloudSelectable
                      ? " 需要先登录并确认会员能力。"
                      : ""}
                </small>
              </button>
            );
          })}
        </div>
        {aiMode === "cloud" && !officialAi ? (
          <p className="local-note mt-3">
            当前账户未包含 学涯 AI。你仍可使用自定义模型或离线学习。
          </p>
        ) : null}
      </div>}

      <div className="form-section">
        <div className="row">
          <h2>学涯云</h2>
          <span className="small">{planLabel(account?.plan)}</span>
        </div>

        {restoring ? (
          <div className="mt-3 settings-status">
            <p className="prototype-note">正在恢复 Cloud session…</p>
            <p className="mt-2 text-xs text-muted">本地学习、路线与资料始终可用。</p>
          </div>
        ) : signedOut ? (
          <form
            className="mt-3 space-y-3"
            onSubmit={(event) => {
              event.preventDefault();
              void login(email.trim(), password);
            }}
          >
            <p className="prototype-note">
              登录后可使用官方 AI，并默认开启跨设备同步；你可以随时关闭。
            </p>
            <label htmlFor="cloud-email">邮箱</label>
            <input
              id="cloud-email"
              className="text-input"
              type="email"
              autoComplete="email"
              placeholder="you@example.com"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
            />
            <label htmlFor="cloud-password">密码</label>
            <input
              id="cloud-password"
              className="text-input"
              type="password"
              autoComplete="current-password"
              placeholder="至少 8 位"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
            />
            <div className="flex gap-2">
              <button className="primary" type="submit" disabled={busy} data-motion="own">
                登录 <Icon name="arrow" />
              </button>
              <button
                className="rounded-md border border-subtle px-4 py-2 text-sm text-primary"
                type="button"
                disabled={busy}
                onClick={() => void register(email.trim(), password)}
                data-motion="menu"
              >
                创建账号
              </button>
            </div>
          </form>
        ) : account ? (
          <div className="mt-3 space-y-3">
            <div className="settings-status">
              <div className="flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <strong className="block truncate text-sm text-primary">
                    {account.email}
                  </strong>
                  <small className="text-xs text-muted">
                    {degraded ? "离线 · 等待重新连接" : "在线"} ·{" "}
                    {planLabel(account.plan)} · {account.device?.name ?? "当前设备"}
                  </small>
                  {degraded ? (
                    <p className="mt-2 text-xs text-muted">
                      {managed ? "服务器连接暂不可用，请稍后重试。" : `上次已知 ${planLabel(account.plan)} · 官方 AI 暂不可用 · 本地 BYOK 与离线模式仍可用`}
                    </p>
                  ) : null}
                </div>
                <button
                  type="button"
                  className="text-xs text-muted"
                  onClick={() => void logout()}
                  data-motion="menu"
                >
                  退出登录
                </button>
              </div>
            </div>

            <div className="settings-status">
              <div className="flex items-center justify-between gap-3">
                <div>
                  <strong className="block text-sm text-primary">云同步</strong>
                  <small className="text-xs text-muted">
                    {degraded
                      ? "等待连接 · 本地学习不受影响"
                      : account.syncEnabled
                        ? "同步学习路线、进度、笔记和选择的学习资料"
                        : "账号仅用于官方 AI，不同步本地数据"}
                  </small>
                </div>
                <button
                  type="button"
                  className={`choice${account.syncEnabled ? " selected" : ""}`}
                  aria-pressed={account.syncEnabled}
                  disabled={degraded || busy}
                  onClick={() => void setSyncEnabled(!account.syncEnabled)}
                  data-motion="menu"
                >
                  {degraded ? "等待连接" : account.syncEnabled ? "已开启" : "已关闭"}
                </button>
              </div>
              {degraded ? (
                <div className="mt-3 flex flex-wrap items-center gap-2 border-t border-subtle pt-3">
                  <small className="mr-auto text-[11px] text-muted">
                    已保留本地路线、进度与资料，联网后会自动追赶。
                  </small>
                  <button
                    type="button"
                    className="rounded-md border border-subtle px-3 py-1.5 text-xs text-primary"
                    disabled={busy}
                    onClick={() => void load()}
                    data-motion="menu"
                  >
                    重试连接
                  </button>
                </div>
              ) : account.syncEnabled ? (
                <div className="mt-3 flex flex-wrap items-center gap-2 border-t border-subtle pt-3">
                  <small className="mr-auto text-[11px] text-muted">
                    {syncStatus
                      ? `待同步 ${syncStatus.pendingChanges} 项 · 冲突 ${syncStatus.openConflicts} 项`
                      : "正在读取同步状态…"}
                  </small>
                  <button
                    type="button"
                    className="rounded-md border border-subtle px-3 py-1.5 text-xs text-primary"
                    disabled={syncing}
                    onClick={() => void syncNow()}
                    data-motion="menu"
                  >
                    {syncing ? "同步中…" : "立即同步"}
                  </button>
                  <button
                    type="button"
                    className="rounded-md border border-subtle px-3 py-1.5 text-xs text-primary"
                    disabled={syncing}
                    onClick={() => void syncFiles()}
                    data-motion="menu"
                  >
                    同步学习资料
                  </button>
                </div>
              ) : null}
              {conflicts.length > 0 ? (
                <p className="local-note mt-3">
                  检测到 {conflicts.length} 个同步冲突，本地副本已保留，不会被静默覆盖。
                </p>
              ) : null}
            </div>

            <div className="settings-status">
              <div className="flex items-center justify-between gap-3">
                <strong className="text-sm text-primary">本月 AI 使用额度</strong>
                <small className="text-xs text-muted">
                  {!account.entitlements ? "权限信息待确认" : !officialAi ? "未开通官方 AI" : `${creditsUsed} / ${creditsLimit}`}
                </small>
              </div>
              <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-subtle">
                <div
                  className="h-full rounded-full bg-accent transition-all"
                  style={{ width: `${creditPercent}%` }}
                />
              </div>
              {exhausted ? (
                <small className="mt-2 block text-xs text-review">
                  {managed ? "额度已用完，请联系服务器管理员。" : "额度已用完，可在上方切换到自定义 API 或离线模式。"}
                </small>
              ) : null}
            </div>

            <div className="settings-status">
              <div className="row">
                <strong className="text-sm text-primary">设备</strong>
                <small className="text-xs text-muted">
                  {devices.length} / {account.entitlements?.maxDevices ?? "?"}
                </small>
              </div>
              <ul className="mt-2 space-y-2">
                {devices.map((device) => (
                  <li key={device.id} className="flex items-center justify-between gap-3">
                    <span className="min-w-0">
                      <strong className="block truncate text-xs text-primary">
                        {device.name}
                      </strong>
                      <small className="text-[11px] text-muted">
                        {device.platform} · {device.appVersion}
                      </small>
                    </span>
                    {device.id === account.device?.id ? (
                      <small className="text-[11px] text-muted">当前设备</small>
                    ) : (
                      <button
                        type="button"
                        className="text-[11px] text-muted"
                        onClick={() => void removeDevice(device.id)}
                        data-motion="menu"
                      >
                        移除
                      </button>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          </div>
        ) : null}

        {!managed && <details className="mt-3">
          <summary className="cursor-pointer text-xs text-muted">Cloud 服务地址</summary>
          <div className="mt-2 flex gap-2">
            <input
              className="text-input"
              type="url"
              value={baseUrl}
              onChange={(event) => setBaseUrlDraft(event.target.value)}
              aria-label="Cloud 服务地址"
            />
            <button
              type="button"
              className="rounded-md border border-subtle px-3 text-xs text-primary"
              onClick={() => void setBaseUrl(baseUrl.trim())}
              data-motion="menu"
            >
              应用
            </button>
          </div>
        </details>}

        {error ? <p className="local-note mt-3">{error}</p> : null}
      </div>
    </>
  );
}
