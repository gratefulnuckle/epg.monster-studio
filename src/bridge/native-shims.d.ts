declare module "@studio-native/core" {
  export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
}

declare module "@studio-native/event" {
  export function listen<T>(
    event: string,
    handler: (ev: { payload: T }) => void,
  ): Promise<() => void>;
  export function emit(event: string, payload?: unknown): Promise<void>;
}
