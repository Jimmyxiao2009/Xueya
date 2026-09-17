import { describe, expect, it } from "vitest";
import { detailDrag, swipeTiming } from "@/lib/swipeEngine";
import { completedPath, roadGeometry } from "@/lib/roadPath";
import { lessonCardClip } from "@/lib/lessonTransition";
import { nodeFace, routeGlyph, chapterTitle } from "@/lib/format";

const box = (top: number, left: number, width: number, height: number) => ({
  top,
  left,
  right: left + width,
  bottom: top + height,
  width,
  height,
});

describe("roadmap geometry", () => {
  it("reproduces the prototype's hard-coded six-node canvas", () => {
    const geometry = roadGeometry(6);
    // The prototype shipped .path{height:800px}, .road{height:923px} and
    // .endnote{top:845px} for exactly six stops.
    expect(geometry.pathHeight).toBe(800);
    expect(geometry.roadHeight).toBe(923);
    expect(geometry.endnoteTop).toBe(845);
    expect(geometry.viewBox).toBe("0 0 400 800");
    expect(geometry.stopTop(0)).toBe(14);
    expect(geometry.stopTop(5)).toBe(714);
  });

  it("scales the canvas with the route length", () => {
    const short = roadGeometry(3);
    expect(short.pathHeight).toBe(68 + 2 * 140 + 32);
    expect(roadGeometry(1).pathHeight).toBe(120);
    expect(roadGeometry(0).pathHeight).toBe(120);
  });

  it("alternates node columns and grows one segment per edge", () => {
    const geometry = roadGeometry(6);
    const segments = geometry.full.match(/[MS][^MS]*/g) ?? [];
    expect(segments).toHaveLength(5);
    expect(geometry.full.startsWith("M68 68C")).toBe(true);
    // Node 1 sits in the right column, node 2 back in the left one.
    expect(geometry.full).toContain("332 208");
    expect(geometry.full).toContain("68 348");
  });

  it("only paints the travelled part of the path", () => {
    const geometry = roadGeometry(6);
    const all = geometry.full.match(/[MS][^MS]*/g) ?? [];
    expect(completedPath(geometry, 0)).toBe("");
    expect(completedPath(geometry, 3)).toBe(all.slice(0, 3).join(""));
    // Never overshoots the available segments.
    expect(completedPath(geometry, 99)).toBe(geometry.full);
  });
});

describe("weighted drag physics", () => {
  it("follows the finger one-to-one up to the lead distance", () => {
    expect(detailDrag(12, 400)).toEqual({ offset: 12, slope: 1 });
    expect(detailDrag(-12, 400)).toEqual({ offset: -12, slope: 1 });
  });

  it("is always monotonic and never exceeds the travel cap", () => {
    let previous = 0;
    for (let raw = 0; raw <= 4000; raw += 7) {
      const { offset } = detailDrag(raw, 400);
      expect(offset).toBeGreaterThanOrEqual(previous);
      previous = offset;
    }
    expect(detailDrag(4000, 400).offset).toBeCloseTo(400 * 0.94, 6);
  });

  it("resists harder across the detent band than after it", () => {
    const width = 400;
    const beforeDetent = detailDrag(width * 0.31 - 1, width).slope;
    const inDetent = detailDrag(width * 0.38, width).slope;
    const afterDetent = detailDrag(width * 0.8, width).slope;
    expect(beforeDetent).toBe(0.82);
    expect(inDetent).toBe(0.4);
    expect(afterDetent).toBe(0.7);
    // The detent compresses motion: a long drag advances the card slowly.
    expect(detailDrag(width * 0.43, width).offset).toBeLessThan(width * 0.43);
  });

  it("uses the prototype's release timing curve", () => {
    expect(swipeTiming(0)).toEqual({
      duration: 720,
      easing: "cubic-bezier(.14,.68,.16,1)",
      fill: "both",
    });
    expect(Number(swipeTiming(200, 0).duration)).toBe(820 + 200 * 0.24);
    // A fast flick is quicker, and the whole curve stays inside 560..1000ms.
    expect(Number(swipeTiming(200, 3).duration)).toBeLessThan(Number(swipeTiming(200, 0).duration));
    expect(Number(swipeTiming(0, 99).duration)).toBe(560);
    expect(Number(swipeTiming(9999, 0).duration)).toBe(1000);
  });
});

describe("lesson clip transition", () => {
  const frame = box(0, 0, 480, 900);

  it("clips to the tapped node card plus its breathing room", () => {
    const clip = lessonCardClip(box(300, 40, 280, 108), frame);
    expect(clip).toBe("inset(294px 152px 486px 32px round 24px)");
  });

  it("never produces a negative inset for a node at the screen edge", () => {
    const clip = lessonCardClip(box(0, 0, 480, 900), frame);
    expect(clip).toBe("inset(0px 0px 0px 0px round 24px)");
    expect(clip).not.toContain("-");
  });
});

describe("node faces", () => {
  const base = {
    index: 0,
    kind: "lesson",
    done: false,
    current: false,
    estimatedMinutes: 15,
    conceptCount: 2,
  };

  it("labels completed, current, review and quiz nodes distinctly", () => {
    // Done and current keep the default node colouring but change their copy.
    expect(nodeFace({ ...base, done: true })).toMatchObject({
      tag: "已掌握",
      tone: "",
      icon: "check",
    });
    expect(nodeFace({ ...base, current: true }).tag).toBe("从这里继续");
    // Inserted review and quiz nodes are tinted so they read as different work.
    expect(nodeFace({ ...base, kind: "review", index: 2 })).toMatchObject({
      tag: "为你插入 · 复习",
      tone: "purple",
      icon: "review",
    });
    expect(nodeFace({ ...base, kind: "quiz", index: 4 })).toMatchObject({
      tag: "检查理解 · 小测",
      tone: "orange",
      icon: "quiz",
    });
    expect(nodeFace(base)).toMatchObject({ tag: "课程 01", tone: "" });
  });

  it("shows a tag, an icon and meta copy for every state", () => {
    for (const kind of ["lesson", "review", "quiz"]) {
      for (const state of [{}, { done: true }, { current: true }]) {
        const face = nodeFace({ ...base, kind, ...state });
        expect(face.tag).toBeTruthy();
        expect(face.icon).toBeTruthy();
        expect(face.meta).toBeTruthy();
      }
    }
  });
});

describe("route presentation helpers", () => {
  it("maps route kinds onto their glyphs and chapter names", () => {
    expect(routeGlyph("geometry")).toBeTruthy();
    expect(routeGlyph("unknown-kind")).toBeTruthy();
    // Chapters are counted from the second node onwards.
    expect(chapterTitle("review", 2)).toBe("第三阶段");
    expect(chapterTitle("lesson", 0)).toBe("第一章 · 走向无穷远");
  });
});
