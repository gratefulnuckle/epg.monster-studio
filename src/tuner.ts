import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { AppSettings, TunerProfile } from "./settings";

export type TunerStatus = {
  kind: string;
  friendlyName: string;
  enabled: boolean;
  running: boolean;
  activeConnections: number;
  maxConnections: number;
  port: number;
  baseUrl: string;
  deviceId: string;
  error?: string | null;
  statusLabel: string;
  advertisedEpg?: string;
};

export type TunerPickRow = {
  id: string;
  name: string;
  group: string;
  included: boolean;
  number?: number | null;
};

type LogLine = { at: string; kind: string; line: string };
type GraphRow = {
  kind: string;
  live: number;
  max: number;
  discover: number;
  lineup: number;
  guide: number;
  m3u: number;
  stream: number;
  notFound: number;
  bytes: number;
};
type ProbeStep = { Client: string; Name: string; Ok: boolean; Detail: string };
type ProbeReport = { Kind: string; BaseUrl: string; Steps: ProbeStep[] };

const LANDING: { kind: string; name: string; icon: string }[] = [
  { kind: "Plex", name: "Plex", icon: "/tuner/plex.svg" },
  { kind: "Jellyfin", name: "Jellyfin", icon: "/tuner/jellyfin.svg" },
  { kind: "Emby", name: "Emby", icon: "/tuner/emby.png" },
  { kind: "Iptv", name: "IPTV", icon: "/tuner/iptv.svg" },
];

