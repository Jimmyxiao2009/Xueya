import type { RouteNode } from "@/api";

export interface NodeMeta {
  concepts: string[];
  evidenceChunkIds: string[];
  knowledgePointId?: string;
  origin?: string;
  insertedFor?: string;
  reason?: string;
  contentStatus?: "pending" | "generating" | "ready" | "failed";
}

/** Node metadata is written by the planner; the UI reads it defensively. */
export function nodeMeta(node: RouteNode | undefined): NodeMeta {
  const fallback: NodeMeta = { concepts: [], evidenceChunkIds: [] };
  if (!node?.metadataJson) return fallback;
  try {
    const parsed = JSON.parse(node.metadataJson) as Partial<NodeMeta>;
    return {
      concepts: Array.isArray(parsed.concepts) ? parsed.concepts : [],
      evidenceChunkIds: Array.isArray(parsed.evidenceChunkIds)
        ? parsed.evidenceChunkIds
        : [],
      knowledgePointId: parsed.knowledgePointId,
      origin: parsed.origin,
      insertedFor: parsed.insertedFor,
      reason: parsed.reason,
      contentStatus: parsed.contentStatus,
    };
  } catch {
    return fallback;
  }
}

export function contentStatusLabel(node: RouteNode): string | null {
  switch (nodeMeta(node).contentStatus) {
    case "pending": return "内容排队准备中";
    case "generating": return "正在准备讲解与练习";
    case "failed": return "内容生成失败，打开后重试";
    default: return null;
  }
}

export interface RouteMeta {
  learningObjectives: string[];
  prerequisites: string[];
  sourceIds: string[];
  origin?: string;
}

export function routeMeta(metadataJson: string | undefined): RouteMeta {
  const fallback: RouteMeta = { learningObjectives: [], prerequisites: [], sourceIds: [] };
  if (!metadataJson) return fallback;
  try {
    const parsed = JSON.parse(metadataJson) as Partial<RouteMeta>;
    return {
      learningObjectives: Array.isArray(parsed.learningObjectives)
        ? parsed.learningObjectives
        : [],
      prerequisites: Array.isArray(parsed.prerequisites) ? parsed.prerequisites : [],
      sourceIds: Array.isArray(parsed.sourceIds) ? parsed.sourceIds : [],
      origin: parsed.origin,
    };
  } catch {
    return fallback;
  }
}

/** `origin` tells the learner whether a plan came from a model or a template. */
export function originLabel(origin?: string | null): string {
  if (origin === "official_grounded" || origin === "byok_grounded") return "基于你的资料生成";
  if (origin === "official_general" || origin === "byok_general") return "AI 生成 · 通用知识";
  if (origin === "ai") return "由模型生成";
  if (origin === "adaptation") return "由你的答题结果插入";
  if (origin === "mock") return "离线回退 · 非 AI";
  return "离线回退 · 内置讲解";
}
