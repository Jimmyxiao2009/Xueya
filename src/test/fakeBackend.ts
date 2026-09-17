/**
 * An in-memory stand-in for the Rust backend.
 *
 * Frontend tests need the *real* IPC contract (camelCase DTOs, job polling,
 * grading semantics), so instead of stubbing individual calls this module
 * implements the same commands over a tiny data model. It mirrors the Rust
 * behaviour that the UI depends on: two seeded demo routes, the first node of
 * each already completed, five question kinds per node, and a wrong objective
 * answer inserting a review node.
 */

export interface FakeNode {
  id: string;
  courseId: string;
  title: string;
  description: string;
  objective: string;
  orderHint: number;
  estimatedMinutes: number;
  status: string;
  kind: string;
  confidence: number;
  reviewAt: string | null;
  generatedBy: string;
  metadataJson: string;
}

export interface FakeRoute {
  id: string;
  topic: string;
  title: string;
  description: string;
  status: string;
  goal: string | null;
  experienceLevel: string | null;
  desiredDepth: string | null;
  kind: string;
  currentNodeId: string | null;
  seeded: boolean;
  metadataJson: string;
  createdAt: string;
}

export interface FakeQuestion {
  id: string;
  nodeId: string;
  ordinal: number;
  kind: string;
  prompt: string;
  options: string[];
  answer: string[];
  difficulty: number;
}

const NOW = "2026-01-01T00:00:00Z";
const STEPS = [
  ["明确本课学习目标", "先弄清楚这一课要解决什么问题。", "3 min"],
  ["阅读与整理资料", "把资料里的说法和自己的理解对上。", "5 min"],
  ["形成自己的解释", "用自己的话复述所学内容。", "5 min"],
  ["通过练习检查掌握情况", "答错会触发路线插入加练节点。", "4 min"],
];

function nodeId(route: string, index: number): string {
  return `${route}-node-${index}`;
}

function makeNodes(route: FakeRoute): FakeNode[] {
  const titles =
    route.kind === "algebra"
      ? ["向量与线性组合", "数乘与方向", "加练 · 系数与线性组合", "子空间直觉", "第一阶段 · 小试身手", "综合实践"]
      : ["射影平面", "无穷远点", "加练 · 方向与齐次坐标", "交比与调和", "检查理解 · 小测", "综合实践"];
  return titles.map((title, index) => ({
    id: nodeId(route.id, index),
    courseId: route.id,
    title,
    description: `${title}的核心概念。`,
    objective: `理解${title}，并能用它解释一个现象。`,
    orderHint: index,
    estimatedMinutes: index === 2 ? 8 : 15,
    status: index === 0 ? "completed" : index === 1 ? "available" : "locked",
    kind: index === 2 ? "review" : index === 4 ? "quiz" : "lesson",
    confidence: 0,
    reviewAt: null,
    generatedBy: "deterministic",
    metadataJson: JSON.stringify({
      concepts: [`${title}·概念一`, `${title}·概念二`],
      knowledgePointId: `${route.id}-point-${index}`,
      origin: "deterministic",
    }),
  }));
}

function makeQuestions(node: FakeNode): FakeQuestion[] {
  const option = `${node.title}的正确说法`;
  const distractor = `${node.title}的常见误解`;
  return [
    {
      id: `${node.id}-q0`,
      nodeId: node.id,
      ordinal: 0,
      kind: "single",
      prompt: `关于${node.title}，下面哪个说法是对的？`,
      options: [option, distractor, "以上都不对"],
      answer: [option],
      difficulty: 1,
    },
    {
      id: `${node.id}-q1`,
      nodeId: node.id,
      ordinal: 1,
      kind: "multi",
      prompt: `下面哪些说法正确？`,
      options: [option, distractor, `${node.title}与它自身一致`],
      answer: [option, `${node.title}与它自身一致`],
      difficulty: 2,
    },
    {
      id: `${node.id}-q2`,
      nodeId: node.id,
      ordinal: 2,
      kind: "fill",
      prompt: `填空：这一课的主题是「____」。`,
      options: [],
      answer: [node.title],
      difficulty: 2,
    },
    {
      id: `${node.id}-q3`,
      nodeId: node.id,
      ordinal: 3,
      kind: "short",
      prompt: `用自己的话解释${node.title}。`,
      options: [],
      answer: [],
      difficulty: 3,
    },
    {
      id: `${node.id}-q4`,
      nodeId: node.id,
      ordinal: 4,
      kind: "image",
      prompt: `画出${node.title}的示意图。`,
      options: [],
      answer: [],
      difficulty: 3,
    },
  ];
}

