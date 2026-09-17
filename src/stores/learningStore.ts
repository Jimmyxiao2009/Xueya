import { create } from "zustand";
import {
  api,
  errorOf,
  type AdaptationEvent,
  type Lesson,
  type QuizQuestion,
  type LearningEvaluation,
  type QuizResult,
} from "@/api";

export type MessageKind = "text" | "summary" | "user" | "answer";

export interface ChatMessage {
  id: string;
  role: "assistant" | "user";
  kind: MessageKind;
  text: string;
  note?: string;
}

export interface QuizDraft {
  selected: string[];
  text: string;
  photo: string | null;
  result: QuizResult | null;
  submitting: boolean;
}

interface LearningState {
  sessionId: string | null;
  nodeId: string | null;
  lesson: Lesson | null;
  loading: boolean;
  error: string | null;
  starting: boolean;
  /** How many questions have been closed out. */
  answered: number;
  messages: ChatMessage[];
  draft: QuizDraft;
  complete: boolean;
  planExpanded: boolean;
  planPulse: number;
  lastAdaptation: AdaptationEvent | null;
  /** The model's verdict on a free-text answer, once it has been asked for. */
  evaluation: LearningEvaluation | null;
  reviewing: boolean;

  start: (nodeId: string) => Promise<boolean>;
  reset: () => void;
  togglePlan: () => void;
  selectOption: (option: string) => void;
  setText: (text: string) => void;
  setPhoto: (photo: string | null) => void;
  submit: () => Promise<void>;
  advance: () => void;
  send: (text: string) => Promise<void>;
  review: () => Promise<void>;
  finish: () => Promise<void>;
}

const emptyDraft: QuizDraft = {
  selected: [],
  text: "",
  photo: null,
  result: null,
  submitting: false,
};

let messageSeq = 0;
const nextId = () => `m${++messageSeq}`;

export function currentQuestion(state: {
  lesson: Lesson | null;
  answered: number;
}): QuizQuestion | null {
  if (!state.lesson) return null;
  return state.lesson.questions[state.answered] ?? null;
}

/**
 * The prototype lit its four-step plan as questions were answered: three lit
 * while working, the fourth only when the whole check set was done.
 */
/**
 * A submitted answer only ends the question when it was right, or when it could
 * not be graded locally (short / photo). A wrong objective answer is retried —
 * the roadmap records the mistake, the learner is not blocked by it.
 */
export function canAdvance(result: QuizResult | null): boolean {
  return Boolean(result && (result.correct || !result.graded));
}

export function completedSteps(state: {
  lesson: Lesson | null;
  answered: number;
  complete: boolean;
}): number {
  const total = state.lesson?.steps.length ?? 0;
  if (total === 0) return 0;
  if (state.complete) return total;
  return Math.min(total - 1, state.answered);
}

