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
  resultJson?: string | null;
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

/* ------------------------------------------------------------------ routes */

export interface LearningRoute {
  id: string;
  topic: string;
  title: string;
  description: string;
  status: string;
  goal?: string | null;
  experienceLevel?: string | null;
  desiredDepth?: string | null;
  kind: string;
  currentNodeId?: string | null;
  seeded: boolean;
  nodeCount: number;
  completedCount: number;
  progress: number;
  metadataJson: string;
  createdAt: string;
}

export interface RouteNode {
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
  reviewAt?: string | null;
  generatedBy: string;
  metadataJson: string;
}

export interface RouteDependency {
  nodeId: string;
  dependsOnNodeId: string;
}

export interface RouteDetail {
  course: LearningRoute;
  nodes: RouteNode[];
  dependencies: RouteDependency[];
}

export interface RouteCreateInput {
  topic: string;
  goal?: string | null;
  experienceLevel?: string | null;
  desiredDepth?: string | null;
  sourceIds?: string[];
}

export interface CourseNodeComplete {
  course: LearningRoute;
  completedNodeId: string;
  nextNodeId?: string | null;
  adaptation?: AdaptationEvent | null;
}

export interface AdaptationEvent {
  id: string;
  courseId: string;
  nodeId?: string | null;
  kind: string;
  reason: string;
  payloadJson: string;
  createdAt: string;
}

export interface LearnerState {
  knowledgePointId: string;
  title: string;
  mastery: number;
  confidence: number;
  mistakes: number;
  attempts: number;
  lastSeen?: string | null;
  nextReview?: string | null;
  reviewDue: boolean;
}

/* ------------------------------------------------------------------ lesson */

export interface LessonCitation {
  chunkId: string;
  quote: string;
  text: string;
  pageNumber?: number | null;
  /** Name of the source the chunk came from, when it is known. */
  sourceTitle?: string | null;
  /** Original link, so the learner can open the real material. */
  uri?: string | null;
}

export interface LessonStep {
  id: string;
  ordinal: number;
  title: string;
  detail: string;
  kind: string;
  durationLabel: string;
}

export interface QuizQuestion {
  id: string;
  nodeId: string;
  ordinal: number;
  kind: string;
  prompt: string;
  options: string[];
  difficulty: number;
}

export interface Lesson {
  title: string;
  explanation: string;
  keyPoints: string[];
  diagnosticQuestion: string;
  citations: LessonCitation[];
  steps: LessonStep[];
  questions: QuizQuestion[];
  mastery: number;
  estimatedMinutes: number;
  nodeKind: string;
  generatedBy: string;
  hasEvidence: boolean;
}

export interface LearningStart {
  sessionId: string;
  lesson: Lesson;
  mastery: number;
}

export interface QuizResult {
  questionId: string;
  nodeId: string;
  correct: boolean;
  graded: boolean;
  score: number;
  feedback: string;
  expected: string[];
  misconceptions: string[];
  mastery: number;
  confidence: number;
  nextReview?: string | null;
  adaptation?: AdaptationEvent | null;
  insertedNodeId?: string | null;
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

export interface TutorTurn {
  message: string;
  suggestedAction: string;
  diagnosis?: string | null;
  generatedBy: string;
  routeUpdated: boolean;
  adaptation?: AdaptationEvent | null;
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
    call<Array<{ title: string; url: string; snippet: string; source: string }>>("search_web", {
      input,
    }),
  knowledgeSearch: (input: { query: string; limit?: number }) =>
    call<KnowledgeHit[]>("knowledge_search", { input }),
  knowledgeObject: (id: string) =>
    call<{ id: string; kind: string; title: string; description?: string | null }>(
      "knowledge_get_object",
      { id, path: null },
    ),

  /* routes */
  routeList: () => call<LearningRoute[]>("course_list"),
  routeGet: (id: string) => call<RouteDetail>("course_get", { id }),
  routeCreate: (input: RouteCreateInput) => call<Job>("course_create", { input }),
  routeDelete: (id: string) => call<void>("course_delete", { id }),
  routeRegenerate: (id: string) => call<Job>("course_regenerate", { id }),
  nodeGet: (nodeId: string) => call<RouteNode>("course_node_get", { nodeId }),
  nodeComplete: (nodeId: string) => call<CourseNodeComplete>("course_node_complete", { nodeId }),
  routeLearnerStates: (id: string) => call<LearnerState[]>("course_learner_states", { id }),
  routeAdaptations: (id: string, limit?: number) =>
    call<AdaptationEvent[]>("course_adaptations", { id, limit: limit ?? null }),

  /* learning */
  nodePrepare: (nodeId: string) => call<Lesson>("course_node_prepare", { nodeId }),
  learningStart: (input: { nodeId: string }) => call<LearningStart>("learning_start", { input }),
  learningRespond: (input: { sessionId: string; answer: string }) =>
    call<LearningEvaluation>("learning_respond", { input }),
  learningFinish: (input: { sessionId: string }) => call<void>("learning_finish", { input }),
  quizList: (nodeId: string) => call<QuizQuestion[]>("quiz_list", { nodeId }),
  quizSubmit: (input: { questionId: string; answer: string[]; sessionId?: string | null }) =>
    call<QuizResult>("quiz_submit", { input }),
  tutorSend: (input: { sessionId: string; message: string }) =>
    call<TutorTurn>("tutor_send", { input }),

  /* jobs */
  jobGet: (id: string) => call<Job>("job_get", { id }),
  jobList: () => call<Job[]>("job_list"),
  jobCancel: (id: string) => call<Job>("job_cancel", { id }),
};

const TERMINAL = ["completed", "failed", "cancelled"];

export async function watchJob(id: string, onUpdate: (job: Job) => void): Promise<Job> {
  for (;;) {
    const job = await api.jobGet(id);
    onUpdate(job);
    if (TERMINAL.includes(job.status)) {
      return job;
    }
    await new Promise((resolve) => window.setTimeout(resolve, 220));
  }
}