export class FakeBackend {
  routes: FakeRoute[] = [];
  nodes = new Map<string, FakeNode[]>();
  questions = new Map<string, FakeQuestion[]>();
  adaptations = new Map<string, unknown[]>();
  jobs = new Map<string, Record<string, unknown>>();
  sessions = new Map<string, { nodeId: string; answered: number }>();
  sources: unknown[] = [];
  providers: Array<{
    id: string;
    displayName: string;
    baseUrl: string;
    chatModel: string;
    embeddingModel: string;
    enabled: boolean;
    apiKey: string | null;
  }> = [];
  settings: { searchProvider: string; braveApiKey: string | null } = {
    searchProvider: "none",
    braveApiKey: null,
  };
  calls: string[] = [];

  constructor() {
    this.seed("射影几何", "geometry");
    this.seed("线性代数", "algebra");
  }

  /** Mirrors `bootstrap.rs::seed_demo_routes`. */
  seed(title: string, kind: string) {
    const id = `route-${kind}`;
    const route: FakeRoute = {
      id,
      topic: title,
      title,
      description: `${title}的完整学习路线。`,
      status: "ready",
      goal: null,
      experienceLevel: "beginner",
      desiredDepth: "overview",
      kind,
      currentNodeId: nodeId(id, 1),
      seeded: true,
      metadataJson: JSON.stringify({
        learningObjectives: [`理解${title}的核心概念`],
        prerequisites: [],
        sourceIds: [],
        origin: "deterministic",
      }),
      createdAt: NOW,
    };
    this.routes.push(route);
    const nodes = makeNodes(route);
    this.nodes.set(id, nodes);
    this.adaptations.set(id, []);
    for (const node of nodes) this.questions.set(node.id, makeQuestions(node));
  }

  private courseDto(route: FakeRoute) {
    const nodes = this.nodes.get(route.id) ?? [];
    const completed = nodes.filter((node) => node.status === "completed").length;
    return {
      ...route,
      nodeCount: nodes.length,
      completedCount: completed,
      progress: nodes.length ? completed / nodes.length : 0,
    };
  }

  private lessonDto(node: FakeNode) {
    const questions = this.questions.get(node.id) ?? [];
    return {
      title: node.title,
      explanation: `${node.title}：${node.description}先抓住一个方向，再看它如何延伸。`,
      keyPoints: [`${node.title}的第一个关键点`, `${node.title}的第二个关键点`],
      diagnosticQuestion: `为什么${node.title}重要？`,
      citations: [],
      steps: STEPS.map(([title, detail, duration], ordinal) => ({
        id: `${node.id}-step-${ordinal}`,
        ordinal,
        title,
        detail,
        kind: "study",
        durationLabel: duration,
      })),
      questions: questions.map((question) => ({
        id: question.id,
        nodeId: question.nodeId,
        ordinal: question.ordinal,
        kind: question.kind,
        prompt: question.prompt,
        options: question.options,
        difficulty: question.difficulty,
      })),
      mastery: 0,
      estimatedMinutes: node.estimatedMinutes,
      nodeKind: node.kind,
      generatedBy: "deterministic",
      hasEvidence: false,
    };
  }

