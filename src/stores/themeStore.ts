import { create } from "zustand";

export type Theme = "light" | "dark";
export type ColorScheme = Theme | "system";
export type VisualMotif = "lighthouse" | "buoy";
export interface ThemePreferences { visualMotif: VisualMotif; iconMotif: VisualMotif; colorScheme: ColorScheme }
export const THEME_KEY = "verruckt.theme.v2";
export function parseTheme(value: string | null, legacy: string | null = null): ThemePreferences {
  try {
    const parsed = JSON.parse(value ?? "null");
    if (parsed && ["lighthouse", "buoy"].includes(parsed.visualMotif) && ["light", "dark", "system"].includes(parsed.colorScheme)) return { visualMotif: parsed.visualMotif, iconMotif: ["lighthouse", "buoy"].includes(parsed.iconMotif) ? parsed.iconMotif : parsed.visualMotif, colorScheme: parsed.colorScheme };
  } catch { /* Invalid preferences fall back safely. */ }
  return { visualMotif: "buoy", iconMotif: "buoy", colorScheme: legacy === "dark" || legacy === "light" ? legacy : "system" };
}
export function resolveScheme(scheme: ColorScheme, osDark: boolean): Theme {
  return scheme === "system" ? (osDark ? "dark" : "light") : scheme;
}
const media = typeof matchMedia === "function" ? matchMedia("(prefers-color-scheme: dark)") : null;
function saved() {
  try { return parseTheme(localStorage.getItem(THEME_KEY), localStorage.getItem("orbit.theme")); }
  catch { return parseTheme(null); }
}
function apply(prefs: ThemePreferences): Theme {
  const resolved = resolveScheme(prefs.colorScheme, media?.matches ?? false);
  document.documentElement.dataset.theme = resolved;
  document.documentElement.dataset.motif = prefs.visualMotif;
  document.documentElement.dataset.icons = prefs.iconMotif;
  return resolved;
}
const initial = saved();
const resolved = apply(initial);
interface ThemeState extends ThemePreferences {
  theme: Theme; resolvedColorScheme: Theme; chosen: boolean;
  choose: (scheme: ColorScheme) => void;
  chooseMotif: (motif: VisualMotif) => void;
  chooseIcons: (motif: VisualMotif) => void;
}
export const useThemeStore = create<ThemeState>((set, get) => {
  function update(prefs: ThemePreferences) {
    const theme = apply(prefs);
    try { localStorage.setItem(THEME_KEY, JSON.stringify(prefs)); localStorage.setItem("orbit.theme", theme); }
    catch { /* Private storage must never gate learning. */ }
    set({ ...prefs, theme, resolvedColorScheme: theme, chosen: true });
  }
  return { ...initial, theme: resolved, resolvedColorScheme: resolved, chosen: false,
    choose: (colorScheme) => update({ visualMotif: get().visualMotif, iconMotif: get().iconMotif, colorScheme }),
    chooseMotif: (visualMotif) => update({ visualMotif, iconMotif: get().iconMotif, colorScheme: get().colorScheme }),
    chooseIcons: (iconMotif) => update({ visualMotif: get().visualMotif, iconMotif, colorScheme: get().colorScheme }) };
});
media?.addEventListener("change", () => {
  const theme = apply(useThemeStore.getState());
  useThemeStore.setState({ theme, resolvedColorScheme: theme });
});