export function tunerHtml(): string {
  return `
    <div id="tn-view-landing">
      <h1 class="page-title">TV Tuner</h1>
      <p class="page-sub">Choose a tuner to start, set options, and pick the channel list. Ports are 8080 Plex, 8081 Jellyfin, 8082 Emby, 8083 IPTV.</p>
      <div class="tabs-row">
        <button class="accent" id="tn-start-all">Start all enabled</button>
        <button id="tn-stop-all">Stop all</button>
        <button id="tn-log" title="Verbose tuner log">Log</button>
        <button id="tn-graphs" title="Live tuner stats">Graphs</button>
        <button id="tn-test" title="Mimic Plex / Jellyfin / Emby / TiviMate HTTP without those apps">Self-test</button>
        <span class="page-sub" id="tn-summary"></span>
      </div>
      <div class="tn-landing">
        <div class="tn-landing-row" id="tn-row-media"></div>
        <div class="tn-landing-row tn-landing-row-iptv" id="tn-row-iptv"></div>
      </div>
      <label class="check tn-disco"><input type="checkbox" id="tn-disco" /> Advertise tuners on the network (HDHomeRun UDP 65001 + SSDP). Turn on Allow LAN if Plex is another PC.</label>
      <p class="page-sub">Advertise + Allow LAN: any device on the LAN can hit the tuner HTTP ports. There is no password. Default bind is loopback until Allow LAN is on.</p>
    </div>
    <div id="tn-view-detail" class="editor-workspace" hidden>
      <div class="tabs-row">
        <button id="tn-back" type="button">Back</button>
        <span class="editor-title" id="tn-detail-title">Tuner</span>
        <button class="accent" id="tn-start" type="button">Start</button>
        <button id="tn-stop" type="button">Stop</button>
        <button id="tn-log-one" type="button">Log</button>
        <button id="tn-graphs-one" type="button">Graphs</button>
        <button id="tn-info" type="button">Info</button>
        <button id="tn-links" type="button">Open TV tuner links</button>
        <span class="page-sub" id="tn-detail-status"></span>
      </div>
      <p class="page-sub" id="tn-detail-urls" style="user-select:text;margin-bottom:8px"></p>
      <div class="tn-detail-body">
        <section class="tile tn-settings">
          <h2>Settings</h2>
          <p class="hint" id="tn-settings-hint">Saved with this tuner. Start and stop live above.</p>
          <label class="check"><input type="checkbox" id="tn-on" /> Enable this tuner</label>
          <div class="field"><label>Friendly name</label><input id="tn-name" /></div>
          <div class="field"><label>Port</label><input id="tn-port" type="number" /></div>
          <div class="field"><label>Tuner count</label><input id="tn-count" type="number" min="1" max="16" /></div>
          <label class="check"><input type="checkbox" id="tn-lan" /> Allow LAN</label>
          <p class="page-sub" id="tn-lan-warn">Trusted LAN only. Binds 0.0.0.0 with no client auth. For IPTV, keep remux on so the playlist stays on Studio URLs (remux off lists provider stream URLs).</p>
          <div id="tn-jelly-extra" hidden>
            <label class="check"><input type="checkbox" id="tn-down" /> Downspiral — one playlist + guide per group (switch lists without changing Jellyfin profiles)</label>
          </div>
          <div id="tn-iptv-extra" hidden>
            <label class="check"><input type="checkbox" id="tn-remux" /> Remux IPTV playlist through Studio (MPEG-TS)</label>
            <div class="field"><label>Tuner EPG for IPTV players</label>
              <select id="tn-epgsrc">
                <option value="0">Local Studio guide (/guide.xml)</option>
                <option value="1">my.epg.monster curated feed</option>
              </select></div>
            <p class="page-sub" id="tn-epghint"></p>
          </div>
          <button class="accent" id="tn-save" type="button">Save settings</button>
        </section>
        <section class="tile tn-lineup">
          <h2>Tuner channel list</h2>
          <p class="hint">Checked channels are published to every enabled tuner. Auto Populate numbers the checked rows 1, 2, 3… in playlist group order (or every row if none are checked) and saves. Manual checkbox or number edits need Save list.</p>
          <div class="tabs-row">
            <button id="tn-auto" type="button">Auto Populate</button>
            <button class="accent" id="tn-lsave" type="button">Save list</button>
          </div>
          <div class="tn-search-row">
            <label class="tn-search-label">Search</label>
            <span class="page-sub" id="tn-lineup-count"></span>
          </div>
          <input id="tn-lq" placeholder="name or group…" />
          <div id="tn-lpicks" class="editor-list tn-lpicks"></div>
        </section>
      </div>
    </div>
    <div class="dialog-backdrop" id="tn-dlg">
      <div class="dialog" style="width:640px;max-height:80vh;overflow:auto">
        <h2 id="tn-dlg-title"></h2>
        <pre id="tn-dlg-body" class="page-sub" style="white-space:pre-wrap;user-select:text"></pre>
        <div class="dialog-actions"><button id="tn-dlg-close">Close</button></div>
      </div>
    </div>
    <div class="dialog-backdrop" id="tn-log-dlg">
      <div class="dialog" style="width:720px;max-height:85vh;overflow:auto">
        <h2>TV Tuner log</h2>
        <p class="page-sub">Verbose requests, binds, and tunes for this Studio session. File copy is under Settings → Open logs folder.</p>
        <div class="editor-toolbar" style="margin-top:8px">
          <select id="tn-log-kind" style="width:180px">
            <option value="">All tuners</option>
            <option>Plex</option>
            <option>Jellyfin</option>
            <option>Emby</option>
            <option>Iptv</option>
          </select>
          <label class="check"><input type="checkbox" id="tn-log-pause" /> Pause</label>
          <button id="tn-log-copy">Copy</button>
          <button id="tn-log-clear">Clear</button>
          <span class="page-sub" id="tn-log-count"></span>
        </div>
        <pre id="tn-log-body" class="page-sub" style="white-space:pre;overflow:auto;max-height:420px;background:#12121a;padding:12px;border-radius:8px;user-select:text;font-family:Consolas,monospace"></pre>
        <div class="dialog-actions"><button id="tn-log-close">Close</button></div>
      </div>
    </div>
    <div class="dialog-backdrop" id="tn-graph-dlg">
      <div class="dialog" style="width:640px;max-height:85vh;overflow:auto">
        <h2>TV Tuner graphs</h2>
        <p class="page-sub" id="tn-graph-sum">Loading…</p>
        <div id="tn-graph-body"></div>
        <div class="dialog-actions"><button id="tn-graph-close">Close</button></div>
      </div>
    </div>
    <div class="dialog-backdrop" id="tn-links-dlg">
      <div class="dialog" style="width:560px;max-height:80vh;overflow:auto">
        <h2 id="tn-links-title"></h2>
        <p class="page-sub" id="tn-links-hint"></p>
        <div id="tn-links-rows"></div>
        <div class="dialog-actions">
          <button id="tn-links-close">Close</button>
          <button id="tn-links-copy">Copy all</button>
          <button class="accent" id="tn-links-open">Open tuner</button>
        </div>
      </div>
    </div>
    <div class="dialog-backdrop" id="tn-test-dlg">
      <div class="dialog" style="width:640px;max-height:85vh;overflow:auto">
        <h2>Tuner self-test</h2>
        <p class="page-sub">Mimics Plex / Jellyfin / Emby / TiviMate HTTP. Saved to tunertest.json for submission.</p>
        <p class="page-sub" id="tn-test-path" style="user-select:text"></p>
        <div id="tn-test-body"></div>
        <div class="dialog-actions">
          <button id="tn-test-close">Close</button>
          <button id="tn-test-copy">Copy</button>
        </div>
      </div>
    </div>
  `;
}

