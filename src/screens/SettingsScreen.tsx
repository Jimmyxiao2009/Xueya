import { useEffect, useState } from "react";
import { Icon } from "@/components/ui/Icon";
import { toast } from "@/stores/uiStore";
import {
  EMPTY_DRAFT,
  useSettingsStore,
  type ProviderDraft,
} from "@/stores/settingsStore";

interface SettingsScreenProps {
  onBack: () => void;
}

const SEARCH_OPTIONS = [
  { value: "none", label: "不联网" },
  { value: "brave", label: "Brave Search" },
];

/**
 * BYOK settings: which model service Orbit talks to, and whether it may search
 * the web.
 *
 * The API key field is write-only. It is never prefilled, never echoed back, and
 * the backend stores it in the OS keyring — the UI can only learn *whether* a
 * key exists (`hasApiKey`).
 */
export function SettingsScreen({ onBack }: SettingsScreenProps) {
  const providers = useSettingsStore((state) => state.providers);
  const searchProvider = useSettingsStore((state) => state.searchProvider);
  const hasBraveApiKey = useSettingsStore((state) => state.hasBraveApiKey);
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

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    setSearch(searchProvider);
  }, [searchProvider]);

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

      <main className="create-page scroll-pane">
        <div className="eyebrow">BRING YOUR OWN KEY</div>
        <h1 className="page-title">模型与搜索</h1>
        <p className="subtext">
          不配置也能用：Orbit 会用本地模型生成路线与题目。配置之后，
          路线、讲解与题目改由你自己的模型产出。
        </p>

        <div className="form-section">
          <div className="row">
            <h2>模型服务</h2>
            <span className="small">
              {providers.length ? `已配置 ${providers.length} 个` : "尚未配置"}
            </span>
          </div>

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
                      {provider.hasApiKey ? "API Key 已存入系统钥匙串" : "未设置 API Key"}
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
                  <button
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
                  </button>
                </div>
                {probe ? (
                  <p
                    className={`mt-2 text-xs ${probe.ok ? "text-success" : "text-review"}`}
                  >
                    {probe.ok ? "连接正常" : "连接失败"} · {probe.message}
                  </p>
                ) : null}
                <button
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
                </button>
              </div>
            );
          })}

          <form
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
          </form>
        </div>

        <div className="form-section">
          <div className="row">
            <h2>联网搜索</h2>
            <span className="small">
              {hasBraveApiKey ? "已配置" : "未配置"}
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
              });
              setBraveKey("");
              toast("搜索设置已保存");
            }}
          >
            保存搜索设置 <Icon name="arrow" />
          </button>
        </div>

        {error ? <p className="local-note">{error}</p> : null}
      </main>

      <footer className="create-dock">
        <button
          className="primary"
          form="provider-form"
          type="submit"
          disabled={busy}
          data-motion="own"
        >
          {draft.id ? "更新模型服务" : "保存模型服务"} <Icon name="arrow" />
        </button>
      </footer>
    </>
  );
}
