import { useEffect } from "react";
import { Icon, type IconName } from "@/components/ui/Icon";
import { hostnameOf, relativeTime } from "@/lib/format";
import { toast } from "@/stores/uiStore";
import { useSourceStore } from "@/stores/sourceStore";

interface SourcesScreenProps {
  onBack: () => void;
}

const KIND_ICON: Record<string, IconName> = {
  file: "book",
  url: "orbit",
  web: "spark",
  note: "line",
};

const STATUS_COPY: Record<string, string> = {
  ready: "已索引，可被引用",
  indexing: "正在解析…",
  pending: "等待处理",
  failed: "解析失败",
};

/**
 * The source library. Everything a route can cite lives here, so this is where
 * a learner sees what 学涯 actually knows and can drop material that is wrong
 * or out of date.
 */
export function SourcesScreen({ onBack }: SourcesScreenProps) {
  const sources = useSourceStore((state) => state.sources);
  const loading = useSourceStore((state) => state.loading);
  const error = useSourceStore((state) => state.error);
  const load = useSourceStore((state) => state.load);
  const remove = useSourceStore((state) => state.remove);
  const reindex = useSourceStore((state) => state.reindex);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <>
      <header className="create-header acrylic">
        <button className="close" onClick={onBack} aria-label="返回路线管理">
          <Icon name="back" />
        </button>
        <span>资料库</span>
        <span className="header-orbit">
          <Icon name="book" />
        </span>
      </header>

      <main className="create-page scroll-pane sources-surface">
        <div className="eyebrow">YOUR OWN MATERIAL</div>
        <h1 className="page-title">资料库</h1>
        <p className="subtext">
          路线里的引用都来自这里。移除一份资料后，新生成的课程不会再引用它。
        </p>

        {loading && !sources.length ? (
          <p className="empty-note">正在读取资料库…</p>
        ) : null}

        {!loading && !sources.length ? (
          <p className="empty-note">
            还没有资料。回到「探索新的未知」添加文件或链接，它们会出现在这里。
          </p>
        ) : null}

        <div className="mt-4 flex flex-col gap-3">
          {sources.map((source) => (
            <div
              key={source.id}
              className="rounded-lg border border-subtle bg-paper px-4 py-3 shadow-panel"
            >
              <div className="flex items-center gap-3">
                <Icon name={KIND_ICON[source.kind] ?? "book"} />
                <div className="min-w-0 flex-1">
                  <strong className="block truncate text-sm text-primary">
                    {source.title || source.originalFilename || "未命名资料"}
                  </strong>
                  <small className="block truncate text-xs text-muted">
                    {source.uri ? hostnameOf(source.uri) : source.originalFilename ?? source.kind}
                    {" · "}
                    {relativeTime(source.createdAt)}
                  </small>
                  <small className="block text-xs text-muted">
                    {STATUS_COPY[source.status] ?? source.status}
                  </small>
                </div>
                <button
                  type="button"
                  className="rounded-md border border-subtle px-3 py-1 text-xs text-primary"
                  onClick={() => {
                    void reindex(source.id);
                    toast("正在重新解析…");
                  }}
                  data-motion="menu"
                >
                  重新索引
                </button>
                <button
                  type="button"
                  aria-label={`移除${source.title}`}
                  className="px-1 text-muted"
                  onClick={() => {
                    void remove(source.id);
                    toast("资料已移除");
                  }}
                  data-motion="menu"
                >
                  <Icon name="trash" />
                </button>
              </div>
            </div>
          ))}
        </div>

        {sources.length ? (
          <p className="local-note">共 {sources.length} 份资料</p>
        ) : null}
        {error ? <p className="local-note">{error}</p> : null}
      </main>
    </>
  );
}