export async function mountTuner(page: HTMLElement, toast: (s: string) => void): Promise<() => void> {
  let rows: TunerStatus[] = [];
  let currentKind = "";
  let logPause = false;
  let logKind = "";
  let links: { label: string; url: string }[] = [];
  let linkRoot = "";
  let lastTestJson = "";
  let logTimer = 0;
  let graphTimer = 0;
  let picks: TunerPickRow[] = [];
  let settings: AppSettings | null = null;

  const landing = page.querySelector<HTMLElement>("#tn-view-landing")!;
  const detail = page.querySelector<HTMLElement>("#tn-view-detail")!;

  const epgOf = (s: TunerStatus) =>
    (s.advertisedEpg && s.advertisedEpg.trim()) || `${s.baseUrl.replace(/\/$/, "")}/guide.xml`;

  const profileOf = (st: AppSettings, kind: string): TunerProfile => {
    if (kind === "Jellyfin") return st.JellyfinTuner;
    if (kind === "Emby") return st.EmbyTuner;
    if (kind === "Iptv") return st.IptvTuner;
    return st.PlexTuner;
  };

  const paintLanding = () => {
    const media = page.querySelector("#tn-row-media")!;
    const iptvRow = page.querySelector("#tn-row-iptv")!;
    if (!media.querySelector(".tn-pick")) {
      for (const item of LANDING) {
        const btn = document.createElement("button");
        btn.type = "button";
        btn.className = item.kind === "Iptv" ? "tn-pick tn-pick-iptv" : "tn-pick";
        btn.dataset.kind = item.kind;
        btn.setAttribute("aria-label", item.name);
        btn.title = item.name;
        const label =
          item.kind === "Iptv" ? `<span class="tn-pick-name">${esc(item.name)}</span>` : "";
        btn.innerHTML = `<img class="tn-pick-icon" src="${item.icon}" alt="${esc(item.name)}" />${label}`;
        (item.kind === "Iptv" ? iptvRow : media).appendChild(btn);
      }
    }
    for (const item of LANDING) {
      const s = rows.find((r) => r.kind === item.kind);
      const btn = page.querySelector<HTMLButtonElement>(`button.tn-pick[data-kind="${item.kind}"]`);
      if (!btn) continue;
      btn.classList.toggle("is-running", !!s?.running);
      btn.classList.toggle("is-enabled", !!s?.enabled && !s?.running);
    }
    const enabled = rows.filter((r) => r.enabled);
    const running = rows.filter((r) => r.running);
    const active = rows.reduce((n, r) => n + r.activeConnections, 0);
    page.querySelector("#tn-summary")!.textContent =
      enabled.length === 0
        ? "None enabled — open a tuner to turn it on"
        : `${enabled.length} enabled · ${running.length} running · ${active} live connection(s)`;
    (page.querySelector("#tn-start-all") as HTMLButtonElement).disabled = !enabled.some((s) => !s.running);
    (page.querySelector("#tn-stop-all") as HTMLButtonElement).disabled = running.length === 0;
  };

  const paintDetailStatus = () => {
    const s = rows.find((r) => r.kind === currentKind);
    if (!s) return;
    const status = s.error
      ? `${s.statusLabel} · ${s.error}`
      : `${s.statusLabel} · port ${s.port} · ${s.deviceId}`;
    const conn = s.running
      ? `${s.activeConnections} of ${s.maxConnections} connections in use`
      : `0 of ${s.maxConnections} connections (stopped)`;
    const epg = epgOf(s);
    const detailLine =
      s.kind === "Iptv"
        ? `Playlist ${s.baseUrl}/playlist.m3u8   ·   EPG ${epg}`
        : `${s.baseUrl}   ·   EPG ${epg}`;
    page.querySelector("#tn-detail-title")!.textContent = s.friendlyName;
    page.querySelector("#tn-detail-status")!.textContent = `${status} · ${conn}`;
    page.querySelector("#tn-detail-urls")!.textContent = detailLine;
    (page.querySelector("#tn-start") as HTMLButtonElement).disabled = !(s.enabled && !s.running);
    (page.querySelector("#tn-stop") as HTMLButtonElement).disabled = !s.running;
    (page.querySelector("#tn-links") as HTMLButtonElement).disabled = !s.enabled;
  };

  const fillSettingsForm = (st: AppSettings, kind: string) => {
    const p = profileOf(st, kind);
    (page.querySelector("#tn-on") as HTMLInputElement).checked = p.Enabled;
    (page.querySelector("#tn-name") as HTMLInputElement).value = p.FriendlyName ?? "";
    (page.querySelector("#tn-port") as HTMLInputElement).value = String(p.Port);
    (page.querySelector("#tn-count") as HTMLInputElement).value = String(p.TunerCount);
    (page.querySelector("#tn-lan") as HTMLInputElement).checked = p.AllowLan;
    const jelly = page.querySelector<HTMLElement>("#tn-jelly-extra")!;
    const iptv = page.querySelector<HTMLElement>("#tn-iptv-extra")!;
    jelly.hidden = kind !== "Jellyfin";
    iptv.hidden = kind !== "Iptv";
    if (kind === "Jellyfin") {
      (page.querySelector("#tn-down") as HTMLInputElement).checked = !!p.DownspiralEnabled;
    }
    if (kind === "Iptv") {
      (page.querySelector("#tn-remux") as HTMLInputElement).checked = p.RemuxEnabled !== false;
      const hasFeed = !!(st.MemberFeedUrlGz || st.MemberFeedUrl);
      (page.querySelector("#tn-epgsrc") as HTMLSelectElement).value = st.TunerUseMemberEpg && hasFeed ? "1" : "0";
      page.querySelector("#tn-epghint")!.textContent = hasFeed
        ? "Curated feed: " + (st.MemberFeedUrlGz || st.MemberFeedUrl)
        : "Upload channels.json first to use the my.epg.monster feed as tuner EPG.";
    }
  };

  const paintPicks = () => {
    const q = (page.querySelector("#tn-lq") as HTMLInputElement).value.trim().toLowerCase();
    const el = page.querySelector("#tn-lpicks")!;
    el.innerHTML = "";
    let shown = 0;
    for (const p of picks) {
      if (q && !p.name.toLowerCase().includes(q) && !p.group.toLowerCase().includes(q)) continue;
      shown += 1;
      const row = document.createElement("div");
      row.className = "lineup-row";
      row.innerHTML = `<label class="check"><input type="checkbox" data-id="${esc(p.id)}" ${p.included ? "checked" : ""} />
        <span><span class="chan-name">${esc(p.name)}</span><span class="chan-sub">${esc(p.group)}</span></span></label>
        <input class="lineup-num" data-nid="${esc(p.id)}" placeholder="#" value="${p.number ?? ""}" />`;
      el.appendChild(row);
    }
    el.querySelectorAll<HTMLInputElement>("input[data-id]").forEach((box) => {
      box.addEventListener("change", () => {
        const p = picks.find((x) => x.id === box.dataset.id);
        if (p) p.included = box.checked;
      });
    });
    el.querySelectorAll<HTMLInputElement>("input[data-nid]").forEach((inp) => {
      inp.addEventListener("change", () => {
        const p = picks.find((x) => x.id === inp.dataset.nid);
        if (!p) return;
        const n = parseInt(inp.value.trim(), 10);
        p.number = Number.isFinite(n) && n > 0 ? n : null;
      });
    });
    const included = picks.filter((p) => p.included).length;
    page.querySelector("#tn-lineup-count")!.textContent = `${included} in lineup · ${shown} shown`;
  };

  const showLanding = () => {
    currentKind = "";
    landing.hidden = false;
    detail.hidden = true;
    paintLanding();
  };

  const showDetail = async (kind: string) => {
    currentKind = kind;
    landing.hidden = true;
    detail.hidden = false;
    settings = await invoke<AppSettings>("load_settings");
    if (!page.querySelector("#tn-view-detail")) return;
    fillSettingsForm(settings, kind);
    paintDetailStatus();
    picks = await invoke<TunerPickRow[]>("lineup_candidates");
    (page.querySelector("#tn-lq") as HTMLInputElement).value = "";
    paintPicks();
  };

  const reload = async () => {
    rows = await invoke<TunerStatus[]>("tuner_statuses");
    if (currentKind) paintDetailStatus();
    else paintLanding();
  };

  const infoDlg = (title: string, body: string) => {
    page.querySelector("#tn-dlg-title")!.textContent = title;
    page.querySelector("#tn-dlg-body")!.textContent = body;
    page.querySelector("#tn-dlg")!.classList.add("open");
  };

  const fmtLog = (l: LogLine) => {
    const t = l.at ? new Date(l.at) : null;
    const clock =
      t && !Number.isNaN(t.getTime())
        ? `${String(t.getHours()).padStart(2, "0")}:${String(t.getMinutes()).padStart(2, "0")}:${String(t.getSeconds()).padStart(2, "0")}.${String(t.getMilliseconds()).padStart(3, "0")}`
        : "--:--:--.---";
    const who = (l.kind || "ALL").toUpperCase();
    return `${clock}  [INFO ]  [${who}]  ${l.line}`;
  };

  const paintLog = async () => {
    if (logPause) return;
    const body = page.querySelector("#tn-log-body");
    const count = page.querySelector("#tn-log-count");
    if (!body || !count) return;
    const lines = await invoke<LogLine[]>("tuner_logs");
    if (!page.querySelector("#tn-log-body")) return;
    const shown = logKind ? lines.filter((l) => l.kind === logKind || !l.kind) : lines;
    body.textContent = shown.map(fmtLog).join("\n") || "(empty)";
    count.textContent = `${shown.length} line(s)`;
  };

  const openLog = (kind: string) => {
    logKind = kind;
    (page.querySelector("#tn-log-kind") as HTMLSelectElement).value = kind;
    page.querySelector("#tn-log-dlg")!.classList.add("open");
    void paintLog();
    if (logTimer) window.clearInterval(logTimer);
    logTimer = window.setInterval(() => void paintLog().catch(() => undefined), 1000);
  };

  const bar = (label: string, n: number, max: number) => {
    const pct = Math.round((n / Math.max(1, max)) * 100);
    return `<div class="chan-sub" style="margin:4px 0">${esc(label)} ${n}
      <div style="height:8px;background:#1e1e2a;border-radius:4px;overflow:hidden">
        <div style="width:${pct}%;height:8px;background:#6c5ce7"></div>
      </div></div>`;
  };

  const paintGraphs = async () => {
    if (!page.querySelector("#tn-graph-sum")) return;
    const g = await invoke<GraphRow[]>("tuner_graphs");
    if (!page.querySelector("#tn-graph-sum")) return;
    const live = g.reduce((n, r) => n + r.live, 0);
    const req = g.reduce((n, r) => n + r.discover + r.lineup + r.guide + r.m3u + r.stream, 0);
    page.querySelector("#tn-graph-sum")!.textContent =
      `${g.length} tuners  ·  ${g.filter((r) => r.live > 0 || rows.find((s) => s.kind === r.kind)?.running).length} running  ·  ${live} live connection(s)  ·  ${req} request(s) this session`;
    const maxReq = Math.max(1, ...g.map((r) => r.discover + r.lineup + r.guide + r.m3u + r.stream));
    page.querySelector("#tn-graph-body")!.innerHTML = g
      .map((r) => {
        const run = rows.find((s) => s.kind === r.kind)?.running ? "Running" : "Stopped";
        return `<section class="tile" style="margin-bottom:10px">
          <div class="chan-name">${esc(r.kind)} · ${run}</div>
          ${bar("Active connections", r.live, r.max || 1)}
          ${bar("Discover", r.discover, maxReq)}
          ${bar("Lineup", r.lineup, maxReq)}
          ${bar("Guide", r.guide, maxReq)}
          ${bar("Playlist", r.m3u, maxReq)}
          ${bar("Streams", r.stream, maxReq)}
          <div class="chan-sub">404 ${r.notFound} · ${r.bytes} bytes</div>
        </section>`;
      })
      .join("");
  };

  const openGraphs = () => {
    page.querySelector("#tn-graph-dlg")!.classList.add("open");
    void paintGraphs();
    if (graphTimer) window.clearInterval(graphTimer);
    graphTimer = window.setInterval(() => void paintGraphs().catch(() => undefined), 1000);
  };

  const openLinks = (s: TunerStatus) => {
    const root = s.baseUrl.replace(/\/$/, "");
    const epg = epgOf(s);
    if (s.kind === "Iptv") {
      links = [
        { label: "Playlist", url: `${root}/playlist.m3u8` },
        { label: "EPG", url: epg },
        { label: "M3U", url: `${root}/tuner.m3u` },
      ];
    } else {
      links = [
        { label: "Tuner", url: root },
        { label: "Discover", url: `${root}/discover.json` },
        { label: "Lineup", url: `${root}/lineup.json` },
        { label: "EPG", url: epg },
      ];
      if (s.kind === "Jellyfin") {
        links.push({ label: "Playlist", url: `${root}/playlist.m3u8` });
        links.push({ label: "M3U", url: `${root}/tuner.m3u` });
      }
    }
    linkRoot = root;
    page.querySelector("#tn-links-title")!.textContent = `${s.friendlyName} links`;
    page.querySelector("#tn-links-hint")!.textContent = s.running
      ? s.kind === "Iptv"
        ? "Paste Playlist + EPG into TiviMate or another IPTV player."
        : "Paste these into Plex, Jellyfin, or Emby Live TV setup."
      : "Start the tuner first so these URLs accept connections.";
    page.querySelector("#tn-links-rows")!.innerHTML = links
      .map(
        (l) =>
          `<div class="field"><label>${esc(l.label)}</label><input readonly value="${esc(l.url)}" /></div>`,
      )
      .join("");
    page.querySelector("#tn-links-dlg")!.classList.add("open");
  };

  page.querySelector("#tn-dlg-close")!.addEventListener("click", () => {
    page.querySelector("#tn-dlg")!.classList.remove("open");
  });
  page.querySelector("#tn-log-close")!.addEventListener("click", () => {
    page.querySelector("#tn-log-dlg")!.classList.remove("open");
    if (logTimer) window.clearInterval(logTimer);
    logTimer = 0;
  });
  page.querySelector("#tn-graph-close")!.addEventListener("click", () => {
    page.querySelector("#tn-graph-dlg")!.classList.remove("open");
    if (graphTimer) window.clearInterval(graphTimer);
    graphTimer = 0;
  });
  page.querySelector("#tn-links-close")!.addEventListener("click", () => {
    page.querySelector("#tn-links-dlg")!.classList.remove("open");
  });
  page.querySelector("#tn-test-close")!.addEventListener("click", () => {
    page.querySelector("#tn-test-dlg")!.classList.remove("open");
  });
  page.querySelector("#tn-log-kind")!.addEventListener("change", (ev) => {
    logKind = (ev.target as HTMLSelectElement).value;
    void paintLog();
  });
  page.querySelector("#tn-log-pause")!.addEventListener("change", (ev) => {
    logPause = (ev.target as HTMLInputElement).checked;
    if (!logPause) void paintLog();
  });
  page.querySelector("#tn-log-copy")!.addEventListener("click", async () => {
    const text = page.querySelector("#tn-log-body")!.textContent ?? "";
    try {
      await navigator.clipboard.writeText(text);
      toast("Tuner log copied");
    } catch {
      toast("Could not copy log.");
    }
  });
  page.querySelector("#tn-log-clear")!.addEventListener("click", async () => {
    await invoke("tuner_clear_logs");
    await paintLog();
  });
  page.querySelector("#tn-links-copy")!.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(links.map((l) => `${l.label}: ${l.url}`).join("\n"));
      toast("Tuner links copied");
    } catch {
      toast("Could not copy links.");
    }
  });
  page.querySelector("#tn-links-open")!.addEventListener("click", async () => {
    try {
      await openUrl(linkRoot);
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#tn-test-copy")!.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(lastTestJson);
      toast("Tuner test JSON copied");
    } catch {
      toast("Could not copy JSON.");
    }
  });

  page.querySelector("#tn-start-all")!.addEventListener("click", async () => {
    const errs = await invoke<string[]>("tuner_start_all");
    await reload();
    if (errs.length) toast(errs.join(" · "));
  });
  page.querySelector("#tn-stop-all")!.addEventListener("click", async () => {
    await invoke("tuner_stop_all");
    await reload();
  });
  page.querySelector("#tn-log")!.addEventListener("click", () => openLog(""));
  page.querySelector("#tn-graphs")!.addEventListener("click", () => openGraphs());
  page.querySelector("#tn-test")!.addEventListener("click", async () => {
    toast("Self-test running…");
    try {
      const dto = await invoke<{ json: string; path: string; reports: ProbeReport[] }>("tuner_self_test");
      lastTestJson = dto.json;
      page.querySelector("#tn-test-path")!.textContent = dto.path;
      page.querySelector("#tn-test-body")!.innerHTML = (dto.reports ?? [])
        .map((r) => {
          const raw = r as ProbeReport & { steps?: ProbeStep[]; kind?: string };
          const steps = raw.Steps ?? raw.steps ?? [];
          const kind = raw.Kind ?? raw.kind ?? "";
          const body = steps
            .map((s) => {
              const st = s as ProbeStep & { client?: string; name?: string; ok?: boolean; detail?: string };
              const ok = st.Ok ?? st.ok;
              return `<div class="chan-sub">${ok ? "OK" : "FAIL"}  ${esc(st.Client ?? st.client ?? "")} · ${esc(st.Name ?? st.name ?? "")} — ${esc(st.Detail ?? st.detail ?? "")}</div>`;
            })
            .join("");
          const fail = steps.filter((s) => !(s.Ok ?? (s as { ok?: boolean }).ok)).length;
          const sum = fail
            ? `${kind}: ${fail} failed / ${steps.length}`
            : `${kind}: ${steps.length} checks passed`;
          return `<section class="tile" style="margin:8px 0"><div class="chan-name">${esc(sum)}</div>${body}</section>`;
        })
        .join("");
      page.querySelector("#tn-test-dlg")!.classList.add("open");
    } catch (e) {
      toast(String(e));
    }
  });

  landing.addEventListener("click", (ev) => {
    const btn = (ev.target as HTMLElement).closest<HTMLButtonElement>("button.tn-pick");
    if (!btn?.dataset.kind) return;
    void showDetail(btn.dataset.kind).catch((e) => toast(String(e)));
  });

  page.querySelector("#tn-back")!.addEventListener("click", () => showLanding());

  page.querySelector("#tn-start")!.addEventListener("click", async () => {
    if (!currentKind) return;
    try {
      const msg = await invoke<string>("tuner_start", { kind: currentKind });
      toast(msg);
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#tn-stop")!.addEventListener("click", async () => {
    if (!currentKind) return;
    try {
      await invoke("tuner_stop", { kind: currentKind });
      await reload();
      toast(`${currentKind} tuner stopped`);
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#tn-log-one")!.addEventListener("click", () => openLog(currentKind));
  page.querySelector("#tn-graphs-one")!.addEventListener("click", () => openGraphs());
  page.querySelector("#tn-info")!.addEventListener("click", async () => {
    if (!currentKind) return;
    try {
      const s = rows.find((r) => r.kind === currentKind);
      const help = await invoke<string>("tuner_help", { kind: currentKind });
      infoDlg(`${s?.friendlyName ?? currentKind} — setup`, help);
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#tn-links")!.addEventListener("click", () => {
    const s = rows.find((r) => r.kind === currentKind);
    if (s) openLinks(s);
  });

  page.querySelector("#tn-save")!.addEventListener("click", async () => {
    if (!currentKind) return;
    try {
      const st = await invoke<AppSettings>("load_settings");
      const p = profileOf(st, currentKind);
      p.Kind = currentKind;
      p.Enabled = (page.querySelector("#tn-on") as HTMLInputElement).checked;
      p.Running = p.Enabled ? p.Running : false;
      p.FriendlyName = (page.querySelector("#tn-name") as HTMLInputElement).value.trim() || p.FriendlyName;
      p.Port = parseInt((page.querySelector("#tn-port") as HTMLInputElement).value, 10) || p.Port;
      p.TunerCount = parseInt((page.querySelector("#tn-count") as HTMLInputElement).value, 10) || p.TunerCount;
      p.AllowLan = (page.querySelector("#tn-lan") as HTMLInputElement).checked;
      if (currentKind === "Jellyfin") {
        p.DownspiralEnabled = (page.querySelector("#tn-down") as HTMLInputElement).checked;
      }
      if (currentKind === "Iptv") {
        p.RemuxEnabled = (page.querySelector("#tn-remux") as HTMLInputElement).checked;
        let useMember = (page.querySelector("#tn-epgsrc") as HTMLSelectElement).value === "1";
        if (useMember && !st.MemberFeedUrl && !st.MemberFeedUrlGz && !st.MemberAccessKey) useMember = false;
        st.TunerUseMemberEpg = useMember;
      }
      await invoke("save_settings", { settings: st });
      await invoke("tuner_set_max", { kind: currentKind, max: p.TunerCount });
      settings = st;
      fillSettingsForm(st, currentKind);
      toast("Tuner settings saved");
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });

  const saveLineup = async (note?: string) => {
    const msg = await invoke<string>("save_tuner_lineup", {
      picks: picks.map((p) => ({ id: p.id, included: p.included, number: p.number ?? null })),
    });
    toast(note ?? msg);
    picks = await invoke<TunerPickRow[]>("lineup_candidates");
    paintPicks();
  };

  page.querySelector("#tn-lq")!.addEventListener("input", paintPicks);
  page.querySelector("#tn-auto")!.addEventListener("click", () => {
    let targets = picks.filter((p) => p.included);
    if (targets.length === 0) targets = picks;
    let n = 1;
    for (const p of targets) {
      p.included = true;
      p.number = n++;
    }
    paintPicks();
    void saveLineup("Lineup auto-populated and saved").catch((e) => toast(String(e)));
  });
  page.querySelector("#tn-lsave")!.addEventListener("click", () => {
    void saveLineup().catch((e) => toast(String(e)));
  });

  page.querySelector("#tn-disco")!.addEventListener("change", async () => {
    try {
      const st = await invoke<AppSettings>("load_settings");
      st.DiscoveryEnabled = (page.querySelector("#tn-disco") as HTMLInputElement).checked;
      await invoke("save_settings", { settings: st });
      toast(st.DiscoveryEnabled ? "Tuner advertise on" : "Tuner advertise off");
    } catch (e) {
      toast(String(e));
    }
  });

  let poll = 0;
  try {
    settings = await invoke<AppSettings>("load_settings");
    (page.querySelector("#tn-disco") as HTMLInputElement).checked = settings.DiscoveryEnabled !== false;
    await reload();
    poll = window.setInterval(() => {
      if (!page.querySelector("#tn-view-landing")) {
        window.clearInterval(poll);
        return;
      }
      void reload().catch(() => undefined);
    }, 2000);
  } catch (e) {
    toast(String(e));
  }
  return () => {
    window.clearInterval(poll);
    if (logTimer) window.clearInterval(logTimer);
    if (graphTimer) window.clearInterval(graphTimer);
  };
}

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}
