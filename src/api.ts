import { invoke } from "@tauri-apps/api/core";

export interface AppErrorShape {
  code: string;
  message: string;
  details?: string | null;
  retryable: boolean;
}

export interface AppInfo {
  version: string;
  dataDir: string;
  providerCount: number;
  sourceCount: number;
  courseCount: number;
  searchConfigured: boolean;
}

export interface Provider {
  id: string;
  displayName: string;
  baseUrl: string;
  chatModel: string;
  embeddingModel: string;
  enabled: boolean;
  hasApiKey: boolean;
}

export interface Source {
  id: string;
  kind: string;
  title: string;
  uri?: string | null;
  originalFilename?: string | null;
  status: string;
  contentHash?: string | null;
  createdAt: string;
}

export interface Job {
  id: string;
  kind: string;
  status: string;
  progress: number;
  message: string;
  errorJson?: string | null;
  createdAt: string;
  startedAt?: string | null;
  completedAt?: string | null;
}

export interface KnowledgeHit {
  chunkId: string;
  documentId: string;
  text: string;
  pageNumber?: number | null;
  heading?: string | null;
  score: number;
  sourceId?: string | null;
  sourceTitle?: string | null;
}

export interface Course {
  id: string;
  topic: string;
  title: string;
  description: string;
  status: string;
  createdAt: string;
}

export interface CourseNode {
  id: string;
  courseId: string;
  title: string;
  description: string;
  objective: string;
  orderHint: number;
  estimatedMinutes: number;
  status: string;
  metadataJson: string;
}

export interface CourseDependency {
  nodeId: string;
  dependsOnNodeId: string;
}

export interface CourseDetail {
  course: Course;
  nodes: CourseNode[];
  dependencies: CourseDependency[];
}

export interface LessonCitation {
  chunkId: string;
  quote: string;
  text: string;
  pageNumber?: number | null;
}

export interface Lesson {
  title: string;
  explanation: string;
  keyPoints: string[];
  diagnosticQuestion: string;
  citations: LessonCitation[];
}

export interface LearningStart {
  sessionId: string;
  lesson: Lesson;
  mastery: number;
}

export interface LearningEvaluation {
  feedback: string;
  correctness: number;
  misconceptions: string[];
  masteryDelta: number;
  followUpQuestion?: string | null;
  suggestedAction: string;
  mastery: number;
  attempts: number;
}

export function errorOf(value: unknown): AppErrorShape {
  if (typeof value === "object" && value !== null && "code" in value && "message" in value) {
    return value as AppErrorShape;
  }
  if (typeof value === "string") {
    return { code: "INTERNAL_ERROR", message: value, retryable: false };
  }
  return { code: "INTERNAL_ERROR", message: "Unexpected error", retryable: false };
}

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw errorOf(error);
  }
}

export const api = {
  bootstrap: () => call<AppInfo>("app_bootstrap"),
  info: () => call<AppInfo>("app_get_info"),
  settingsGet: () => call<{ searchProvider: string; hasBraveApiKey: boolean }>("settings_get"),
  settingsUpdate: (input: { searchProvider?: string; braveApiKey?: string }) =>
    call<{ searchProvider: string; hasBraveApiKey: boolean }>("settings_update", { input }),
  providerList: () => call<Provider[]>("provider_list"),
  providerUpsert: (input: {
    id?: string;
    displayName: string;
    baseUrl: string;
    chatModel: string;
    embeddingModel: string;
    apiKey?: string;
    enabled?: boolean;
  }) => call<Provider>("provider_upsert", { input }),
  providerDelete: (id: string) => call<void>("provider_delete", { id }),
  providerTest: (id: string) => call<{ ok: boolean; message: string }>("provider_test", { id }),
  sourceList: () => call<Source[]>("source_list"),
  sourceImportBytes: (input: {
    filename: string;
    bytesBase64: string;
    title?: string;
    mimeType?: string;
  }) => call<Job>("source_import_bytes", { input }),
  sourceImportUrl: (input: { url: string; title?: string }) =>
    call<Job>("source_import_url", { input }),
  sourceDelete: (id: string) => call<void>("source_delete", { id }),
  sourceReindex: (id: string) => call<Job>("source_reindex", { id }),
  searchWeb: (input: { query: string; count?: number }) =>
    call<Array<{ title: string; url: string; snippet: string; source: string }>>("search_web", { input }),
  knowledgeSearch: (input: { query: string; limit?: number }) =>
    call<KnowledgeHit[]>("knowledge_search", { input }),
  knowledgeObject: (id: string) =>
    call<{ id: string; kind: string; title: string; description?: string | null }>("knowledge_get_object", {
      id,
      path: null,
    }),
  courseList: () => call<Course[]>("course_list"),
  courseGet: (id: string) => call<CourseDetail>("course_get", { id }),
  courseCreate: (input: { topic: string }) => call<Job>("course_create", { input }),
  courseDelete: (id: string) => call<void>("course_delete", { id }),
  courseRegenerate: (id: string) => call<Job>("course_regenerate", { id }),
  courseNodeGet: (nodeId: string) => call<CourseNode>("course_node_get", { nodeId }),
  courseNodePrepare: (nodeId: string) => call<Lesson>("course_node_prepare", { nodeId }),
  learningStart: (input: { nodeId: string }) => call<LearningStart>("learning_start", { input }),
  learningRespond: (input: { sessionId: string; answer: string }) =>
    call<LearningEvaluation>("learning_respond", { input }),
  learningFinish: (input: { sessionId: string }) => call<void>("learning_finish", { input }),
  jobGet: (id: string) => call<Job>("job_get", { id }),
  jobList: () => call<Job[]>("job_list"),
  jobCancel: (id: string) => call<Job>("job_cancel", { id }),
};

export async function watchJob(
  id: string,
  onUpdate: (job: Job) => void,
): Promise<Job> {
  for (;;) {
    const job = await api.jobGet(id);
    onUpdate(job);
    if (["completed", "failed", "cancelled"].includes(job.status)) {
      return job;
    }
    await new Promise((resolve) => window.setTimeout(resolve, 500));
  }
}
