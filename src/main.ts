import { invoke } from "@tauri-apps/api/core";
import "./styles.css";
import { mountShell } from "./shell";
import { installCrashHooks, installHideOnMinimize, showPendingCrash, startHeartbeat } from "./crash";
import { runSplash } from "./splash";
import { startEpgSchedule } from "./epg-schedule";
import { loadStudioCaps } from "./capabilities";

installCrashHooks();
startHeartbeat();

const appEl = document.querySelector<HTMLDivElement>("#app");
if (!appEl) throw new Error("#app missing");
const app = appEl;
if (!(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__) {
  document.documentElement.classList.add("studio-web");
  const { gateWebLogin } = await import("./web/login");
  await gateWebLogin(app);
}

function isCatalogView(): boolean {
  try {
    if (new URLSearchParams(window.location.search).get("view") === "catalog") return true;
  } catch {
    /* ignore */
  }
  return (window as Window & { __STUDIO_VIEW?: string }).__STUDIO_VIEW === "catalog";
}

if (isCatalogView()) {
  const { mountCatalogWindow } = await import("./catalog-window");
  await mountCatalogWindow(app);
} else {
const liveReload = sessionStorage.getItem("studio-live") === "1";
if (!liveReload) {
  await runSplash(app);
  sessionStorage.setItem("studio-live", "1");
}
try {
  await invoke("promote_main_window");
} catch {
  try {
    await invoke("promote_main_window");
  } catch {
    /* stay on splash size if both attempts fail */
  }
}
installHideOnMinimize();
await loadStudioCaps();
const shell = mountShell(app);
startEpgSchedule(shell.toast);
await showPendingCrash();
}
