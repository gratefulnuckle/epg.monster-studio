import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

export type CrashReport = {
  title: string;
  summary: string;
  details: string;
  reportPath: string;
  logPath: string;
  when: string;
  kind: string;
};

let sendingIssue = false;

export function installCrashHooks(): void {
  window.addEventListener("error", (ev) => {
    if (sendingIssue) return;
    void invoke("write_crash_report", {
      kind: "managed",
      title: "Unhandled UI exception",
      summary: ev.message || "error",
      details: `${ev.message}\n${ev.filename}:${ev.lineno}:${ev.colno}\n${ev.error?.stack ?? ""}`,
    }).catch(() => undefined);
  });
  window.addEventListener("unhandledrejection", (ev) => {
    if (sendingIssue) return;
    void invoke("write_crash_report", {
      kind: "managed",
      title: "Unhandled promise rejection",
      summary: String(ev.reason ?? "rejection"),
      details: String(ev.reason?.stack ?? ev.reason ?? ""),
    }).catch(() => undefined);
  });
}

export function startHeartbeat(): void {
  const tick = async () => {
    try {
      const vis = await getCurrentWindow().isVisible();
      await invoke("log_heartbeat", { visible: vis, tray: !vis });
    } catch {
      /* ignore */
    }
  };
  void tick();
  window.setInterval(() => void tick(), 5000);
}

export function installHideOnMinimize(): void {
  const win = getCurrentWindow();
  void win.onResized(async () => {
    try {
      if (await win.isMinimized()) {
        await win.unminimize();
        await win.hide();
        await invoke("mark_tray_state");
        await toastIfAuditRunning();
      }
    } catch {
      /* ignore */
    }
  });
  void listen("studio-hidden-to-tray", () => {
    void toastIfAuditRunning();
  });
}

async function toastIfAuditRunning(): Promise<void> {
  try {
    const snap = await invoke<{ job?: { state?: string } | null }>("audit_snapshot");
    if (snap.job?.state !== "running") return;
    const toast = document.getElementById("toast");
    if (!toast) return;
    toast.textContent = "Still running in the tray (auto-audit continues). Use tray icon → Show / Close app.";
    toast.classList.add("open");
    window.setTimeout(() => toast.classList.remove("open"), 3000);
  } catch {
    /* ignore */
  }
}

export async function showPendingCrash(root: HTMLElement): Promise<void> {
  let report: CrashReport | null = null;
  try {
    report = await invoke<CrashReport | null>("consume_pending_crash");
  } catch {
    return;
  }
  if (!report) return;
  presentCrash(root, report);
}

export function presentCrash(root: HTMLElement, report: CrashReport): void {
  const kind = (report.kind || "unknown").toUpperCase();
  const host = document.createElement("div");
  host.className = "dialog-backdrop open";
  host.innerHTML = `
    <div class="dialog" style="width:720px;max-height:85vh;overflow:auto">
      <h2>Something went wrong</h2>
      <div class="chan-name">${esc(report.title)}</div>
      <p class="page-sub">${esc(kind)} · ${esc(report.when)}</p>
      <p>${esc(report.summary)}</p>
      <pre class="page-sub" style="white-space:pre-wrap;user-select:text;max-height:240px;overflow:auto;background:#0e0e14;padding:10px;border-radius:8px">${esc(report.details)}</pre>
      <div class="field"><label>Optional notes for epg.monster</label>
        <textarea id="crash-notes" placeholder="What were you doing when this happened?" rows="3"></textarea></div>
      <p class="page-sub">Crash report: ${esc(report.reportPath)}
Log file: ${esc(report.logPath)}</p>
      <p class="page-sub" id="crash-status"></p>
      <div class="dialog-actions">
        <button id="crash-send">Send report to epg.monster</button>
        <button id="crash-logs">Open logs folder</button>
        <button id="crash-file">Open crash report</button>
        <button id="crash-copy">Copy details</button>
        <button class="accent" id="crash-close">Close</button>
      </div>
    </div>`;
  root.appendChild(host);
  const status = host.querySelector<HTMLElement>("#crash-status")!;
  const sendBtn = host.querySelector<HTMLButtonElement>("#crash-send")!;
  host.querySelector("#crash-close")!.addEventListener("click", () => host.remove());
  host.querySelector("#crash-logs")!.addEventListener("click", () => {
    void invoke("open_folder", { path: report.logPath ?? "" });
  });
  host.querySelector("#crash-file")!.addEventListener("click", () => {
    void invoke("open_folder", { path: report.reportPath ?? "" });
  });
  host.querySelector("#crash-copy")!.addEventListener("click", () => {
    void navigator.clipboard.writeText(report.details ?? "");
  });
  sendBtn.addEventListener("click", async () => {
    const notes = (host.querySelector("#crash-notes") as HTMLTextAreaElement).value;
    sendBtn.disabled = true;
    sendingIssue = true;
    status.textContent = "Sending…";
    try {
      const r = await invoke<{ ok: boolean; message: string; githubUrl?: string | null }>("post_issue", {
        kind: "crash",
        title: report.title ?? "Crash",
        summary: report.summary ?? "",
        details: report.details ?? "",
        notes,
      });
      status.textContent = (r.ok ? "Report sent to epg.monster\n" : "Could not send report\n") + (r.message ?? "");
      if (r.ok && r.githubUrl) {
        const a = document.createElement("a");
        a.href = r.githubUrl;
        a.target = "_blank";
        a.rel = "noreferrer";
        a.textContent = "Open tracker";
        status.appendChild(document.createElement("br"));
        status.appendChild(a);
      }
    } catch (e) {
      status.textContent = "Could not send report\n" + String(e);
    } finally {
      sendingIssue = false;
      sendBtn.disabled = false;
    }
  });
}

function esc(s: string | null | undefined): string {
  return String(s ?? "").replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c] ?? c,
  );
}
