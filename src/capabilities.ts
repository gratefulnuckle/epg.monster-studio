import { invoke } from "@tauri-apps/api/core";

export type StudioCaps = {
  ffmpeg: boolean;
  ffprobe: boolean;
  mpv: boolean;
  vlc: boolean;
};

let caps: StudioCaps = { ffmpeg: false, ffprobe: false, mpv: false, vlc: false };

export function studioCaps(): StudioCaps {
  return caps;
}

export function canStreamAudit(): boolean {
  return caps.ffmpeg && caps.ffprobe;
}

export function canPlay(): boolean {
  return caps.mpv || caps.vlc;
}

export async function loadStudioCaps(): Promise<StudioCaps> {
  try {
    caps = await invoke<StudioCaps>("studio_tools_status");
  } catch {
    caps = { ffmpeg: false, ffprobe: false, mpv: false, vlc: false };
  }
  window.dispatchEvent(new Event("studio-tools-changed"));
  return caps;
}

export function playerEngineOptionsHtml(): string {
  return `<option value="0">mpv</option><option value="1">VLC</option>`;
}

export function applyPlayerEngineValue(sel: HTMLSelectElement | null, stored: unknown): void {
  if (!sel) return;
  const n = Number(stored);
  const want = n === 1 ? "1" : "0";
  if ([...sel.options].some((o) => o.value === want)) {
    sel.value = want;
    return;
  }
  sel.value = sel.options[0]?.value ?? "0";
}

export function applyPlayGate(root: ParentNode): void {
  const on = canPlay();
  root.querySelectorAll<HTMLElement>(".player-field").forEach((el) => {
    el.classList.toggle("is-disabled", !on);
    el.querySelectorAll<HTMLSelectElement>("select").forEach((s) => {
      s.disabled = !on;
    });
    el.title = on ? "" : "Install mpv or VLC, or set a player path in Settings";
  });
  root.querySelectorAll<HTMLButtonElement>("button.play, button[data-act='play']").forEach((b) => {
    b.disabled = !on;
    if (!on) b.title = "Install mpv or VLC, or set a player path in Settings";
  });
  root.querySelectorAll(".chan-head .col-icon").forEach((el) => {
    if (el.textContent?.trim() === "Play") el.classList.toggle("is-disabled", !on);
  });
}
