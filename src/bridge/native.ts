export async function nativeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!__STUDIO_WEB__) {
    const { invoke } = await import("@studio-native/core");
    return invoke<T>(cmd, args ?? {});
  }
  throw new Error("native invoke is desktop-only");
}

export async function nativeListen<T>(
  event: string,
  handler: (ev: { payload: T }) => void,
): Promise<() => void> {
  if (!__STUDIO_WEB__) {
    const { listen } = await import("@studio-native/event");
    return listen(event, handler);
  }
  return () => undefined;
}

export async function nativeEmit(event: string, payload?: unknown): Promise<void> {
  if (!__STUDIO_WEB__) {
    const { emit } = await import("@studio-native/event");
    await emit(event, payload);
  }
}
