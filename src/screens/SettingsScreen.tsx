import { useEffect, useState } from "react";
import { isManagedBrowser } from "@/lib/runtime";
import { ThemeChoices } from "@/components/onboarding/OnboardingGate";
import { CloudAccountSection } from "@/components/settings/CloudAccountSection";
import { Icon } from "@/components/ui/Icon";
import { toast } from "@/stores/uiStore";
import {
  EMPTY_DRAFT,
  useSettingsStore,
  type ProviderDraft,
  type ProviderProbe,
} from "@/stores/settingsStore";

function capabilityLines(capabilities: NonNullable<ProviderProbe["capabilities"]>): string[] {
  const mark = (value: boolean) => (value ? "✓" : "✗");
  const lines = [
    `Chat             ${mark(capabilities.chatCompletions || capabilities.responses)}`,
    `Streaming        ${mark(capabilities.streaming)}`,
    `Structured       ${mark(capabilities.structuredOutput)}`,
  ];
  if (capabilities.embeddings) {
    lines.push(
      capabilities.embeddingDimension
        ? `Embeddings       ✓ · ${capabilities.embeddingDimension} dimensions`
        : "Embeddings       ✓ · dimension unknown",
    );
  } else {
    lines.push("Embeddings       ✗");
  }
  lines.push(`Protocol         ${capabilities.protocol ?? "unknown"}`);
  return lines;
}

interface SettingsScreenProps {
  onBack: () => void;
}

const SEARCH_OPTIONS = [
  { value: "none", label: "不联网" },
  { value: "auto", label: "自动 · 全部已配置" },
  { value: "tavily", label: "Tavily" },
  { value: "serper", label: "Serper · Google" },
  { value: "searxng", label: "SearXNG" },
  { value: "brave", label: "Brave Search" },
];

/**
 * BYOK settings: which model service 学涯 talks to, and whether it may search
 * the web.
 *
 * The API key field is write-only. It is never prefilled, never echoed back, and
 * the backend stores it in the OS keyring — the UI can only learn *whether* a
 * key exists (`hasApiKey`).
 */
