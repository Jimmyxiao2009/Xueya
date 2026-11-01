import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it } from "vitest";
import { ThemeChoices } from "@/components/onboarding/OnboardingGate";
import { THEME_KEY, useThemeStore } from "@/stores/themeStore";

beforeEach(() => {
  useThemeStore.getState().chooseMotif("buoy");
  useThemeStore.getState().chooseIcons("buoy");
  useThemeStore.getState().choose("light");
});

it("switches the existing visual style without forcing a color scheme", () => {
  render(<ThemeChoices />);
  fireEvent.click(screen.getByRole("button", { name: "灯塔雾海" }));
  expect(document.documentElement.dataset.motif).toBe("lighthouse");
  expect(document.documentElement.dataset.theme).toBe("light");
  fireEvent.click(screen.getByRole("button", { name: /夜色书房/ }));
  expect(document.documentElement.dataset.motif).toBe("lighthouse");
  expect(document.documentElement.dataset.theme).toBe("dark");
  fireEvent.click(screen.getByRole("button", { name: "浮标暖纸" }));
  expect(document.documentElement.dataset.theme).toBe("dark");
  expect(JSON.parse(localStorage.getItem(THEME_KEY)!)).toEqual({visualMotif:"buoy",iconMotif:"buoy",colorScheme:"dark"});
});

it("keeps follow-system selected when the motif changes", () => {
  render(<ThemeChoices />);
  fireEvent.click(screen.getByRole("button", { name: "跟随系统" }));
  fireEvent.click(screen.getByRole("button", { name: "灯塔雾海" }));
  expect(screen.getByRole("button", { name: "跟随系统" })).toHaveAttribute("aria-pressed", "true");
  expect(useThemeStore.getState().colorScheme).toBe("system");
});
