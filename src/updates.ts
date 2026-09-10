import { invoke } from "@tauri-apps/api/core";
import { isWebHost } from "./bridge/client";

type StudioUpdate = {
  current: string;
  displayVersion: string;
  edition?: string;
  latest?: string | null;
  updateAvailable: boolean;
  releaseUrl: string;
  notes?: string | null;
  error?: string | null;
  canApply?: boolean;
  assetName?: string | null;
};

export function updatesHtml(): string {
  return `
    <h1 class="page-title">Check For Updates</h1>
    <p class="page-sub">Compares this build with the latest GitHub Release. If a portable build for this OS is attached, Install and relaunch replaces this binary and starts it again. ./data is not touched.</p>
    <section class="tile" style="max-width:720px">
      <h2>GitHub release</h2>
      <p class="hint" id="upd-current">This build: …</p>
      <p class="page-sub" id="upd-status">Checking GitHub…</p>
      <pre class="upd-notes" id="upd-notes" hidden></pre>
      <div style="display:flex;gap:8px;flex-wrap:wrap;margin-top:12px">
        <button class="accent" id="upd-check">Check again</button>
        <button class="accent" id="upd-apply" hidden>Install and relaunch</button>
        <button id="upd-open">Open GitHub release</button>
      </div>
    </section>
  `;
}

export async function mountUpdates(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  const status = page.querySelector("#upd-status")!;
  const current = page.querySelector("#upd-current")!;
  const notes = page.querySelector<HTMLElement>("#upd-notes")!;
  const openBtn = page.querySelector<HTMLButtonElement>("#upd-open")!;
  const applyBtn = page.querySelector<HTMLButtonElement>("#upd-apply")!;

  const paint = async () => {
    status.textContent = "Checking GitHub…";
    notes.hidden = true;
    notes.textContent = "";
    applyBtn.hidden = true;
    try {
      const r = await invoke<StudioUpdate>("check_studio_update");
      current.textContent = `This build: ${r.displayVersion || r.current}`;
      if (r.error) {
        status.textContent = r.error;
        toast(r.error);
        return;
      }
      if (!r.updateAvailable) {
        const latest = r.latest || r.current;
        status.textContent = `Already current. Latest on GitHub: ${latest}.`;
        toast("Already current");
      } else {
        status.textContent = r.canApply
          ? `Update ${r.latest} is available (${r.assetName || "portable build"}).`
          : `Update ${r.latest} is on GitHub, but this release has no portable build for this OS. Open the release page.`;
        toast(`Update ${r.latest} available`);
        applyBtn.hidden = !r.canApply;
      }
      if (r.notes) {
        notes.hidden = false;
        notes.textContent = r.notes;
      }
    } catch (e) {
      const msg = String(e);
      status.textContent = msg;
      toast(msg);
    }
  };

  page.querySelector("#upd-check")!.addEventListener("click", () => void paint());
  applyBtn.addEventListener("click", () => {
    void (async () => {
      applyBtn.disabled = true;
      status.textContent = "Downloading update… this process will exit and come back.";
      try {
        await invoke("apply_studio_update");
      } catch (e) {
        if (!isWebHost()) {
          applyBtn.disabled = false;
          status.textContent = String(e);
          toast(String(e));
          return;
        }
      }
      if (isWebHost()) {
        status.textContent = "Relaunching the server…";
        for (let i = 0; i < 40; i++) {
          await new Promise((r) => setTimeout(r, 1500));
          try {
            const h = await fetch("/api/health", { cache: "no-store" });
            if (h.ok) {
              window.location.reload();
              return;
            }
          } catch {
            /* still down */
          }
        }
        status.textContent = "The server did not come back. Start it with studio.ps1 / studio.sh --start.";
        applyBtn.disabled = false;
      }
    })();
  });
  openBtn.addEventListener("click", async () => {
    try {
      await invoke("open_latest_release");
      toast("GitHub release opened");
    } catch (e) {
      toast(String(e));
    }
  });

  await paint();
}
