import "@testing-library/jest-dom/vitest";
import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import { backend } from "./fakeBackend";

// The whole app talks to Rust through `invoke`; tests get the same contract
// backed by an in-memory model instead of a stubbed call list.
vi.mock("@tauri-apps/api/core", async () => {
  const { backend: fake } = await import("./fakeBackend");
  return {
    invoke: (command: string, args?: Record<string, unknown>) => fake.invoke(command, args),
  };
});

// jsdom has no layout engine and no Web Animations API. The app degrades to
// instant state changes when `animate` is missing, which is exactly what the
// reduced-motion path does in production.
if (!Element.prototype.animate) {
  // @ts-expect-error - deliberately minimal stand-in for tests
  Element.prototype.animate = undefined;
}

if (!window.matchMedia) {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    addListener: () => undefined,
    removeListener: () => undefined,
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}

// jsdom ships `<dialog>` without the top-layer API. Tauri's WebViews all have
// it; the shim only keeps the layer mountable inside jsdom.
if (typeof HTMLDialogElement !== "undefined") {
  const proto = HTMLDialogElement.prototype;
  if (!proto.showModal) {
    proto.showModal = function showModal(this: HTMLDialogElement) {
      this.setAttribute("open", "");
    };
  }
  if (!proto.show) {
    proto.show = function show(this: HTMLDialogElement) {
      this.setAttribute("open", "");
    };
  }
  if (!proto.close) {
    proto.close = function close(this: HTMLDialogElement) {
      this.removeAttribute("open");
      this.dispatchEvent(new Event("close"));
    };
  }
  if (!Object.getOwnPropertyDescriptor(proto, "open")) {
    Object.defineProperty(proto, "open", {
      configurable: true,
      get(this: HTMLDialogElement) {
        return this.hasAttribute("open");
      },
      set(this: HTMLDialogElement, value: boolean) {
        if (value) this.setAttribute("open", "");
        else this.removeAttribute("open");
      },
    });
  }
}

if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => undefined;
}

afterEach(() => {
  cleanup();
  backend.calls.length = 0;
});
