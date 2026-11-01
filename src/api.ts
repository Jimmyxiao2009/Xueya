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
  protocol: string;
  maskedHint?: string | null;
}

export interface ProviderCapabilities {
  chatCompletions: boolean;
  responses: boolean;
  streaming: boolean;
  structuredOutput: boolean;
  embeddings: boolean;
  embeddingDimension?: number | null;
  visionOptional: boolean;
  protocol?: string | null;
  probeErrors: string[];
}

export interface CloudEntitlements {
  cloudSync: boolean;
  officialAi: boolean;
  maxDevices: number;
  monthlyAiCredits: number;
  storageBytes: number;
  devicesUsed: number;
  monthlyAiCreditsUsed: number;
  cloudStorageBytesUsed: number;
  plan: string;
}

export interface CloudDevice {
  id: string;
  name: string;
  platform: string;
  appVersion: string;
  createdAt: string;
  lastSeenAt: string;
}

export type CloudSessionState =
  | "restoring"
  | "authenticated"
  | "degradedOffline"
  | "signedOut";

export interface CloudAccountState {
  state: CloudSessionState;
  loggedIn: boolean;
  email?: string | null;
  userId?: string | null;
  plan?: string | null;
  entitlements?: CloudEntitlements | null;
  device?: CloudDevice | null;
  syncEnabled: boolean;
  baseUrl: string;
  lastSuccessfulRefreshAt?: string | null;
}

export interface CloudMe {
  id: string;
  email: string;
  displayName?: string | null;
  createdAt: string;
}

export interface CloudUsage {
  periodStart: string;
  creditsUsed: number;
  creditsLimit: number;
  creditsRemaining: number;
}

export interface SyncStatus {
  loggedIn: boolean;
  syncEnabled: boolean;
  cloudSyncEntitled: boolean;
  bootstrapDone: boolean;
  pendingChanges: number;
  openConflicts: number;
  cursor: string;
  lastPushAt?: string | null;
  lastPullAt?: string | null;
  lastError?: string | null;
}

export interface SyncSummary {
  pushed: number;
  pulled: number;
  conflicts: number;
  skipped: boolean;
  reason?: string | null;
}

export interface SyncConflict {
  id: string;
  entityType: string;
  entityId: string;
  serverRevision: number;
  clientRevision: number;
  createdAt: string;
}

