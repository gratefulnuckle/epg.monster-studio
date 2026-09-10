import { isRemoteConnected, isWebHost } from "./client";
import { httpListen } from "./http";
import { nativeEmit, nativeListen } from "./native";

export async function listen<T>(event: string, handler: (ev: { payload: T }) => void): Promise<() => void> {
  if (isWebHost() || isRemoteConnected()) return httpListen(event, handler);
  return nativeListen(event, handler);
}

export async function emit(event: string, payload?: unknown): Promise<void> {
  if (isWebHost()) return;
  await nativeEmit(event, payload);
}
