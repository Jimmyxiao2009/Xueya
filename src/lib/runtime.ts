/** Browser demo settings are owned by the server administrator. */
export function isManagedBrowser(): boolean {
  return import.meta.env.MODE !== "test" && !("__TAURI_INTERNALS__" in window);
}