export function SettingsScreen({ onBack }: SettingsScreenProps) {
  const managed = isManagedBrowser();
  const providers = useSettingsStore((state) => state.providers);
  const searchProvider = useSettingsStore((state) => state.searchProvider);
  const hasBraveApiKey = useSettingsStore((state) => state.hasBraveApiKey);
  const hasTavilyApiKey = useSettingsStore((state) => state.hasTavilyApiKey);
  const hasSerperApiKey = useSettingsStore((state) => state.hasSerperApiKey);
  const searxngUrl = useSettingsStore((state) => state.searxngUrl);
  const probes = useSettingsStore((state) => state.probes);
  const error = useSettingsStore((state) => state.error);
  const load = useSettingsStore((state) => state.load);
  const save = useSettingsStore((state) => state.save);
  const remove = useSettingsStore((state) => state.remove);
  const test = useSettingsStore((state) => state.test);
  const saveSearch = useSettingsStore((state) => state.saveSearch);

  const [draft, setDraft] = useState<ProviderDraft>(EMPTY_DRAFT);
  const [busy, setBusy] = useState(false);
  const [search, setSearch] = useState(searchProvider);
  const [braveKey, setBraveKey] = useState("");
  const [tavilyKey, setTavilyKey] = useState("");
  const [serperKey, setSerperKey] = useState("");
  const [searxngBase, setSearxngBase] = useState(searxngUrl);
  const [advancedOpen, setAdvancedOpen] = useState(false);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    setSearch(searchProvider);
  }, [searchProvider]);

  useEffect(() => {
    setSearxngBase(searxngUrl);
  }, [searxngUrl]);

  const patch = (values: Partial<ProviderDraft>) =>
    setDraft((current) => ({ ...current, ...values }));

  const submitProvider = async () => {
    if (busy) return;
    if (!draft.displayName.trim() || !draft.baseUrl.trim() || !draft.chatModel.trim()) {
      toast("名称、地址与对话模型都要填写");
      return;
    }
    setBusy(true);
    const ok = await save(draft);
    setBusy(false);
    if (ok) {
      toast(draft.id ? "模型服务已更新" : "模型服务已保存");
      setDraft(EMPTY_DRAFT);
    } else {
      toast("保存失败，请检查地址与密钥");
    }
  };

  return (
    <>
      <header className="create-header acrylic">
        <button className="close" onClick={onBack} aria-label="返回路线管理">
          <Icon name="back" />
        </button>
        <span>设置</span>
        <span className="header-orbit">
          <Icon name="spark" />
        </span>
      </header>

      <main className="create-page scroll-pane settings-surface">
        <div className="eyebrow">YOUR LEARNING SPACE</div>
        <h1 className="page-title">账号与学涯云</h1>
        <p className="subtext">
          管理账号、同步、外观与高级模型设置。普通使用无需配置 Provider。
        </p>

        <CloudAccountSection />

        <div className="form-section">
          <h2>外观</h2>
          <ThemeChoices />
        </div>
        <details
          className="form-section advanced-settings"
          open={advancedOpen}
          onToggle={(event) =>
            setAdvancedOpen((event.currentTarget as HTMLDetailsElement).open)
          }
        >
          <summary>
            <h2>高级设置</h2>
            <p className="prototype-note">自定义 AI / BYOK、搜索服务与 Provider 测试仅在这里需要时配置。</p>
          </summary>


        <div className="form-section">
          <div className="row">
            <h2>{managed ? "服务器模型服务" : "自定义模型服务"}</h2>
            <span className="small">
              {providers.length ? `已配置 ${providers.length} 个` : "尚未配置"}
            </span>
          </div>

          {managed && <p className="prototype-note">模型与搜索由管理员配置。密钥在服务器上加密保存，不会发送到浏览器。</p>}
          {providers.map((provider) => {
            const probe = probes[provider.id];
            return (
              <div
                key={provider.id}
                className="mt-3 rounded-lg border border-subtle bg-paper px-4 py-3 shadow-panel"
              >
                <div className="flex items-center gap-3">
                  <Icon name="spark" />
                  <div className="min-w-0 flex-1">
                    <strong className="block truncate text-sm text-primary">
                      {provider.displayName}
                    </strong>
                    <small className="block truncate text-xs text-muted">
                      {provider.chatModel} · {provider.baseUrl}
                    </small>
                    <small className="block text-xs text-muted">
                      {provider.hasApiKey ? (managed ? "API Key 已在服务器上加密保存" : "API Key 已存入系统钥匙串") : "未设置 API Key"}
                    </small>
                  </div>
                  <button
                    type="button"
                    className="rounded-md border border-subtle px-3 py-1 text-xs text-primary"
                    onClick={() => {
                      void test(provider.id);
                      toast("正在测试连接…");
                    }}
                    data-motion="menu"
                  >
                    测试
                  </button>
                  {!managed && <button
                    type="button"
                    aria-label={`移除${provider.displayName}`}
                    className="px-1 text-muted"
                    onClick={() => {
                      void remove(provider.id);
                      toast("模型服务已移除");
                    }}
                    data-motion="menu"
                  >
                    <Icon name="trash" />
                  </button>}
                </div>
                {probe ? (
                  <p
                    className={`mt-2 text-xs ${probe.ok ? "text-success" : "text-review"}`}
                  >
                    {probe.ok ? "连接正常" : "连接失败"} · {probe.message}
                  </p>
                ) : null}
                {probe?.capabilities ? (
                  <ul className="mt-2 space-y-0.5 font-mono text-[11px] text-muted">
                    {capabilityLines(probe.capabilities).map((line) => (
                      <li key={line}>{line}</li>
                    ))}
                  </ul>
                ) : null}
                {!managed && <button
                  type="button"
                  className="mt-2 text-xs text-accent"
                  onClick={() =>
                    setDraft({
                      id: provider.id,
                      displayName: provider.displayName,
                      baseUrl: provider.baseUrl,
                      chatModel: provider.chatModel,
                      embeddingModel: provider.embeddingModel,
                      apiKey: "",
                      enabled: provider.enabled,
                    })
                  }
                  data-motion="menu"
                >
                  编辑这个服务
                </button>}
              </div>
            );
          })}

          {!managed && <form
            id="provider-form"
            onSubmit={(event) => {
              event.preventDefault();
              void submitProvider();
            }}
          >
            <label htmlFor="provider-name">
              显示名称 <span>可选</span>
            </label>
            <input
              id="provider-name"
              className="text-input"
              placeholder="例如：我的 OpenAI"
              value={draft.displayName}
              onChange={(event) => patch({ displayName: event.target.value })}
            />

            <label htmlFor="provider-base">接口地址</label>
            <input
              id="provider-base"
              className="text-input"
              type="url"
              placeholder="https://api.openai.com/v1"
              value={draft.baseUrl}
              onChange={(event) => patch({ baseUrl: event.target.value })}
            />

            <label htmlFor="provider-chat">对话模型</label>
            <input
              id="provider-chat"
              className="text-input"
              placeholder="gpt-4o-mini"
              value={draft.chatModel}
              onChange={(event) => patch({ chatModel: event.target.value })}
            />

            <label htmlFor="provider-embed">
              向量模型 <span>可选</span>
            </label>
            <input
              id="provider-embed"
              className="text-input"
              placeholder="text-embedding-3-small"
              value={draft.embeddingModel}
              onChange={(event) => patch({ embeddingModel: event.target.value })}
            />

            <label htmlFor="provider-key">
              API Key {draft.id ? <span>留空则不修改</span> : null}
            </label>
            <input
              id="provider-key"
              className="text-input"
              type="password"
              autoComplete="off"
              placeholder={draft.id ? "已保存 · 留空保持不变" : "sk-…"}
              value={draft.apiKey}
              onChange={(event) => patch({ apiKey: event.target.value })}
            />

            <p className="prototype-note">
              密钥只写入操作系统的钥匙串，不会回传给界面，也不会出现在任何日志里。
            </p>
          </form>}
        </div>

        {!managed && <div className="form-section">
          <div className="row">
            <h2>联网搜索</h2>
            <span className="small">
              {hasTavilyApiKey || hasBraveApiKey || hasSerperApiKey || searxngUrl
                ? "已配置"
                : "未配置"}
            </span>
          </div>
          <div className="choice-row">
            <span className="choice-label">搜索服务</span>
            <div className="choice-group">
              {SEARCH_OPTIONS.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  className={`choice${search === option.value ? " selected" : ""}`}
                  aria-pressed={search === option.value}
                  onClick={() => setSearch(option.value)}
                >
                  {option.label}
                </button>
              ))}
            </div>
          </div>
          {search === "auto" ? (
            <p className="local-note">
              自动模式会同时使用已保存的 Tavily、Brave、Serper 与 SearXNG，并把多路结果合并去重；没有配置任何服务时仍可手动导入文件或网页。
            </p>
          ) : null}
          {search === "tavily" ? (
            <>
              <label htmlFor="tavily-key">
                Tavily API Key {hasTavilyApiKey ? <span>留空则不修改</span> : null}
              </label>
              <input
                id="tavily-key"
                className="text-input"
                type="password"
                autoComplete="off"
                placeholder={hasTavilyApiKey ? "已保存 · 留空保持不变" : "填写 Tavily 密钥（tvly-…）"}
                value={tavilyKey}
                onChange={(event) => setTavilyKey(event.target.value)}
              />
            </>
          ) : null}
          {search === "serper" ? (
            <>
              <label htmlFor="serper-key">
                Serper API Key {hasSerperApiKey ? <span>留空则不修改</span> : null}
              </label>
              <input
                id="serper-key"
                className="text-input"
                type="password"
                autoComplete="off"
                placeholder={hasSerperApiKey ? "已保存 · 留空保持不变" : "填写 serper.dev 密钥"}
                value={serperKey}
                onChange={(event) => setSerperKey(event.target.value)}
              />
            </>
          ) : null}
          {search === "searxng" ? (
            <>
              <label htmlFor="searxng-url">
                SearXNG 地址 {searxngUrl ? <span>已保存 · 留空则清除</span> : null}
              </label>
              <input
                id="searxng-url"
                className="text-input"
                type="url"
                autoComplete="off"
                placeholder="https://你的-searxng.example/search"
                value={searxngBase}
                onChange={(event) => setSearxngBase(event.target.value)}
              />
            </>
          ) : null}
          {search === "brave" ? (
            <>
              <label htmlFor="brave-key">
                Brave API Key {hasBraveApiKey ? <span>留空则不修改</span> : null}
              </label>
              <input
                id="brave-key"
                className="text-input"
                type="password"
                autoComplete="off"
                placeholder={hasBraveApiKey ? "已保存 · 留空保持不变" : "填写搜索服务密钥"}
                value={braveKey}
                onChange={(event) => setBraveKey(event.target.value)}
              />
            </>
          ) : null}
          <button
            type="button"
            className="primary mt-4"
            data-motion="own"
            onClick={() => {
              void saveSearch({
                searchProvider: search,
                braveApiKey: braveKey.trim() || undefined,
                tavilyApiKey: tavilyKey.trim() || undefined,
                serperApiKey: serperKey.trim() || undefined,
                searxngUrl:
                  search === "searxng" || search === "auto"
                    ? searxngBase.trim()
                    : undefined,
              });
              setBraveKey("");
              setTavilyKey("");
              setSerperKey("");
              toast("搜索设置已保存");
            }}
          >
            保存搜索设置 <Icon name="arrow" />
          </button>
        </div>

        }
        </details>
        {error ? <p className="local-note">{error}</p> : null}
      </main>

      {!managed && advancedOpen && <footer className="create-dock">
        <button
          className="primary"
          form="provider-form"
          type="submit"
          disabled={busy}
          data-motion="own"
        >
          {draft.id ? "更新模型服务" : "保存模型服务"} <Icon name="arrow" />
        </button>
      </footer>}
    </>
  );
}
