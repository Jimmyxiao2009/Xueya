import "./styles.css";
import {
  api,
  errorOf,
  watchJob,
  type AppInfo,
  type Course,
  type CourseDetail,
  type LearningEvaluation,
  type LearningStart,
  type Lesson,
  type Provider,
  type Source,
} from "./api";

const root = document.querySelector<HTMLDivElement>("#app")!;

interface ViewState {
  info?: AppInfo;
  providers: Provider[];
  sources: Source[];
  courses: Course[];
  detail?: CourseDetail;
  lesson?: Lesson;
  start?: LearningStart;
  evaluation?: LearningEvaluation;
  searchResults: Array<{ title: string; url: string; snippet: string; source: string }>;
  message: string;
  error: string;
  busy: boolean;
}

const state: ViewState = {
  providers: [],
  sources: [],
  courses: [],
  searchResults: [],
  message: "Ready",
  error: "",
  busy: false,
};

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function setStatus(message: string, error = ""): void {
  state.message = message;
  state.error = error;
  render();
}

function showError(error: unknown): void {
  const value = errorOf(error);
  setStatus(value.message, value.details ?? value.message);
}

async function refresh(): Promise<void> {
  try {
    const [info, providers, sources, courses] = await Promise.all([
      api.info(),
      api.providerList(),
      api.sourceList(),
      api.courseList(),
    ]);
    state.info = info;
    state.providers = providers;
    state.sources = sources;
    state.courses = courses;
    render();
  } catch (error) {
    showError(error);
  }
}

function shell(content: string): string {
  const info = state.info;
  return `
    <main class="app-shell">
      <header class="topbar">
        <div>
          <h1>Verruckt</h1>
          <p class="muted">Local-first, BYOK self-study workbench</p>
        </div>
        <div class="status ${state.error ? "status-error" : ""}">
          <strong>${escapeHtml(state.message)}</strong>
          ${state.error ? `<span>${escapeHtml(state.error)}</span>` : ""}
        </div>
      </header>
      <section class="metrics">
        <span>${info?.providerCount ?? 0} providers</span>
        <span>${info?.sourceCount ?? 0} sources</span>
        <span>${info?.courseCount ?? 0} courses</span>
        <span>${info?.searchConfigured ? "web search ready" : "manual import ready"}</span>
      </section>
      ${content}
    </main>
  `;
}

function render(): void {
  root.innerHTML = shell(`
    <div class="grid">
      ${renderProviders()}
      ${renderSources()}
      ${renderCourses()}
      ${renderLesson()}
    </div>
  `);
}

function renderProviders(): string {
  const rows = state.providers
    .map(
      (provider) => `
      <div class="row">
        <div>
          <strong>${escapeHtml(provider.displayName)}</strong>
          <div class="muted">${escapeHtml(provider.baseUrl)} · ${escapeHtml(provider.chatModel)}</div>
          <div class="muted">embed: ${escapeHtml(provider.embeddingModel)} · key: ${provider.hasApiKey ? "stored" : "missing"}</div>
        </div>
        <div class="row-actions">
          <button data-action="provider-test" data-id="${provider.id}">Test</button>
          <button data-action="provider-delete" data-id="${provider.id}" class="danger">Delete</button>
        </div>
      </div>
    `,
    )
    .join("");
  return `
    <section class="panel">
      <h2>1. AI provider</h2>
      <form data-form="provider" class="form-grid">
        <input name="displayName" placeholder="Display name" required />
        <input name="baseUrl" placeholder="https://api.openai.com/v1" required />
        <input name="chatModel" placeholder="gpt-4o-mini" required />
        <input name="embeddingModel" placeholder="text-embedding-3-small" required />
        <input name="apiKey" type="password" placeholder="API key (stored in OS keyring)" />
        <button type="submit">Save provider</button>
      </form>
      <div class="list">${rows || '<p class="muted">No provider configured.</p>'}</div>
    </section>
  `;
}

