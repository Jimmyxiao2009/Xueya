import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

/**
 * The frontend and the Rust backend only agree by convention: `api.ts` names a
 * command as a string and `lib.rs` lists the same string in
 * `generate_handler!`. Nothing in either language checks the other, so a typo
 * would only show up as a runtime error in the real app — after the WebView has
 * already booted. These tests are that check.
 */
const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

const read = (relative: string) => readFileSync(join(root, relative), "utf8");

function frontendCommands(): Set<string> {
  const source = read("src/api.ts");
  // `[\s\S]*?` up to `>("` so nested generics like Array<{...}> still match.
  return new Set(
    [...source.matchAll(/call<[\s\S]*?>\(\s*"([a-z_]+)"/g)].map((match) => match[1]),
  );
}

function backendCommands(): Set<string> {
  const source = read("src-tauri/src/lib.rs");
  const start = source.indexOf("generate_handler![");
  const block = source.slice(start, source.indexOf("]);", start));
  return new Set([...block.matchAll(/^\s*([a-z_]+),/gm)].map((match) => match[1]));
}

function fakeCommands(): Set<string> {
  const source = read("src/test/fakeBackend.ts");
  return new Set([...source.matchAll(/case "([a-z_]+)"/g)].map((match) => match[1]));
}

/**
 * Commands the backend exposes that no screen calls yet. They are real and
 * typed; keeping the list here means adding another one is a deliberate act.
 */
const BACKEND_ONLY = [
  "job_subscribe", // the frontend polls job_get instead
  "provider_stream_chat", // dev/streaming shell hook, not a screen yet
  "knowledge_get_related", // reserved for a future related-items view
  "knowledge_retrieval_debug", // dev-only hybrid retrieval inspector
  "search_web", // called through sourceStore.collect, not directly by a screen
  "source_import_files", // the UI imports bytes or a URL
  "source_summarize", // backend-only cache until the materials tab exposes it
];

describe("ipc contract", () => {
  it("only calls commands the Rust handler actually registers", () => {
    const called = frontendCommands();
    const handled = backendCommands();
    // Guard the guard: if either parser silently stopped matching, the set
    // differences below would be empty and these tests would prove nothing.
    expect(called.size).toBeGreaterThan(30);
    expect(handled.size).toBeGreaterThan(30);
    expect([...called].filter((name) => !handled.has(name))).toEqual([]);
  });

  it("has no backend command the frontend forgot about", () => {
    const called = frontendCommands();
    const orphans = [...backendCommands()]
      .filter((name) => !called.has(name) && !BACKEND_ONLY.includes(name))
      .sort();
    expect(orphans).toEqual([]);
  });

  it("keeps the fake backend complete, so no test can silently no-op", () => {
    const faked = fakeCommands();
    const missing = [...frontendCommands()].filter((name) => !faked.has(name)).sort();
    expect(missing).toEqual([]);
  });
});
