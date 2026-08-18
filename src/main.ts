import { invoke } from "@tauri-apps/api/core";
import "./styles.css";
import { mountShell } from "./shell";

const appEl = document.querySelector<HTMLDivElement>("#app");
if (!appEl) throw new Error("#app missing");
const app = appEl;

type SplashCheck = { label: string; ok: boolean; detail: string };

async function runSplash(): Promise<void> {
  app.innerHTML = `
    <div class="splash">
      <div class="splash-card">
        <img src="/logo.png" alt="epg.monster studio" />
        <div class="splash-ver" id="splash-ver">epg.monster studio  ·  v1.0-beta</div>
        <div class="splash-list" id="splash-list"></div>
        <div class="splash-bar"><span id="splash-bar"></span></div>
      </div>
    </div>
  `;

  const list = document.getElementById("splash-list")!;
  const bar = document.getElementById("splash-bar")!;
  const ver = document.getElementById("splash-ver")!;

  try {
    const info = await invoke<{ version: string; displayName: string }>("get_studio_info");
    ver.textContent = `${info.displayName}  ·  ${info.version}`;
  } catch {
    /* splash still works offline from UI */
  }

  const started = Date.now();
  let checks: SplashCheck[] = [];
  try {
    checks = await invoke<SplashCheck[]>("splash_checks");
  } catch {
    checks = [
      { label: "ffmpeg", ok: false, detail: "not detected yet" },
      { label: "ffprobe", ok: false, detail: "not detected yet" },
      { label: "mpv", ok: false, detail: "not detected yet" },
      { label: "XMLTV catalog", ok: true, detail: "opens after splash" },
    ];
  }

  checks.forEach((c, i) => {
    const row = document.createElement("div");
    row.className = "splash-row";
    row.innerHTML = `<span>${c.ok ? "✓" : "✗"}</span><span>${c.label}</span><span>${c.detail}</span>`;
    list.appendChild(row);
    bar.style.width = `${((i + 1) / checks.length) * 100}%`;
  });

  const wait = Math.max(0, 5000 - (Date.now() - started));
  await new Promise((r) => setTimeout(r, wait));
}

await runSplash();
mountShell(app);
