import { isRemoteConnected, isWebHost, remoteApiKey, remoteBase } from "./client";

function apiRoot(): string {
  if (isWebHost()) return "";
  return remoteBase();
}

function apiHeaders(json = true): HeadersInit {
  const h: Record<string, string> = {};
  if (json) h["Content-Type"] = "application/json";
  const key = isWebHost() ? "" : remoteApiKey();
  if (key) h.Authorization = `Bearer ${key}`;
  return h;
}

function cred(): RequestCredentials {
  return isWebHost() ? "same-origin" : "omit";
}

async function pickAndUpload(accept: string): Promise<string | null> {
  const input = document.createElement("input");
  input.type = "file";
  input.accept = accept;
  const file = await new Promise<File | null>((resolve) => {
    input.onchange = () => resolve(input.files?.[0] ?? null);
    input.oncancel = () => resolve(null);
    input.click();
  });
  if (!file) return null;
  const body = new FormData();
  body.append("file", file, file.name);
  const r = await fetch(`${apiRoot()}/api/upload`, {
    method: "POST",
    credentials: cred(),
    headers: apiHeaders(false),
    body,
  });
  if (!r.ok) throw new Error(await r.text());
  const j = (await r.json()) as { path: string };
  return j.path;
}

function downloadText(name: string, body: string) {
  const blob = new Blob([body], { type: "application/octet-stream" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = name;
  a.click();
  URL.revokeObjectURL(a.href);
}

export async function httpInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (cmd === "pick_source_file" || cmd === "pick_playlist_path") {
    return (await pickAndUpload(".m3u,.m3u8,.txt")) as T;
  }
  if (cmd === "import_curated") {
    const path = await pickAndUpload(".m3u,.m3u8,.txt");
    if (!path) return "cancelled" as T;
    args = { ...(args ?? {}), path };
  }
  if (cmd === "add_slate") {
    const path = await pickAndUpload(".png,.jpg,.jpeg");
    if (!path) return "cancelled" as T;
    args = { ...(args ?? {}), path };
  }
  const r = await fetch(`${apiRoot()}/api/invoke`, {
    method: "POST",
    credentials: cred(),
    headers: apiHeaders(true),
    body: JSON.stringify({ cmd, args: args ?? {} }),
  });
  const j = (await r.json()) as { result?: unknown; error?: string };
  if (!r.ok || j.error) {
    const err = j.error || r.statusText;
    if (err.startsWith("WEB_PICK:open:")) {
      const ext = err
        .slice("WEB_PICK:open:".length)
        .split(",")
        .map((e) => `.${e}`)
        .join(",");
      const path = await pickAndUpload(ext);
      if (!path) return (cmd.startsWith("pick") ? null : "cancelled") as T;
      return httpInvoke<T>(cmd, { ...(args ?? {}), path });
    }
    throw new Error(err);
  }
  const result = j.result as T & { __download?: boolean; name?: string; body?: string };
  if (result && typeof result === "object" && result.__download && result.body && result.name) {
    downloadText(result.name, result.body);
    return `Saved ${result.name}` as T;
  }
  return j.result as T;
}

export async function httpListen<T>(event: string, handler: (ev: { payload: T }) => void): Promise<() => void> {
  const ac = new AbortController();
  const run = async () => {
    const r = await fetch(`${apiRoot()}/api/events`, {
      method: "GET",
      credentials: cred(),
      headers: apiHeaders(false),
      signal: ac.signal,
    });
    if (!r.ok || !r.body) throw new Error(`events ${r.status}`);
    const reader = r.body.getReader();
    const dec = new TextDecoder();
    let buf = "";
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      buf += dec.decode(value, { stream: true });
      const parts = buf.split("\n\n");
      buf = parts.pop() ?? "";
      for (const block of parts) {
        const line = block
          .split("\n")
          .filter((l) => l.startsWith("data:"))
          .map((l) => l.slice(5).trim())
          .join("");
        if (!line) continue;
        try {
          const j = JSON.parse(line) as { event?: string; payload?: T };
          if (j.event === event) handler({ payload: j.payload as T });
        } catch {
          /* ignore */
        }
      }
    }
  };
  void run().catch((e) => {
    if ((e as { name?: string }).name === "AbortError") return;
  });
  return () => ac.abort();
}

export function usesHttpInvoke(): boolean {
  return isWebHost() || isRemoteConnected();
}
