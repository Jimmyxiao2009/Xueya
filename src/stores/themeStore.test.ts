import { describe, expect, it } from "vitest";
import { parseTheme, resolveScheme, useThemeStore, THEME_KEY } from "./themeStore";
describe("orthogonal persisted themes", () => {
  for (const visualMotif of ["lighthouse", "buoy"] as const) {
    for (const colorScheme of ["light", "dark", "system"] as const) {
      it(`${visualMotif} × ${colorScheme} round-trips independently`, () => {
        useThemeStore.getState().chooseMotif(visualMotif);
        useThemeStore.getState().choose(colorScheme);
        expect(parseTheme(localStorage.getItem(THEME_KEY))).toEqual({ visualMotif, iconMotif: useThemeStore.getState().iconMotif, colorScheme });
        expect(document.documentElement.dataset.motif).toBe(visualMotif);
        expect(resolveScheme(colorScheme, true)).toBe(colorScheme === "light" ? "light" : "dark");
      });
    }
  }
  it("migrates legacy colors and handles corrupt preferences", () => {
    expect(parseTheme("invalid", "dark")).toEqual({ visualMotif: "buoy", iconMotif: "buoy", colorScheme: "dark" });
    expect(parseTheme('{"visualMotif":"bad"}')).toEqual({ visualMotif: "buoy", iconMotif: "buoy", colorScheme: "system" });
    expect(parseTheme('{"visualMotif":"lighthouse","colorScheme":"dark"}')).toEqual({ visualMotif: "lighthouse", iconMotif: "lighthouse", colorScheme: "dark" });
  });
});
