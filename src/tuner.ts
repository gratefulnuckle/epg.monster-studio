import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

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

export function tunerHtml(): string {
  return `
    <div class="editor-toolbar">
      <span class="editor-title">TV Tuner</span>
      <button class="accent" id="tn-start-all">Start all enabled</button>
      <button id="tn-stop-all">Stop all</button>
      <button id="tn-log" title="Verbose tuner log">Log</button>
      <button id="tn-graphs" title="Live tuner stats">Graphs</button>
      <button id="tn-test" title="Mimic Plex / Jellyfin / Emby / TiviMate HTTP without those apps">Self-test</button>
      <span class="page-sub" id="tn-summary"></span>
    </div>
    <p class="page-sub">Start and stop the local tuner hosts. Enable a card in Settings, then Start here. Ports are 8080 Plex, 8081 Jellyfin, 8082 Emby, 8083 IPTV. Channel numbers stay in Managed Output → Tuner lineup.</p>
    <p class="page-sub" id="tn-empty" hidden>Enable a tuner in Settings (Plex, Jellyfin, Emby, or IPTV), Save, then press Start on that card.</p>
    <div id="tn-cards"></div>
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

export async function mountTuner(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  let rows: TunerStatus[] = [];
  let logPause = false;
  let logKind = "";
  let links: { label: string; url: string }[] = [];
  let linkRoot = "";
  let lastTestJson = "";
  let logTimer = 0;
  let graphTimer = 0;

  const epgOf = (s: TunerStatus) =>
    (s.advertisedEpg && s.advertisedEpg.trim()) || `${s.baseUrl.replace(/\/$/, "")}/guide.xml`;

  const paint = () => {
    const el = page.querySelector("#tn-cards")!;
    el.innerHTML = "";
    for (const s of rows) {
      const card = document.createElement("section");
      card.className = "tile tuner-card";
      const status = s.error
        ? `${s.statusLabel} · ${s.error}`
        : `${s.statusLabel} · port ${s.port} · ${s.deviceId}`;
      const conn = s.running
        ? `${s.activeConnections} of ${s.maxConnections} connections in use`
        : `0 of ${s.maxConnections} connections (stopped)`;
      const epg = epgOf(s);
      const detail =
        s.kind === "Iptv"
          ? `Playlist ${s.baseUrl}/playlist.m3u8   ·   EPG ${epg}`
          : `${s.baseUrl}   ·   EPG ${epg}`;
      card.innerHTML = `
        <div class="tuner-head">
          <div>
            <div class="chan-name" style="font-size:16px">${esc(s.friendlyName)}</div>
            <div class="chan-sub">${esc(status)}</div>
          </div>
          <div class="tuner-actions">
            <button data-start="${esc(s.kind)}" ${s.enabled && !s.running ? "" : "disabled"}>Start</button>
            <button data-stop="${esc(s.kind)}" ${s.running ? "" : "disabled"}>Stop</button>
            <button data-log="${esc(s.kind)}">Log</button>
            <button data-graphs="${esc(s.kind)}">Graphs</button>
            <button data-info="${esc(s.kind)}">Info</button>
          </div>
        </div>
        <div class="chan-name">${esc(conn)}</div>
        <div class="field" style="max-width:180px">
          <label>Allowed connections</label>
          <input type="number" min="1" max="16" value="${s.maxConnections}" data-max="${esc(s.kind)}" />
        </div>
        <button data-links="${esc(s.kind)}" ${s.enabled ? "" : "disabled"}>Open TV tuner links</button>
        <p class="page-sub" style="user-select:text">${esc(detail)}</p>`;
      el.appendChild(card);
    }
    const enabled = rows.filter((r) => r.enabled);
    const running = rows.filter((r) => r.running);
    const active = rows.reduce((n, r) => n + r.activeConnections, 0);
    page.querySelector("#tn-summary")!.textContent =
      enabled.length === 0
        ? "All four tuners listed · none enabled in Settings"
        : `${enabled.length} enabled · ${running.length} running · ${active} live connection(s)`;
    (page.querySelector("#tn-start-all") as HTMLButtonElement).disabled = !enabled.some((s) => !s.running);
    (page.querySelector("#tn-stop-all") as HTMLButtonElement).disabled = running.length === 0;
  };

  const reload = async () => {
    rows = await invoke<TunerStatus[]>("tuner_statuses");
    paint();
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
    const lines = await invoke<LogLine[]>("tuner_logs");
    const shown = logKind ? lines.filter((l) => l.kind === logKind || !l.kind) : lines;
    page.querySelector("#tn-log-body")!.textContent = shown.map(fmtLog).join("\n") || "(empty)";
    page.querySelector("#tn-log-count")!.textContent = `${shown.length} line(s)`;
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
    const g = await invoke<GraphRow[]>("tuner_graphs");
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

  page.addEventListener("click", async (ev) => {
    const t = ev.target as HTMLElement;
    const start = t.getAttribute("data-start");
    const stop = t.getAttribute("data-stop");
    const info = t.getAttribute("data-info");
    const log = t.getAttribute("data-log");
    const graphs = t.getAttribute("data-graphs");
    const linkKind = t.getAttribute("data-links");
    try {
      if (start) {
        const msg = await invoke<string>("tuner_start", { kind: start });
        toast(msg);
        await reload();
      } else if (stop) {
        await invoke("tuner_stop", { kind: stop });
        await reload();
        toast(`${stop} tuner stopped`);
      } else if (info) {
        const s = rows.find((r) => r.kind === info);
        const help = await invoke<string>("tuner_help", { kind: info });
        infoDlg(`${s?.friendlyName ?? info} — setup`, help);
      } else if (log) {
        openLog(log);
      } else if (graphs) {
        openGraphs();
      } else if (linkKind) {
        const s = rows.find((r) => r.kind === linkKind);
        if (s) openLinks(s);
      }
    } catch (e) {
      toast(String(e));
    }
  });
  page.addEventListener("change", async (ev) => {
    const t = ev.target as HTMLInputElement;
    const kind = t.getAttribute("data-max");
    if (!kind) return;
    const n = parseInt(t.value, 10);
    if (!Number.isFinite(n)) return;
    try {
      await invoke("tuner_set_max", { kind, max: n });
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });

  try {
    await reload();
    window.setInterval(() => void reload().catch(() => undefined), 2000);
  } catch (e) {
    toast(String(e));
  }
}

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}