export const useLearningStore = create<LearningState>((set, get) => ({
  sessionId: null,
  nodeId: null,
  lesson: null,
  loading: false,
  error: null,
  starting: false,
  answered: 0,
  messages: [],
  draft: emptyDraft,
  complete: false,
  planExpanded: true,
  planPulse: 0,
  lastAdaptation: null,
  evaluation: null,
  reviewing: false,

  start: async (nodeId) => {
    set({
      starting: true,
      error: null,
      lesson: null,
      sessionId: null,
      nodeId,
      answered: 0,
      messages: [],
      draft: emptyDraft,
      complete: false,
      planExpanded: true,
      planPulse: 0,
      lastAdaptation: null,
      evaluation: null,
      reviewing: false,
    });
    try {
      const started = await api.learningStart({ nodeId });
      set({
        starting: false,
        sessionId: started.sessionId,
        lesson: started.lesson,
      });
      return true;
    } catch (error) {
      set({ starting: false, error: errorOf(error).message });
      return false;
    }
  },

  reset: () =>
    set({
      sessionId: null,
      nodeId: null,
      lesson: null,
      loading: false,
      error: null,
      starting: false,
      answered: 0,
      messages: [],
      draft: emptyDraft,
      complete: false,
      planExpanded: true,
      planPulse: 0,
      lastAdaptation: null,
      evaluation: null,
      reviewing: false,
    }),

  togglePlan: () => set((state) => ({ planExpanded: !state.planExpanded })),

  selectOption: (option) =>
    set((state) => {
      if (canAdvance(state.draft.result)) return state;
      const question = currentQuestion(state);
      if (!question) return state;
      const selected =
        question.kind === "multi"
          ? state.draft.selected.includes(option)
            ? state.draft.selected.filter((value) => value !== option)
            : [...state.draft.selected, option]
          : [option];
      // Editing an answer clears the previous verdict so the card resets.
      return { draft: { ...state.draft, selected, result: null } };
    }),

  setText: (text) =>
    set((state) =>
      canAdvance(state.draft.result)
        ? state
        : { draft: { ...state.draft, text, result: null } },
    ),

  setPhoto: (photo) =>
    set((state) =>
      canAdvance(state.draft.result)
        ? state
        : { draft: { ...state.draft, photo, result: null } },
    ),

  submit: async () => {
    const state = get();
    const question = currentQuestion(state);
    const sessionId = state.sessionId;
    if (!question || !sessionId || state.draft.submitting) return;

    const answers =
      question.kind === "single" || question.kind === "multi"
        ? state.draft.selected
        : question.kind === "image"
          ? []
          : [state.draft.text];

    set({ draft: { ...state.draft, submitting: true } });
    try {
      const result = await api.quizSubmit({
        questionId: question.id,
        answer: answers,
        sessionId,
      });
      set((current) => ({
        draft: { ...current.draft, result, submitting: false },
        lastAdaptation: result.adaptation ?? current.lastAdaptation,
      }));
    } catch (error) {
      set((current) => ({
        draft: { ...current.draft, submitting: false },
        error: errorOf(error).message,
      }));
    }
  },

  advance: () => {
    const state = get();
    const question = currentQuestion(state);
    if (!question || !state.draft.result) return;
    const lesson = state.lesson;
    if (!lesson) return;

    const answered = state.answered + 1;
    const complete = answered >= lesson.questions.length;
    const summary: ChatMessage = {
      id: nextId(),
      role: "assistant",
      kind: "summary",
      text: question.prompt,
      note: state.draft.result.graded
        ? "已完成理解检查"
        : "作答已提交 · 未自动评分",
    };
    const messages = [...state.messages, summary];

    if (!complete) {
      const nextKind = lesson.questions[answered]?.kind ?? "single";
      const prompt =
        PROMPTS[nextKind] ??
        "继续下一题，看看这个概念是不是真的稳了。";
      messages.push({ id: nextId(), role: "assistant", kind: "text", text: prompt });
    }

    set({
      answered,
      complete,
      messages,
      draft: emptyDraft,
      evaluation: null,
      reviewing: false,
      planPulse: state.planPulse + 1,
    });
  },

  send: async (text) => {
    const trimmed = text.trim();
    const sessionId = get().sessionId;
    if (!trimmed || !sessionId) return;
    set((state) => ({
      messages: [
        ...state.messages,
        { id: nextId(), role: "user", kind: "user", text: trimmed },
      ],
    }));
    try {
      const turn = await api.tutorSend({ sessionId, message: trimmed });
      set((state) => ({
        messages: [
          ...state.messages,
          {
            id: nextId(),
            role: "assistant",
            kind: "text",
            text: turn.message,
            // A tutor turn can change the roadmap. Saying so here is the
            // difference between adapting and silently rewriting things.
            note:
              turn.adaptation?.kind === "review_inserted"
                ? "已在你的路线里插入一节针对性加练"
                : turn.generatedBy === "mock"
                  ? "离线回答 · 未接入模型"
                  : undefined,
          },
        ],
        lastAdaptation: turn.adaptation ?? state.lastAdaptation,
      }));
    } catch (error) {
      set((state) => ({
        messages: [
          ...state.messages,
          {
            id: nextId(),
            role: "assistant",
            kind: "text",
            text: `没能连上模型：${errorOf(error).message}`,
          },
        ],
      }));
    }
  },

  /**
   * Ask the model to grade a free-text answer. Objective answers are already
   * graded in Rust; this is the door for the ones string matching cannot judge.
   */
  review: async () => {
    const state = get();
    const sessionId = state.sessionId;
    const answer = state.draft.text.trim();
    if (!sessionId || !answer || state.reviewing || state.evaluation) return;
    set({ reviewing: true });
    try {
      const evaluation = await api.learningRespond({ sessionId, answer });
      set((current) => ({
        reviewing: false,
        evaluation,
        messages: [
          ...current.messages,
          {
            id: nextId(),
            role: "assistant",
            kind: "text",
            text: evaluation.feedback,
            note: `模型评阅 · 正确度 ${Math.round(evaluation.correctness * 100)}%`,
          },
        ],
      }));
    } catch (error) {
      set((current) => ({
        reviewing: false,
        messages: [
          ...current.messages,
          {
            id: nextId(),
            role: "assistant",
            kind: "text",
            text: `没能完成评阅：${errorOf(error).message}`,
          },
        ],
      }));
    }
  },

  finish: async () => {
    const { sessionId, nodeId } = get();
    if (!sessionId || !nodeId) return;
    try {
      await api.learningFinish({ sessionId });
    } catch {
      // A session that is already closed is not an error worth showing.
    }
  },
}));

const PROMPTS: Record<string, string> = {
  single: "很好。先确认一个最基础的判断。",
  multi: "现在换几个说法，看看你能不能辨别它们。",
  fill: "接着，把这个关键数值补完整。",
  short: "你已经抓住了主要概念。试着用自己的话讲给我听。",
  image: "最后，在纸上画一画。拍照给我，我们一起检查这张图。",
};