function renderSources(): string {
  const rows = state.sources
    .map(
      (source) => `
      <div class="row">
        <div>
          <strong>${escapeHtml(source.title)}</strong>
          <div class="muted">${escapeHtml(source.kind)} · ${escapeHtml(source.status)} · ${source.contentHash?.slice(0, 10) ?? ""}</div>
        </div>
        <div class="row-actions">
          <button data-action="source-reindex" data-id="${source.id}">Reindex</button>
          <button data-action="source-delete" data-id="${source.id}" class="danger">Delete</button>
        </div>
      </div>
    `,
    )
    .join("");
  return `
    <section class="panel">
      <h2>2. Sources</h2>
      <div class="import-row">
        <input id="file-input" type="file" accept=".txt,.md,.markdown,.pdf,.html,.htm,.docx" multiple />
      </div>
      <form data-form="url" class="url-form">
        <input name="url" placeholder="https://example.com/article" required />
        <button type="submit">Import URL</button>
      </form>
      <div class="list">${rows || '<p class="muted">No sources yet. Import a PDF, Markdown, text, HTML, or DOCX file.</p>'}</div>
    </section>
  `;
}

function renderCourses(): string {
  const courseRows = state.courses
    .map(
      (course) => `
      <button class="course-row ${state.detail?.course.id === course.id ? "active" : ""}" data-action="course-open" data-id="${course.id}">
        <strong>${escapeHtml(course.title)}</strong>
        <span class="muted">${escapeHtml(course.topic)} · ${escapeHtml(course.status)}</span>
      </button>
    `,
    )
    .join("");
  const nodeRows = state.detail
    ? state.detail.nodes
        .map(
          (node) => `
          <div class="node-row">
            <div>
              <strong>${escapeHtml(node.title)}</strong>
              <div class="muted">${escapeHtml(node.objective)} · ${node.estimatedMinutes} min · ${escapeHtml(node.status)}</div>
            </div>
            <button data-action="node-prepare" data-id="${node.id}">Open lesson</button>
          </div>
        `,
        )
        .join("")
    : '<p class="muted">Select a course to inspect its DAG.</p>';
  return `
    <section class="panel">
      <h2>3. Courses</h2>
      <form data-form="course" class="course-form">
        <input name="topic" placeholder="e.g. Distributed consensus" required />
        <button type="submit">Create roadmap</button>
      </form>
      <div class="course-list">${courseRows || '<p class="muted">No courses yet.</p>'}</div>
      <div class="node-list">${nodeRows}</div>
    </section>
  `;
}

function renderLesson(): string {
  const lesson = state.lesson;
  const start = state.start;
  const evaluation = state.evaluation;
  if (!lesson) {
    return `
      <section class="panel">
        <h2>4. Learning session</h2>
        <p class="muted">Open a course node to generate a citation-backed lesson.</p>
      </section>
    `;
  }
  const citations = lesson.citations
    .map(
      (citation) => `
      <div class="citation">
        <p>${escapeHtml(citation.quote || citation.text.slice(0, 240))}</p>
        <span class="muted">chunk ${escapeHtml(citation.chunkId.slice(0, 8))}${citation.pageNumber ? ` · p.${citation.pageNumber}` : ""}</span>
      </div>
    `,
    )
    .join("");
  const answerForm = start
    ? `
      <form data-form="answer" class="answer-form">
        <textarea name="answer" rows="4" placeholder="Write your answer..." required></textarea>
        <button type="submit">Submit answer</button>
      </form>
    `
    : "";
  const evaluationBlock = evaluation
    ? `
      <div class="evaluation">
        <strong>Feedback:</strong>
        <p>${escapeHtml(evaluation.feedback)}</p>
        <p class="muted">correctness ${Math.round(evaluation.correctness * 100)}% · mastery ${Math.round(evaluation.mastery * 100)}% · attempts ${evaluation.attempts}</p>
        <p>${escapeHtml(evaluation.suggestedAction)}</p>
        ${evaluation.misconceptions.length ? `<p class="muted">Misconceptions: ${escapeHtml(evaluation.misconceptions.join("; "))}</p>` : ""}
      </div>
    `
    : "";
  return `
    <section class="panel">
      <h2>4. Learning session</h2>
      <h3>${escapeHtml(lesson.title)}</h3>
      <p>${escapeHtml(lesson.explanation)}</p>
      <h4>Diagnostic question</h4>
      <p>${escapeHtml(lesson.diagnosticQuestion)}</p>
      ${citations ? `<h4>Citations</h4>${citations}` : ""}
      ${answerForm}
      ${evaluationBlock}
    </section>
  `;
}

