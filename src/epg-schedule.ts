import { invoke } from "@tauri-apps/api/core";

const INTERVAL_MS = 30 * 60 * 1000;

export function startEpgSchedule(toast: (s: string) => void): void {
  const tick = async (onlyIfStale: boolean) => {
    try {
      const kind = await invoke<string>("epg_refresh_schedule", { onlyIfStale });
      if (kind === "downloaded") toast("EPG guide updated from epg.monster");
    } catch {
      /* keep studio usable if the guide is unreachable */
    }
  };
  window.setTimeout(() => void tick(true), 8000);
  window.setInterval(() => void tick(false), INTERVAL_MS);
}