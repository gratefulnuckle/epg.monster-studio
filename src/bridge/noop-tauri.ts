/** Stubs so aliased `@tauri-apps/api/core` still type-matches rare named imports. */
export function convertFileSrc(filePath: string, _protocol?: string): string {
  return filePath;
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && !!(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
}

export function transformCallback<T>(
  callback?: (response: T) => void,
  _once?: boolean,
): string {
  const id = `cb_${Math.random().toString(36).slice(2)}`;
  if (callback) {
    (window as unknown as { [k: string]: unknown })[id] = callback;
  }
  return id;
}
