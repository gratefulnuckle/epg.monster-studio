import { invoke } from "@tauri-apps/api/core";

type SplashCheck = { label: string; ok: boolean; detail: string };
type SplashEpg = { catalog: number; programmes: number; cached: boolean };

const LOCAL_PROGRESS = [8, 16, 24, 32, 38, 44, 50];

export async function runSplash(app: HTMLElement): Promise<void> {
  app.innerHTML = `
    <div class="splash">
      <div class="splash-card">
        <img src="/logo.png" alt="epg.monster studio" />
        <div class="splash-ver" id="splash-ver">epg.monster studio  ·  v1.0-beta</div>
        <div class="splash-list" id="splash-list"></div>
        <div class="splash-foot">
          <div class="splash-bar"><span id="splash-bar"></span></div>
          <div class="splash-status" id="splash-status">Checking resources…</div>
        </div>
      </div>
    </div>
  `;

  const list = document.getElementById("splash-list")!;
  const bar = document.getElementById("splash-bar")!;
  const ver = document.getElementById("splash-ver")!;
  const status = document.getElementById("splash-status")!;

  try {
    const info = await invoke<{ version: string; displayName: string }>("get_studio_info");
    ver.textContent = `${info.displayName}  ·  ${info.version}`;
  } catch {
    /* splash still works if invoke is not ready */
  }

  const started = Date.now();
  setProgress(bar, 0);

  let checks: SplashCheck[] = [];
  try {
    checks = await invoke<SplashCheck[]>("splash_checks");
  } catch {
    checks = [
      { label: "Application data folder", ok: false, detail: "waiting…" },
      { label: "SQLite database", ok: false, detail: "waiting…" },
      { label: "mpv player", ok: false, detail: "waiting…" },
      { label: "ffmpeg (auto-audit)", ok: false, detail: "waiting…" },
      { label: "ffprobe", ok: false, detail: "waiting…" },
      { label: "VLC (optional)", ok: false, detail: "waiting…" },
      { label: "Playlist cache folder", ok: false, detail: "waiting…" },
    ];
  }

  const rows: HTMLElement[] = [];
  for (const c of checks) {
    rows.push(addWaiting(list, c.label));
  }
  const xmlRow = addWaiting(list, "XMLTV guide (epg.monster)");
  const nowRow = addWaiting(list, "Now playing index");

  for (let i = 0; i < checks.length; i++) {
    const c = checks[i];
    status.textContent = `Checking ${c.label}…`;
    setProgress(bar, LOCAL_PROGRESS[i] ?? 50);
    await delay(200);
    complete(rows[i], c.ok, c.detail);
    await delay(80);
  }

  setCheckText(xmlRow, "0%");
  setCheckText(nowRow, "0%");
  setProgress(bar, 55);

  try {
    const epg = await invoke<SplashEpg>("splash_epg_status");
    if (epg.cached) {
      status.textContent = "Loading cached epg.monster…";
      complete(xmlRow, true, `cached ${fmt(epg.catalog)} tvg-ids`);
      complete(
        nowRow,
        epg.programmes > 0,
        `cached ${fmt(epg.programmes)} programmes`,
      );
      setProgress(bar, 99);
    } else {
      status.textContent = "Downloading epg.monster…";
      setCheckText(xmlRow, "…");
      try {
        const msg = await invoke<string>("fetch_epg_catalog", { url: null });
        const after = await invoke<SplashEpg>("splash_epg_status");
        complete(xmlRow, true, msg || `${fmt(after.catalog)} tvg-ids`);
        complete(
          nowRow,
          after.programmes > 0,
          `${fmt(after.programmes)} programmes`,
        );
      } catch (e) {
        complete(xmlRow, false, shorten(String(e)));
        complete(nowRow, false, "skipped");
      }
    }
  } catch (e) {
    complete(xmlRow, false, shorten(String(e)));
    complete(nowRow, false, "skipped");
  }

  setProgress(bar, 100);
  status.textContent = "Ready — opening studio…";
  const wait = Math.max(0, 5000 - (Date.now() - started));
  await delay(wait);
}

function addWaiting(list: HTMLElement, label: string): HTMLElement {
  const row = document.createElement("div");
  row.className = "splash-row waiting";
  row.innerHTML = `<span class="splash-icon">○</span><span class="splash-label"></span><span class="splash-detail">waiting…</span>`;
  row.querySelector(".splash-label")!.textContent = label;
  list.appendChild(row);
  return row;
}

function complete(row: HTMLElement, ok: boolean, detail: string): void {
  row.classList.remove("waiting");
  row.classList.toggle("fail", !ok);
  row.querySelector(".splash-icon")!.textContent = ok ? "✓" : "✗";
  row.querySelector(".splash-detail")!.textContent = ok
    ? detail.trim() || "OK"
    : detail;
}

function setCheckText(row: HTMLElement, text: string): void {
  row.querySelector(".splash-detail")!.textContent = text;
}

function setProgress(bar: HTMLElement, percent: number): void {
  bar.style.width = `${Math.max(0, Math.min(100, percent))}%`;
}

function delay(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

function fmt(n: number): string {
  return n.toLocaleString("en-US");
}

function shorten(s: string): string {
  const t = s.replace(/^error:\s*/i, "");
  if (t.length <= 56) return t;
  return "…" + t.slice(-52);
}
