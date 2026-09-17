import { useEffect, useState } from "react";
import type { RouteCreateInput } from "@/api";
import { Icon } from "@/components/ui/Icon";
import { hostnameOf, sizeLabel } from "@/lib/format";
import { useSourceStore } from "@/stores/sourceStore";
import { toast } from "@/stores/uiStore";

interface CreateRouteScreenProps {
  onBack: () => void;
  onCreate: (input: RouteCreateInput) => void;
  submitting: boolean;
}

interface PendingFile {
  name: string;
  size: number;
  sourceId: string | null;
  uploading: boolean;
}

const DEPTHS: { value: string; label: string }[] = [
  { value: "overview", label: "概览" },
  { value: "practical", label: "实践" },
  { value: "deep", label: "深入" },
];

const LEVELS: { value: string; label: string }[] = [
  { value: "beginner", label: "从零开始" },
  { value: "some_experience", label: "接触过" },
  { value: "advanced", label: "已有基础" },
];

/**
 * "下一站，想探索什么？"
 *
 * Every source added here is imported for real: files are read in the webview
 * and sent as bytes, links are fetched by Rust, and both produce a source row
 * the route can then cite.
 */
export function CreateRouteScreen({ onBack, onCreate, submitting }: CreateRouteScreenProps) {
  const [topic, setTopic] = useState("");
  const [goal, setGoal] = useState("");
  const [depth, setDepth] = useState("overview");
  const [level, setLevel] = useState("beginner");
  const [web, setWeb] = useState(true);
  const [files, setFiles] = useState<PendingFile[]>([]);
  const [url, setUrl] = useState("");
  const [urls, setUrls] = useState<{ url: string; sourceId: string | null }[]>([]);
  const [busy, setBusy] = useState(false);

  const importFile = useSourceStore((state) => state.importFile);
  const importUrl = useSourceStore((state) => state.importUrl);
  const sources = useSourceStore((state) => state.sources);
  const loadSources = useSourceStore((state) => state.load);
  const sourceError = useSourceStore((state) => state.error);

  useEffect(() => {
    void loadSources();
  }, [loadSources]);

  const addFiles = async (list: FileList | null) => {
    if (!list?.length) return;
    const chosen = Array.from(list);
    setFiles((current) => [
      ...current,
      ...chosen.map((file) => ({
        name: file.name,
        size: file.size,
        sourceId: null,
        uploading: true,
      })),
    ]);
    for (const file of chosen) {
      const sourceId = await importFile(file);
      setFiles((current) =>
        current.map((entry) =>
          entry.name === file.name && entry.uploading
            ? { ...entry, sourceId, uploading: false }
            : entry,
        ),
      );
      if (sourceId) toast(`${file.name} 已加入资料库`);
    }
  };

  const addUrl = async () => {
    const value = url.trim();
    if (!value) return;
    if (urls.some((entry) => entry.url === value)) {
      toast("这个链接已经添加过了");
      return;
    }
    setUrl("");
    setUrls((current) => [...current, { url: value, sourceId: null }]);
    const sourceId = await importUrl(value);
    setUrls((current) =>
      current.map((entry) => (entry.url === value ? { ...entry, sourceId } : entry)),
    );
    if (sourceId) toast("链接已读取");
  };

  const submit = () => {
    const name = topic.trim();
    if (!name) {
      toast("先写下想学的主题");
      return;
    }
    if (busy || submitting) return;
    setBusy(true);
    const sourceIds = [
      ...files.map((file) => file.sourceId),
      ...urls.map((entry) => entry.sourceId),
    ].filter((value): value is string => Boolean(value));
    onCreate({
      topic: name,
      goal: goal.trim() || undefined,
      desiredDepth: depth,
      experienceLevel: level,
      sourceIds,
    });
  };

  return (
    <>
      <header className="create-header acrylic">
        <button className="close" onClick={onBack} aria-label="返回路线管理">
          <Icon name="back" />
        </button>
        <span>新的学习路线</span>
        <span className="header-orbit">
          <Icon name="spark" />
        </span>
      </header>
      <main className="create-page scroll-pane">
        <div className="eyebrow">FOLLOW YOUR CURIOSITY</div>
        <h1 className="page-title">下一站，想探索什么？</h1>
        <p className="subtext">一个问题，一本书，或者一个新的领域。</p>

        <form
          id="route-form"
          onSubmit={(event) => {
            event.preventDefault();
            submit();
          }}
        >
          <label htmlFor="topic">想学的主题</label>
          <input
            id="topic"
            className="text-input"
            placeholder="例如：线性代数、和声、机器学习"
            maxLength={60}
            value={topic}
            onChange={(event) => setTopic(event.target.value)}
          />

          <label htmlFor="goal">
            你的学习目标 <span>可选</span>
          </label>
          <textarea
            id="goal"
            className="text-input"
            rows={2}
            placeholder="比如，能读懂机器学习论文里的矩阵公式"
            value={goal}
            onChange={(event) => setGoal(event.target.value)}
          />

          <div className="form-section">
            <div className="row">
              <h2>起点与深度</h2>
              <span className="small">决定路线的长度与顺序</span>
            </div>
            <div className="choice-row">
              <span className="choice-label">当前水平</span>
              <div className="choice-group">
                {LEVELS.map((entry) => (
                  <button
                    key={entry.value}
                    type="button"
                    className={`choice${level === entry.value ? " selected" : ""}`}
                    aria-pressed={level === entry.value}
                    onClick={() => setLevel(entry.value)}
                  >
                    {entry.label}
                  </button>
                ))}
              </div>
            </div>
            <div className="choice-row">
              <span className="choice-label">期望深度</span>
              <div className="choice-group">
                {DEPTHS.map((entry) => (
                  <button
                    key={entry.value}
                    type="button"
                    className={`choice${depth === entry.value ? " selected" : ""}`}
                    aria-pressed={depth === entry.value}
                    onClick={() => setDepth(entry.value)}
                  >
                    {entry.label}
                  </button>
                ))}
              </div>
            </div>
          </div>

          <div className="form-section">
            <div className="row">
              <h2>参考资料</h2>
              <span className="small">可同时使用两种来源</span>
            </div>
            <label className="file-drop" htmlFor="reference-files">
              <Icon name="upload" />
              <strong>从本地添加资料</strong>
              <span>PDF、文档、笔记、图片 · 单个不超过 20 MB</span>
            </label>
            <input
              id="reference-files"
              type="file"
              accept=".pdf,.doc,.docx,.txt,.md,.png,.jpg,.jpeg,.webp,.ppt,.pptx"
              multiple
              hidden
              onChange={(event) => {
                void addFiles(event.target.files);
                event.target.value = "";
              }}
            />
            <div id="reference-list">
              {files.map((file, index) => (
                <div className="reference-row" key={`${file.name}-${index}`}>
                  <Icon name="book" />
                  <div>
                    <strong>{file.name}</strong>
                    <small>
                      {sizeLabel(file.size)} ·{" "}
                      {file.uploading
                        ? "正在解析…"
                        : file.sourceId
                          ? "已索引，可被引用"
                          : "未能解析"}
                    </small>
                  </div>
                  <button
                    type="button"
                    aria-label={`移除${file.name}`}
                    onClick={() =>
                      setFiles((current) => current.filter((_, i) => i !== index))
                    }
                  >
                    <Icon name="close" />
                  </button>
                </div>
              ))}
            </div>
          </div>

          <div className="web-source">
            <div className="row">
              <div className="web-title">
                <Icon name="orbit" />
                <span>
                  <strong>联网搜集资料</strong>
                  <small>公开课程、讲义与权威参考</small>
                </span>
              </div>
              <label className="toggle">
                <input
                  type="checkbox"
                  aria-label="联网搜集资料"
                  checked={web}
                  onChange={(event) => setWeb(event.target.checked)}
                />
                <span />
              </label>
            </div>
            <div id="web-options" hidden={!web}>
              <div className="url-input">
                <input
                  id="source-url"
                  className="text-input"
                  type="url"
                  placeholder="粘贴指定来源链接（可选）"
                  value={url}
                  onChange={(event) => setUrl(event.target.value)}
                />
                <button type="button" aria-label="添加来源链接" onClick={addUrl}>
                  <Icon name="plus" />
                </button>
              </div>
              <div id="url-list">
                {urls.map((entry) => (
                  <div className="reference-row" key={entry.url}>
                    <Icon name="book" />
                    <div>
                      <strong>{hostnameOf(entry.url)}</strong>
                      <small>
                        {entry.sourceId ? "已读取，可被引用" : "正在读取…"}
                      </small>
                    </div>
                    <button
                      type="button"
                      aria-label={`移除${entry.url}`}
                      onClick={() =>
                        setUrls((current) =>
                          current.filter((value) => value.url !== entry.url),
                        )
                      }
                    >
                      <Icon name="close" />
                    </button>
                  </div>
                ))}
              </div>
              <p className="prototype-note">
                联网检索需要先在设置中配置搜索服务；未配置时会直接使用你添加的资料，
                并由本地模型按关键词检索片段。
              </p>
            </div>
          </div>

          {sources.length ? (
            <p className="local-note">
              资料库中已有 {sources.length} 份资料，本次导入的会一起参与检索。
            </p>
          ) : null}
          {sourceError ? <p className="local-note">{sourceError}</p> : null}
        </form>
      </main>
      <footer className="create-dock">
        <button
          className="primary"
          form="route-form"
          type="submit"
          disabled={busy || submitting}
          data-motion="own"
        >
          生成学习路线 <Icon name="arrow" />
        </button>
      </footer>
    </>
  );
}
