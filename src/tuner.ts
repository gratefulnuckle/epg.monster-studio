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
};

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
  `;
}

export async function mountTuner(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  let rows: TunerStatus[] = [];

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
      const epg = `${s.baseUrl.replace(/\/$/, "")}/guide.xml`;
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

  const dlg = (title: string, body: string) => {
    page.querySelector("#tn-dlg-title")!.textContent = title;
    page.querySelector("#tn-dlg-body")!.textContent = body;
    page.querySelector("#tn-dlg")!.classList.add("open");
  };

  page.querySelector("#tn-dlg-close")!.addEventListener("click", () => {
    page.querySelector("#tn-dlg")!.classList.remove("open");
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
  page.querySelector("#tn-log")!.addEventListener("click", async () => {
    const lines = await invoke<{ kind: string; line: string }[]>("tuner_logs");
    dlg("Tuner log", lines.map((l) => `[${l.kind}] ${l.line}`).join("\n") || "(empty)");
  });
  page.querySelector("#tn-graphs")!.addEventListener("click", async () => {
    const g = await invoke<string[]>("tuner_graphs");
    dlg("Tuner graphs", g.join("\n") || "Start a tuner to see stats.");
  });
  page.querySelector("#tn-test")!.addEventListener("click", async () => {
    toast("Self-test running…");
    const json = await invoke<string>("tuner_self_test");
    dlg("Self-test", json);
  });

  page.addEventListener("click", async (ev) => {
    const t = ev.target as HTMLElement;
    const start = t.getAttribute("data-start");
    const stop = t.getAttribute("data-stop");
    const info = t.getAttribute("data-info");
    const log = t.getAttribute("data-log");
    const graphs = t.getAttribute("data-graphs");
    const links = t.getAttribute("data-links");
    try {
      if (start) {
        const msg = await invoke<string>("tuner_start", { kind: start });
        toast(msg);
        await reload();
      } else if (stop) {
        await invoke("tuner_stop", { kind: stop });
        await reload();
      } else if (info) {
        dlg(`${info} — setup`, await invoke<string>("tuner_help", { kind: info }));
      } else if (log) {
        (page.querySelector("#tn-log") as HTMLButtonElement).click();
      } else if (graphs) {
        (page.querySelector("#tn-graphs") as HTMLButtonElement).click();
      } else if (links) {
        const s = rows.find((r) => r.kind === links);
        if (!s) return;
        const root = s.baseUrl.replace(/\/$/, "");
        await openUrl(s.kind === "Iptv" ? `${root}/playlist.m3u8` : `${root}/discover.json`);
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