export interface FileSyncSummary {
  uploaded: number;
  downloaded: number;
  reindexSourceIds: string[];
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

export interface ConversationMessage {
  id: string;
  role: string;
  content: string;
  metadataJson: string;
  createdAt: string;
}


export interface LearningStart {
  sessionId: string;
  lesson: Lesson;
  mastery: number;
  /** Agent-authored first turn. Missing when the opening could not be generated. */
  openingTurn?: TutorTurn | null;
  openingError?: string | null;
  resumed?: boolean;
  messages?: ConversationMessage[];
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

export interface Diagnosis {
  kind: string;
  knowledgePointIds: string[];
  confidence: number;
  severity: number;
  explanation: string;
}

export interface TutorCitation {
  chunkId: string;
  sourceId: string;
  sourceTitle: string;
  sourceKind: string;
  sourceUri?: string | null;
  sourceFilename?: string | null;
  text: string;
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
  verdict?: string | null;
  strengths: string[];
  gaps: string[];
  diagnoses: Diagnosis[];
  confidence?: number | null;
  adaptation?: AdaptationEvent | null;
  insertedNodeId?: string | null;
}

export interface TutorTurn {
  message: string;
  suggestedAction: string;
  diagnosis?: string | null;
  diagnosisDetail?: Diagnosis | null;
  citations: TutorCitation[];
  grounding: string;
  generatedBy: string;
  routeUpdated: boolean;
  insertedNodeId?: string | null;
  adaptation?: AdaptationEvent | null;
  masteryDelta?: number;
  mastery?: number;
  readyToComplete?: boolean;
}

/** Event shape emitted by the Rust `tutor_stream` event. */
export type AiStreamEvent =
  | { type: "started"; model: string }
  | { type: "delta"; text: string }
  | { type: "citationMetadata"; citations: string[] }
  | { type: "completed"; finishReason?: string | null }
  | { type: "failed"; code: string; message: string };

export function errorOf(value: unknown): AppErrorShape {
  if (value instanceof TypeError && value.message.includes("invoke") && !("__TAURI_INTERNALS__" in window)) {
    return { code: "DESKTOP_REQUIRED", message: "当前为浏览器页面，请在桌面应用中使用本地资料与学习功能。", retryable: false };
  }
  if (typeof value === "object" && value !== null && "code" in value && "message" in value) {
    return value as AppErrorShape;
  }
  if (typeof value === "string") {
    return { code: "INTERNAL_ERROR", message: value, retryable: false };
  }
  return { code: "INTERNAL_ERROR", message: "Unexpected error", retryable: false };
}

let dataGeneration = 0;
export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (["cloud_login", "cloud_register", "cloud_logout", "cloud_set_base_url"].includes(command)) dataGeneration++;
  const generation = dataGeneration;
  const current = (value: T): T => {
    if (generation !== dataGeneration) throw { code: "STALE_SESSION", message: "账号已切换，请重新读取。", retryable: false };
    return value;
  };
  try {
    if (import.meta.env.MODE !== "test" && !("__TAURI_INTERNALS__" in window)) {
      const response = await fetch("/api/invoke", {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json", "X-Orbit-Client": "web" },
        body: JSON.stringify({ command, args }),
        signal: AbortSignal.timeout(190_000),
      });
      if (!response.headers.get("content-type")?.includes("application/json")) {
        throw { code: "SERVICE_UNAVAILABLE", message: "学习服务尚未连接，请检查服务器是否启动。", retryable: true };
      }
      const payload = await response.json();
      if (!response.ok) throw payload;
      return current(payload as T);
    }
    return current(await invoke<T>(command, args));
  } catch (error) {
    throw errorOf(error);
  }
}

export interface SettingsShape {
  searchProvider: string;
  hasBraveApiKey: boolean;
  hasTavilyApiKey: boolean;
  hasSerperApiKey: boolean;
  searxngUrl: string;
  aiMode: string;
}

/** Active tutor stream listeners, keyed by request id, so a screen can unsubscribe on leave. */
const tutorStreamCancels = new Map<string, () => void>();

/** Removes the `tutor_stream` listener for a request id immediately. */
export function cancelTutorStream(requestId: string): void {
  tutorStreamCancels.get(requestId)?.();
}

export const api = {
  bootstrap: () => call<AppInfo>("app_bootstrap"),
  info: () => call<AppInfo>("app_get_info"),
  competitionDemoSeed: () => call<string>("competition_demo_seed"),
  settingsGet: () => call<SettingsShape>("settings_get"),
  settingsUpdate: (input: {
    searchProvider?: string;
    braveApiKey?: string;
    tavilyApiKey?: string;
    serperApiKey?: string;
    searxngUrl?: string;
    aiMode?: string;
  }) =>
    call<SettingsShape>("settings_update", { input }),
  cloudAccountState: () => call<CloudAccountState>("cloud_account_state"),
  cloudRegister: (input: { email: string; password: string }) =>
    call<CloudAccountState>("cloud_register", { input }),
  cloudLogin: (input: { email: string; password: string }) =>
    call<CloudAccountState>("cloud_login", { input }),
  cloudLogout: () => call<CloudAccountState>("cloud_logout"),
  cloudGetMe: () => call<CloudMe>("cloud_get_me"),
  cloudGetEntitlements: () => call<CloudEntitlements>("cloud_get_entitlements"),
  cloudListDevices: () => call<CloudDevice[]>("cloud_list_devices"),
  cloudRemoveDevice: (id: string) => call<void>("cloud_remove_device", { id }),
  cloudGetAiUsage: () => call<CloudUsage>("cloud_get_ai_usage"),
  cloudSetBaseUrl: (url: string) => call<CloudAccountState>("cloud_set_base_url", { url }),
  cloudSetSyncEnabled: (enabled: boolean) =>
    call<CloudAccountState>("cloud_set_sync_enabled", { enabled }),
  syncStatus: () => call<SyncStatus>("sync_status"),
  syncNow: () => call<SyncSummary>("sync_now"),
  syncConflicts: () => call<SyncConflict[]>("sync_conflicts"),
  syncFilesNow: () => call<FileSyncSummary>("sync_files_now"),
  providerList: () => call<Provider[]>("provider_list"),
  providerUpsert: (input: {
    id?: string;
    displayName: string;
    baseUrl: string;
    chatModel: string;
    embeddingModel: string;
    apiKey?: string;
    enabled?: boolean;
    protocol?: string;
  }) => call<Provider>("provider_upsert", { input }),
  providerDelete: (id: string) => call<void>("provider_delete", { id }),
  providerTest: (id: string) =>
    call<{ ok: boolean; message: string; protocol?: string | null; capabilities?: ProviderCapabilities | null }>(
      "provider_test",
      { id },
    ),
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
  searchWeb: (input: { query: string; count?: number; ai?: boolean }) =>
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
  /**
   * Streams a tutor turn. Deltas arrive through the `tutor_stream` event and
   * the resolved value is the same authoritative turn as `tutorSend`, so the
   * caller can replace the buffered text with the final message.
   *
   * The listener is removed as soon as the caller leaves the screen (see
   * `cancelTutorStream`), so late deltas from an abandoned turn never touch the
   * store.
   */
  tutorSendStream: async (
    input: { sessionId: string; message: string; requestId?: string },
    onEvent: (event: AiStreamEvent) => void,
  ): Promise<TutorTurn> => {
    const requestId =
      input.requestId ?? `tutor-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const { listen } = await import("@tauri-apps/api/event");
    let cancelled = false;
    const unlisten = await listen<{ requestId: string; event: AiStreamEvent }>(
      "tutor_stream",
      (event) => {
        if (!cancelled && event.payload.requestId === requestId) onEvent(event.payload.event);
      },
    );
    const cancel = () => {
      if (cancelled) return;
      cancelled = true;
      unlisten();
      tutorStreamCancels.delete(requestId);
    };
    tutorStreamCancels.set(requestId, cancel);
    try {
      return await call<TutorTurn>("tutor_send_stream", { input: { ...input, requestId } });
    } finally {
      cancel();
    }
  },

  /* jobs */
  jobGet: (id: string) => call<Job>("job_get", { id }),
  jobList: () => call<Job[]>("job_list"),
  jobCancel: (id: string) => call<Job>("job_cancel", { id }),
};

const TERMINAL = ["completed", "failed", "cancelled"];

export async function watchJob(id: string, onUpdate: (job: Job) => void): Promise<Job> {
  let failures = 0;
  for (;;) {
    let job: Job;
    try {
      job = await api.jobGet(id);
      failures = 0;
    } catch (error) {
      if (++failures >= 5) throw error;
      await new Promise((resolve) => window.setTimeout(resolve, Math.min(4000, 500 * 2 ** (failures - 1))));
      continue;
    }
    onUpdate(job);
    if (TERMINAL.includes(job.status)) {
      return job;
    }
    await new Promise((resolve) => window.setTimeout(resolve, 220));
  }
}
