import { invoke } from "@tauri-apps/api/core";
import { mountGhoulUi } from "./ghoul-ui";
import { ENGINE_LABEL, type PrepareDto, type StatusDto } from "./ghoul-types";

export function ghoulHtml(): string {
  return `
    <div class="gh-page" id="gh-page">
      <h1 class="page-title">IPTV Player</h1>
      <p class="page-sub">Live TV for the curated Playlist Editor lineup. G-houl embeds in this page.</p>
      <div class="gh-status" id="gh-status">Loading…</div>
      <button type="button" class="accent gh-launch" id="gh-launch" disabled>
        <img src="/ghoul.png" alt="" />
        <span>G-houl Player</span>
      </button>
      <p class="page-sub" id="gh-gate"></p>
    </div>
  `;
}

export async function mountGhoul(page: HTMLElement, toast: (s: string) => void): Promise<() => void> {
  let stopUi: (() => void) | undefined;
  const statusEl = page.querySelector("#gh-status")!;
  const gateEl = page.querySelector("#gh-gate")!;
  const btn = page.querySelector<HTMLButtonElement>("#gh-launch")!;

  const paint = (s: StatusDto) => {
    const engines = s.engines
      .map((e) =>
        e.available ? `${ENGINE_LABEL[e.id]} ready` : `${ENGINE_LABEL[e.id]}: ${e.reason || "missing"}`,
      )
      .join(" · ");
    const key = s.hasKey ? "access key set" : "no access key";
    const feed = s.feedUrl ? "member feed on file" : "no member feed";
    statusEl.innerHTML = `
      <p>${s.curatedCount} curated channel(s), ${s.playableCount} with a playable URL.</p>
      <p>${key} · ${feed}</p>
      <p>${engines || "no engines found"}</p>
    `;
    btn.disabled = !s.canMount;
    gateEl.textContent = s.gate || "";
  };

  try {
    paint(await invoke<StatusDto>("ghoul_status"));
    if (sessionStorage.getItem("studio-ghoul-open") === "1" && !btn.disabled) {
      btn.click();
    }
  } catch (e) {
    gateEl.textContent = String(e);
    btn.disabled = true;
  }

  btn.addEventListener("click", async () => {
    if (btn.disabled) return;
    try {
      const prep = await invoke<PrepareDto>("ghoul_prepare");
      sessionStorage.setItem("studio-ghoul-open", "1");
      const wrap = document.createElement("div");
      wrap.className = "gh-host";
      page.innerHTML = "";
      page.appendChild(wrap);
      stopUi = mountGhoulUi(wrap, prep, toast);
    } catch (e) {
      toast(String(e));
      gateEl.textContent = String(e);
    }
  });

  return () => {
    stopUi?.();
    void invoke("ghoul_unmount");
  };
}
