import { invoke } from "@tauri-apps/api/core";
import "./styles.css";
import { mountShell } from "./shell";
import { installCrashHooks, installHideOnMinimize, showPendingCrash, startHeartbeat } from "./crash";
import { runSplash } from "./splash";

installCrashHooks();

const appEl = document.querySelector<HTMLDivElement>("#app");
if (!appEl) throw new Error("#app missing");
const app = appEl;

await runSplash(app);
try {
  await invoke("promote_main_window");
} catch {
  /* stay on splash size if promote fails */
}
installHideOnMinimize();
mountShell(app);
startHeartbeat();
void showPendingCrash(app);
