import { isLocalCommand, isRemoteConnected, isWebHost } from "./client";
import { httpInvoke } from "./http";
import { nativeInvoke } from "./native";

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (__STUDIO_WEB__ && cmd === "play_url") {
    const { playInBrowser } = await import("../web-player");
    const sid = args?.sourceId ?? args?.source_id;
    playInBrowser(String(args?.url ?? ""), typeof sid === "string" ? sid : undefined);
    return undefined as T;
  }
  if (isWebHost()) return httpInvoke<T>(cmd, args);
  if (isRemoteConnected() && cmd === "studio_tools_status") {
    const remote = await httpInvoke<{ ffmpeg?: boolean; ffprobe?: boolean; mpv?: boolean; vlc?: boolean }>(
      cmd,
      args,
    );
    const local = await nativeInvoke<{ ffmpeg?: boolean; ffprobe?: boolean; mpv?: boolean; vlc?: boolean }>(
      cmd,
      args,
    );
    return {
      ffmpeg: !!remote.ffmpeg,
      ffprobe: !!remote.ffprobe,
      mpv: !!local.mpv,
      vlc: !!local.vlc,
    } as T;
  }
  if (isRemoteConnected() && !isLocalCommand(cmd)) {
    return httpInvoke<T>(cmd, args);
  }
  return nativeInvoke<T>(cmd, args);
}

export async function invokeLocal<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isWebHost()) return httpInvoke<T>(cmd, args);
  return nativeInvoke<T>(cmd, args);
}

export { convertFileSrc, isTauri, transformCallback } from "./noop-tauri";