  async invoke(command: string, args?: Record<string, unknown>): Promise<unknown> {
    this.calls.push(command);
    const input = (args?.input ?? {}) as Record<string, unknown>;
    const id = (args?.id ?? args?.nodeId) as string | undefined;

    switch (command) {
      case "app_bootstrap":
      case "app_get_info":
        return {
          version: "0.1.0",
          dataDir: "/tmp/verruckt",
          providerCount: 1,
          sourceCount: this.sources.length,
          courseCount: this.routes.length,
          searchConfigured: false,
        };
      case "course_list":
        return this.routes.map((route) => this.courseDto(route));
      case "course_get": {
        const route = this.routes.find((entry) => entry.id === id);
        if (!route) throw { code: "not_found", message: "route not found", retryable: false };
        return {
          course: this.courseDto(route),
          nodes: this.nodes.get(route.id) ?? [],
          dependencies: (this.nodes.get(route.id) ?? [])
            .slice(1)
            .map((node, index) => ({
              nodeId: node.id,
              dependsOnNodeId: nodeId(route.id, index),
            })),
        };
      }
      case "course_create": {
        const topic = String(input.topic ?? "新路线");
        const jobId = `job-${this.jobs.size + 1}`;
        this.jobs.set(jobId, {
          id: jobId,
          kind: "generate_route",
          status: "running",
          progress: 0.3,
          message: "Requesting curriculum",
          errorJson: null,
          resultJson: null,
          createdAt: NOW,
          startedAt: NOW,
          completedAt: null,
          __topic: topic,
        });
        return this.jobs.get(jobId);
      }
      case "course_regenerate":
        return this.jobs.get("job-1");
      case "course_delete": {
        this.routes = this.routes.filter((route) => route.id !== id);
        if (id) this.nodes.delete(id);
        return null;
      }
      case "course_node_get": {
        for (const nodes of this.nodes.values()) {
          const node = nodes.find((entry) => entry.id === id);
          if (node) return node;
        }
        throw { code: "not_found", message: "node not found", retryable: false };
      }
      case "course_node_prepare": {
        const node = this.findNode(id);
        if (!node) throw { code: "not_found", message: "node not found", retryable: false };
        return this.lessonDto(node);
      }
      case "course_node_complete": {
        const node = this.findNode(id);
        if (!node) throw { code: "not_found", message: "node not found", retryable: false };
        const nodes = this.nodes.get(node.courseId) ?? [];
        const target = nodes.find((entry) => entry.id === node.id);
        if (target) target.status = "completed";
        const next = nodes.find(
          (entry) => entry.orderHint > node.orderHint && entry.status !== "completed",
        );
        if (next) next.status = "available";
        const route = this.routes.find((entry) => entry.id === node.courseId);
        if (route) route.currentNodeId = next?.id ?? null;
        return {
          course: route ? this.courseDto(route) : null,
          completedNodeId: node.id,
          nextNodeId: next?.id ?? null,
          adaptation: null,
        };
      }
      case "course_learner_states":
        return [];
      case "course_adaptations":
        return this.adaptations.get(id ?? "") ?? [];
      case "learning_start": {
        const node = this.findNode(String(input.nodeId));
        if (!node) throw { code: "not_found", message: "node not found", retryable: false };
        const sessionId = `session-${this.sessions.size + 1}`;
        this.sessions.set(sessionId, { nodeId: node.id, answered: 0 });
        return { sessionId, lesson: this.lessonDto(node), mastery: 0 };
      }
      case "learning_finish":
        return null;
      case "quiz_list":
        return this.questions.get(String(args?.nodeId)) ?? [];
      case "quiz_submit": {
        const questionId = String(input.questionId);
        let question: FakeQuestion | undefined;
        for (const list of this.questions.values()) {
          question = list.find((entry) => entry.id === questionId);
          if (question) break;
        }
        if (!question) throw { code: "not_found", message: "question not found", retryable: false };
        const answer = (input.answer as string[]) ?? [];
        const node = this.findNode(question.nodeId);
        const objective = ["single", "multi", "fill"].includes(question.kind);
        const graded = objective;
        const normalized = (value: string) => value.trim().replace(/\s+/g, "").toLowerCase();
        const correct =
          objective &&
          question.answer.length === answer.length &&
          question.answer.every((expected) =>
            answer.some((picked) => normalized(picked) === normalized(expected)),
          );
        let adaptation: unknown = null;
        if (objective && !correct && node) {
          const nodes = this.nodes.get(node.courseId) ?? [];
          const title = `加练 · ${node.title}`;
          if (!nodes.some((entry) => entry.title === title)) {
            const inserted: FakeNode = {
              ...node,
              id: `${node.courseId}-review-${nodes.length}`,
              title,
              kind: "review",
              status: "available",
              orderHint: node.orderHint + 1,
              generatedBy: "adaptation",
            };
            nodes.splice(node.orderHint + 1, 0, inserted);
            nodes.forEach((entry, index) => {
              entry.orderHint = index;
            });
            this.questions.set(inserted.id, makeQuestions(inserted));
            adaptation = {
              id: `adapt-${this.adaptations.get(node.courseId)?.length ?? 0}`,
              courseId: node.courseId,
              nodeId: node.id,
              kind: "review_inserted",
              reason: "答错了理解检查",
              payloadJson: "{}",
              createdAt: NOW,
            };
            this.adaptations.set(node.courseId, [
              adaptation,
              ...(this.adaptations.get(node.courseId) ?? []),
            ]);
          }
        }
        return {
          questionId,
          nodeId: question.nodeId,
          correct,
          graded,
          score: correct ? 1 : 0,
          feedback: correct ? "答对了。" : graded ? "再想一想。" : "作答已提交，等待评阅。",
          expected: question.answer,
          misconceptions: correct ? [] : ["需要再确认一次概念边界"],
          mastery: correct ? 0.3 : 0.05,
          confidence: 0.4,
          nextReview: null,
          adaptation,
          insertedNodeId: adaptation ? "inserted" : null,
        };
      }
      case "tutor_send":
        return {
          message: "我们换个角度想：先看它的定义，再看一个反例。",
          suggestedAction: "continue",
          diagnosis: null,
          generatedBy: "mock",
          routeUpdated: false,
          adaptation: null,
        };
      case "source_list":
        return this.sources;
      case "source_import_bytes":
      case "source_import_url": {
        const jobId = `source-job-${this.jobs.size + 1}`;
        const sourceId = `source-${this.sources.length + 1}`;
        this.sources.push({
          id: sourceId,
          kind: "file",
          title: "资料",
          uri: null,
          originalFilename: "notes.pdf",
          status: "ready",
          contentHash: null,
          createdAt: NOW,
        });
        this.jobs.set(jobId, {
          id: jobId,
          kind: "source_ingestion",
          status: "completed",
          progress: 1,
          message: "Indexed",
          errorJson: null,
          resultJson: JSON.stringify({ sourceId }),
          createdAt: NOW,
          startedAt: NOW,
          completedAt: NOW,
        });
        return this.jobs.get(jobId);
      }
      case "settings_get":
        return {
          searchProvider: this.settings.searchProvider,
          hasBraveApiKey: Boolean(this.settings.braveApiKey),
        };
      case "settings_update": {
        if (typeof input.searchProvider === "string") {
          this.settings.searchProvider = input.searchProvider;
        }
        if (typeof input.braveApiKey === "string" && input.braveApiKey) {
          this.settings.braveApiKey = input.braveApiKey;
        }
        return {
          searchProvider: this.settings.searchProvider,
          hasBraveApiKey: Boolean(this.settings.braveApiKey),
        };
      }
      case "provider_list":
        return this.providers.map((provider) => ({
          id: provider.id,
          displayName: provider.displayName,
          baseUrl: provider.baseUrl,
          chatModel: provider.chatModel,
          embeddingModel: provider.embeddingModel,
          enabled: provider.enabled,
          // The key itself is never returned, exactly like the Rust command.
          hasApiKey: Boolean(provider.apiKey),
        }));
      case "provider_upsert": {
        const existing = input.id
          ? this.providers.find((provider) => provider.id === input.id)
          : undefined;
        const target =
          existing ??
          ({
            id: `provider-${this.providers.length + 1}`,
            displayName: "",
            baseUrl: "",
            chatModel: "",
            embeddingModel: "",
            enabled: true,
            apiKey: null,
          } as (typeof this.providers)[number]);
        target.displayName = String(input.displayName ?? target.displayName);
        target.baseUrl = String(input.baseUrl ?? target.baseUrl);
        target.chatModel = String(input.chatModel ?? target.chatModel);
        target.embeddingModel = String(input.embeddingModel ?? target.embeddingModel);
        target.enabled = Boolean(input.enabled ?? target.enabled);
        if (typeof input.apiKey === "string" && input.apiKey) {
          target.apiKey = input.apiKey;
        }
        if (!existing) this.providers.push(target);
        return {
          id: target.id,
          displayName: target.displayName,
          baseUrl: target.baseUrl,
          chatModel: target.chatModel,
          embeddingModel: target.embeddingModel,
          enabled: target.enabled,
          hasApiKey: Boolean(target.apiKey),
        };
      }
      case "provider_delete":
        this.providers = this.providers.filter((provider) => provider.id !== id);
        return null;
      case "provider_test":
        return { ok: true, message: "reachable" };
      case "source_reindex": {
        const jobId = `reindex-job-${this.jobs.size + 1}`;
        this.jobs.set(jobId, {
          id: jobId,
          kind: "source_ingestion",
          status: "completed",
          progress: 1,
          message: "Reindexed",
          errorJson: null,
          resultJson: JSON.stringify({ sourceId: id }),
          createdAt: NOW,
          startedAt: NOW,
          completedAt: NOW,
        });
        return this.jobs.get(jobId);
      }
      case "source_delete":
        this.sources = this.sources.filter((source) => (source as { id: string }).id !== id);
        return null;
      case "search_web":
        return [];
      case "job_get": {
        const job = this.jobs.get(String(args?.id));
        if (!job) throw { code: "not_found", message: "job not found", retryable: false };
        if (job.status === "running") {
          const topic = String(job.__topic ?? "新路线");
          const routeId = `route-created-${this.routes.length + 1}`;
          const route: FakeRoute = {
            id: routeId,
            topic,
            title: topic,
            description: `${topic}的完整学习路线。`,
            status: "ready",
            goal: null,
            experienceLevel: "beginner",
            desiredDepth: "overview",
            kind: "custom",
            currentNodeId: nodeId(routeId, 0),
            seeded: false,
            metadataJson: JSON.stringify({ learningObjectives: [], prerequisites: [], sourceIds: [] }),
            createdAt: NOW,
          };
          this.routes.push(route);
          const nodes = makeNodes(route);
          nodes[0].status = "available";
          this.nodes.set(routeId, nodes);
          for (const node of nodes) this.questions.set(node.id, makeQuestions(node));
          this.adaptations.set(routeId, []);
          job.status = "completed";
          job.progress = 1;
          job.message = "Route ready";
          job.resultJson = JSON.stringify({ courseId: routeId });
          job.completedAt = NOW;
        }
        return job;
      }
      case "job_list":
        return [...this.jobs.values()];
      case "job_cancel":
        return this.jobs.get(String(args?.id)) ?? null;
      default:
        return null;
    }
  }

  private findNode(id: string | undefined): FakeNode | undefined {
    if (!id) return undefined;
    for (const nodes of this.nodes.values()) {
      const node = nodes.find((entry) => entry.id === id);
      if (node) return node;
    }
    return undefined;
  }
}

export const backend = new FakeBackend();
