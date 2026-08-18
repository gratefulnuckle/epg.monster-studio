import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type SplashCheck = { label: string; ok: boolean; detail: string };
type SplashEpg = { catalog: number; programmes: number; cached: boolean };

const LOCAL_PROGRESS = [8, 16, 24, 32, 38, 44, 50];

export async function runSplash(app: HTMLElement): Promise<void> {
  document.documentElement.classList.add("splash-open");
  app.innerHTML = `
    <div class="splash" data-tauri-drag-region>
      <div class="splash-card" data-tauri-drag-region>
        <div class="splash-logo-wrap">
          <img class="splash-logo" src="/logo.png" alt="epg.monster studio" />
        </div>
        <div class="splash-list" id="splash-list"></div>
        <div class="splash-mid">
          <div class="splash-ver-line">
            <span class="splash-ver" id="splash-ver">epg.monster studio  ·  v2.0.0 (dev)</span>
            <span class="splash-ver-sep"> / </span>
            <span class="splash-issues-line" id="splash-issues">0 open issues</span>
          </div>
          <div class="splash-status">
            <span id="splash-pct">0%</span>
            <span id="splash-status">Checking resources…</span>
          </div>
        </div>
      </div>
      <div class="splash-meter" id="splash-meter">
        <div class="splash-meter-fill" id="splash-lock"></div>
      </div>
    </div>
  `;

  const list = document.getElementById("splash-list")!;
  const lock = document.getElementById("splash-lock")!;
  const ver = document.getElementById("splash-ver")!;
  const status = document.getElementById("splash-status")!;

  try {
    const info = await invoke<{
      version: string;
      displayVersion?: string;
      displayName: string;
    }>("get_studio_info");
    ver.textContent = `${info.displayName}  ·  ${info.displayVersion || info.version}`;
  } catch {
    /* splash still works if invoke is not ready */
  }

  const pct = document.getElementById("splash-pct")!;
  const started = Date.now();
  setProgress(lock, pct, 0);

  try {
    const missing = await invoke<{ id: string; label: string }[]>("tools_missing");
    if (missing.length > 0) {
      status.textContent = "Downloading portable ffmpeg / mpv…";
      const unlisten = await listen<{ message: string; percent: number }>("tools-progress", (ev) => {
        status.textContent = ev.payload.message;
        setProgress(lock, pct, Math.max(1, Math.min(44, ev.payload.percent * 0.45)));
      });
      try {
        await invoke("tools_ensure");
      } catch {
        status.textContent = "Tool download failed — you can set paths in Settings.";
      } finally {
        unlisten();
      }
    }
  } catch {
    /* keep going — tools can be set in Settings */
  }

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

  let updatesOn = false;
  try {
    const st = await invoke<{ CheckForAppUpdates?: boolean }>("load_settings");
    updatesOn = !!st.CheckForAppUpdates;
  } catch {
    updatesOn = false;
  }
  if (updatesOn) {
    const updateRow = addWaiting(list, "Checking github for updates");
    void invoke<SplashCheck>("check_app_update")
      .then((r) => complete(updateRow, r.ok, r.detail))
      .catch((e) => complete(updateRow, false, shorten(String(e))));
  }

  const issuesLine = document.getElementById("splash-issues")!;
  void invoke<SplashCheck>("check_github_issues")
    .then((r) => {
      const n = (r.detail || "").match(/^(\d+)\s+open/)?.[1];
      issuesLine.textContent = n != null ? `${n} open issues` : r.detail || "0 open issues";
    })
    .catch(() => {
      issuesLine.textContent = "0 open issues";
    });

  const rows: HTMLElement[] = [];
  for (const c of checks) {
    rows.push(addWaiting(list, c.label));
  }
  const xmlRow = addWaiting(list, "XMLTV guide (epg.monster)");
  const nowRow = addWaiting(list, "Now playing index");

  for (let i = 0; i < checks.length; i++) {
    const c = checks[i];
    status.textContent = `Checking ${c.label}…`;
    setProgress(lock, pct, LOCAL_PROGRESS[i] ?? 50);
    await delay(200);
    complete(rows[i], c.ok, c.detail);
    await delay(80);
  }

  setCheckText(xmlRow, "0%");
  setCheckText(nowRow, "0%");
  setProgress(lock, pct, 55);

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
      setProgress(lock, pct, 99);
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

  status.textContent = "Reticulating splines......";
  setProgress(lock, pct, 99);
  await delay(2000);
  status.textContent = "Launching...";
  setProgress(lock, pct, 100);
  const wait = Math.max(0, 5000 - (Date.now() - started));
  await delay(wait);
  document.documentElement.classList.remove("splash-open");
}

function addWaiting(list: HTMLElement, label: string): HTMLElement {
  const row = document.createElement("div");
  row.className = "splash-row waiting";
  row.innerHTML = `<span class="splash-icon"></span><span class="splash-label"></span><span class="splash-detail"></span>`;
  row.querySelector(".splash-label")!.textContent = label;
  list.appendChild(row);
  return row;
}

function complete(row: HTMLElement, ok: boolean, detail: string): void {
  row.classList.remove("waiting");
  row.classList.add("done");
  row.classList.toggle("fail", !ok);
  row.querySelector(".splash-icon")!.textContent = ok ? "✓" : "✗";
  row.querySelector(".splash-detail")!.textContent = ok
    ? detail.trim() || "OK"
    : detail;
}

function setCheckText(row: HTMLElement, text: string): void {
  row.querySelector(".splash-detail")!.textContent = text;
}

function setProgress(lock: HTMLElement, pct: HTMLElement, percent: number): void {
  const n = Math.max(0, Math.min(100, Math.round(percent)));
  lock.style.width = `${n}%`;
  pct.textContent = `${n}%`;
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
