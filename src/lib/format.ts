import type { IconName } from "@/lib/icons";

export function padIndex(value: number): string {
  return String(value + 1).padStart(2, "0");
}

export function percent(value: number): number {
  return Math.round(Math.max(0, Math.min(1, value)) * 100);
}

export function sizeLabel(bytes: number): string {
  return bytes >= 1048576
    ? `${(bytes / 1048576).toFixed(1)} MB`
    : `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

export function hostnameOf(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

export function relativeTime(iso?: string | null): string {
  if (!iso) return "";
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "";
  const minutes = Math.round((Date.now() - then) / 60000);
  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  return `${Math.round(hours / 24)} 天前`;
}

export function dueLabel(iso?: string | null): string {
  if (!iso) return "尚未安排复习";
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "尚未安排复习";
  const hours = (then - Date.now()) / 3600000;
  if (hours <= 0) return "现在就可以复习";
  if (hours < 24) return `${Math.round(hours)} 小时后复习`;
  return `${Math.round(hours / 24)} 天后复习`;
}

/**
 * The prototype described every node with a one-line tag and a one-line meta.
 * These are derived from the real node now instead of being hard-coded.
 */
export interface NodeFace {
  tag: string;
  tone: "" | "purple" | "orange";
  icon: IconName;
  meta: string;
}

const LESSON_GLYPHS: IconName[] = ["book", "infinity", "review", "line", "quiz", "grid"];

export function nodeFace(input: {
  index: number;
  kind: string;
  done: boolean;
  current: boolean;
  estimatedMinutes: number;
  conceptCount: number;
}): NodeFace {
  const { index, kind, done, current, estimatedMinutes, conceptCount } = input;

  if (done) {
    return { tag: "已掌握", tone: "", icon: "check", meta: metaFor(kind, estimatedMinutes, conceptCount) };
  }
  if (current) {
    return { tag: "从这里继续", tone: "", icon: glyphFor(kind, index), meta: metaFor(kind, estimatedMinutes, conceptCount) };
  }
  if (kind === "review") {
    return { tag: "为你插入 · 复习", tone: "purple", icon: "review", meta: metaFor(kind, estimatedMinutes, conceptCount) };
  }
  if (kind === "quiz") {
    return { tag: "检查理解 · 小测", tone: "orange", icon: "quiz", meta: metaFor(kind, estimatedMinutes, conceptCount) };
  }
  return {
    tag: `课程 ${padIndex(index)}`,
    tone: "",
    icon: glyphFor(kind, index),
    meta: metaFor(kind, estimatedMinutes, conceptCount),
  };
}

function glyphFor(kind: string, index: number): IconName {
  if (kind === "review") return "review";
  if (kind === "quiz") return "quiz";
  if (kind === "practice") return "line";
  return LESSON_GLYPHS[index % LESSON_GLYPHS.length] ?? "book";
}

function metaFor(kind: string, minutes: number, conceptCount: number): string {
  if (kind === "review") return `${minutes} 分钟 · 巩固关键概念`;
  if (kind === "quiz") return `${minutes} 分钟 · 多种题型`;
  if (kind === "practice") return `${minutes} 分钟 · 动手练习`;
  return `${Math.max(1, conceptCount)} 个知识点 · ${minutes} 分钟`;
}

export function chapterTitle(kind: string, index: number): string {
  if (index > 0) return `第${["", "一", "二", "三", "四", "五", "六", "七", "八"][index + 1] ?? index + 1}阶段`;
  return kind === "algebra" ? "第一章 · 建立直觉" : "第一章 · 走向无穷远";
}

export function routeGlyph(kind: string): IconName {
  return kind === "geometry" ? "orbit" : "grid";
}

export function depthLabel(depth?: string | null): string {
  if (depth === "deep") return "深入";
  if (depth === "practical") return "实践";
  return "概览";
}

export function experienceLabel(level?: string | null): string {
  if (level === "advanced") return "已有基础";
  if (level === "some_experience") return "接触过";
  return "从零开始";
}
