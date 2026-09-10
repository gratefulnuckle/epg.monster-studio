export type EngineId = "gstreamer" | "mpv-ipc" | "libmpv";

export type Availability = {
  id: EngineId;
  available: boolean;
  reason?: string | null;
};

export type Prefs = {
  engine: string;
  uaPreset: string;
  customUa: string;
  lastChannel: number;
};

export type StatusDto = {
  curatedCount: number;
  playableCount: number;
  hasKey: boolean;
  feedUrl?: string | null;
  engines: Availability[];
  prefs: Prefs;
  canMount: boolean;
  gate?: string | null;
};

export type Channel = {
  name: string;
  group: string;
  tvgId: string;
  tvgLogo: string;
  url: string;
  ua: string;
  headers: Record<string, string>;
};

export type Category = {
  title: string;
  count: number;
};

export type PrepareDto = {
  channels: Channel[];
  categories: Category[];
  engines: Availability[];
  prefs: Prefs;
  xmlOk: boolean;
  xmlMessage?: string | null;
};

export type Prog = {
  start: number;
  stop: number;
  title: string;
};

export type NowOn = {
  title: string;
  startUnix: number;
  stopUnix: number;
};

export type Snapshot = {
  loading: boolean;
  err?: string | null;
  progs: Record<string, Prog[]>;
  now: Record<string, NowOn>;
  next: Record<string, NowOn>;
};

export const UA_PRESETS: { id: string; label: string }[] = [
  { id: "tivimate", label: "TiviMate 4.6.0" },
  { id: "vlc", label: "VLC" },
  { id: "gse", label: "GSE Smart IPTV" },
  { id: "smarters", label: "IPTV Smarters" },
  { id: "browser", label: "Browser" },
  { id: "custom", label: "Custom" },
];

export const ENGINE_LABEL: Record<EngineId, string> = {
  gstreamer: "GStreamer",
  "mpv-ipc": "mpv IPC",
  libmpv: "libmpv",
};

export function hhmm(unix: number): string {
  const secs = ((unix % 86400) + 86400) % 86400;
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}

export function tvgKeys(id: string): string[] {
  const raw = id.trim();
  if (!raw) return [];
  const out: string[] = [];
  const push = (s: string) => {
    const k = s.trim().toLowerCase();
    if (k && !out.includes(k)) out.push(k);
  };
  push(raw);
  const head = raw.split(" (")[0];
  if (head !== raw) push(head);
  const lower = head.toLowerCase();
  const idx = lower.indexOf(".us_locals");
  if (idx >= 0) {
    const stem = head.slice(0, idx);
    push(`${stem}.us`);
    push(stem);
  }
  return out;
}