async function readFileBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(reader.error ?? new Error("read failed"));
    reader.onload = () => {
      const value = String(reader.result ?? "");
      const index = value.indexOf(",");
      resolve(index >= 0 ? value.slice(index + 1) : value);
    };
    reader.readAsDataURL(file);
  });
}

async function importFiles(files: FileList): Promise<void> {
  state.busy = true;
  render();
  try {
    for (const file of Array.from(files)) {
      const bytesBase64 = await readFileBase64(file);
      const job = await api.sourceImportBytes({
        filename: file.name,
        bytesBase64,
        mimeType: file.type || undefined,
      });
      await watchJob(job.id, (current) => setStatus(`${current.kind}: ${current.message}`));
    }
    await refresh();
    setStatus("Import complete");
  } catch (error) {
    showError(error);
  } finally {
    state.busy = false;
    render();
  }
}

function bindEvents(): void {
  root.addEventListener("change", (event) => {
    const target = event.target as HTMLInputElement;
    if (target.id === "file-input" && target.files?.length) {
      void importFiles(target.files);
    }
  });

  root.addEventListener("submit", (event) => {
    const form = event.target as HTMLFormElement;
    event.preventDefault();
    const formName = form.dataset.form;
    if (formName === "provider") {
      const data = new FormData(form);
      void api
        .providerUpsert({
          displayName: String(data.get("displayName") ?? ""),
          baseUrl: String(data.get("baseUrl") ?? ""),
          chatModel: String(data.get("chatModel") ?? ""),
          embeddingModel: String(data.get("embeddingModel") ?? ""),
          apiKey: String(data.get("apiKey") ?? "") || undefined,
          enabled: true,
        })
        .then(() => refresh())
        .catch(showError);
    }
    if (formName === "url") {
      const data = new FormData(form);
      void api
        .sourceImportUrl({ url: String(data.get("url") ?? "") })
        .then((job) => watchJob(job.id, (current) => setStatus(current.message)))
        .then(() => refresh())
        .catch(showError);
    }
    if (formName === "course") {
      const data = new FormData(form);
      void api
        .courseCreate({ topic: String(data.get("topic") ?? "") })
        .then((job) => watchJob(job.id, (current) => setStatus(current.message)))
        .then(() => refresh())
        .catch(showError);
    }
    if (formName === "answer" && state.start) {
      const data = new FormData(form);
      void api
        .learningRespond({
          sessionId: state.start.sessionId,
          answer: String(data.get("answer") ?? ""),
        })
        .then((evaluation) => {
          state.evaluation = evaluation;
          render();
        })
        .catch(showError);
    }
  });

  root.addEventListener("click", (event) => {
    const target = event.target as HTMLElement;
    const button = target.closest<HTMLButtonElement>("[data-action]");
    if (!button) return;
    const action = button.dataset.action;
    const id = button.dataset.id;
    if (action === "provider-test" && id) {
      void api.providerTest(id).then((result) => setStatus(result.message)).catch(showError);
    }
    if (action === "provider-delete" && id) {
      void api.providerDelete(id).then(() => refresh()).catch(showError);
    }
    if (action === "source-reindex" && id) {
      void api
        .sourceReindex(id)
        .then((job) => watchJob(job.id, (current) => setStatus(current.message)))
        .then(() => refresh())
        .catch(showError);
    }
    if (action === "source-delete" && id) {
      void api.sourceDelete(id).then(() => refresh()).catch(showError);
    }
    if (action === "course-open" && id) {
      void api
        .courseGet(id)
        .then((detail) => {
          state.detail = detail;
          state.lesson = undefined;
          state.start = undefined;
          state.evaluation = undefined;
          render();
        })
        .catch(showError);
    }
    if (action === "node-prepare" && id) {
      void api
        .courseNodePrepare(id)
        .then((lesson) => {
          state.lesson = lesson;
          state.evaluation = undefined;
          return api.learningStart({ nodeId: id });
        })
        .then((start) => {
          state.start = start;
          state.lesson = start.lesson;
          render();
        })
        .catch(showError);
    }
  });
}

render();
bindEvents();
void api
  .bootstrap()
  .then((info) => {
    state.info = info;
    return refresh();
  })
  .catch(showError);

window.addEventListener("beforeunload", () => {
  if (state.start) {
    void api.learningFinish({ sessionId: state.start.sessionId });
  }
});
