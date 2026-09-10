const STORE = "studio-client-v1";

export type ClientPrefs = {
  url: string;
  apiKey: string;
  connected: boolean;
  defaultPlayer: number;
  mpvPath: string;
  vlcPath: string;
  checkForAppUpdates: boolean;
};

const empty: ClientPrefs = {
  url: "",
  apiKey: "",
  connected: false,
  defaultPlayer: 0,
  mpvPath: "",
  vlcPath: "",
  checkForAppUpdates: false,
};

let cache: ClientPrefs | null = null;

export function isWebHost(): boolean {
  if (typeof __STUDIO_WEB__ !== "undefined" && __STUDIO_WEB__) return true;
  return typeof document !== "undefined" && document.documentElement.classList.contains("studio-web");
}

export function isTauri(): boolean {
  return typeof window !== "undefined" && !!(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
}

export function loadClient(): ClientPrefs {
  if (cache) return cache;
  try {
    const raw = localStorage.getItem(STORE);
    if (raw) cache = { ...empty, ...(JSON.parse(raw) as Partial<ClientPrefs>) };
  } catch {
    /* ignore */
  }
  if (!cache) cache = { ...empty };
  return cache;
}

export function saveClient(next: Partial<ClientPrefs>): ClientPrefs {
  const cur = { ...loadClient(), ...next };
  cache = cur;
  try {
    localStorage.setItem(STORE, JSON.stringify(cur));
  } catch {
    /* quota */
  }
  return cur;
}

export function isRemoteConnected(): boolean {
  const c = loadClient();
  return isTauri() && c.connected && !!c.url && !!c.apiKey;
}

export function remoteBase(): string {
  return normalizeStudioUrl(loadClient().url);
}

export function remoteApiKey(): string {
  return loadClient().apiKey.trim();
}

export function normalizeStudioUrl(raw: string): string {
  let u = raw.trim().replace(/\/+$/, "");
  if (!u) return "";
  if (!/^[a-z][a-z0-9+.-]*:\/\//i.test(u)) u = "http://" + u;
  return u;
}

export async function probeRemote(url: string, apiKey: string): Promise<{ ok: boolean; error?: string }> {
  const base = normalizeStudioUrl(url);
  if (!base) return { ok: false, error: "Enter the studio server URL." };
  if (!apiKey.trim()) return { ok: false, error: "Paste the API key from the server." };
  try {
    const r = await fetch(`${base}/api/session`, {
      method: "GET",
      credentials: "omit",
      headers: { Authorization: `Bearer ${apiKey.trim()}` },
    });
    const j = (await r.json().catch(() => ({}))) as { ok?: boolean; error?: string; kind?: string };
    if (!r.ok || !j.ok) return { ok: false, error: j.error || `Could not connect (${r.status}).` };
    return { ok: true };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}

export async function connectRemote(url: string, apiKey: string): Promise<void> {
  const probe = await probeRemote(url, apiKey);
  if (!probe.ok) throw new Error(probe.error || "Connect failed.");
  saveClient({ url: normalizeStudioUrl(url), apiKey: apiKey.trim(), connected: true });
  sessionStorage.setItem("studio-live", "1");
  window.location.reload();
}

export function disconnectRemote(): void {
  saveClient({ connected: false });
  sessionStorage.setItem("studio-live", "1");
  window.location.reload();
}

const LOCAL_ALWAYS = new Set([
  "play_url",
  "promote_main_window",
  "mark_tray_state",
  "mark_clean_exit",
  "log_heartbeat",
  "open_epg_catalog_window",
  "open_source_search_window",
  "open_folder",
  "open_latest_release",
  "host_info",
  "consume_pending_crash",
  "write_crash_report",
  "post_issue",
  "tools_missing",
  "tools_ensure",
  "check_app_update",
  "check_studio_update",
  "splash_checks",
]);

export function isLocalCommand(cmd: string): boolean {
  if (LOCAL_ALWAYS.has(cmd)) return true;
  if (cmd.startsWith("ghoul")) return true;
  return false;
}
