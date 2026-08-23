import { invoke } from "@tauri-apps/api/core";

type StudioUpdate = {
  current: string;
  displayVersion: string;
  edition?: string;
  latest?: string | null;
  updateAvailable: boolean;
  releaseUrl: string;
  notes?: string | null;
  error?: string | null;
};

export function updatesHtml(): string {
  return `
    <h1 class="page-title">Check For Updates</h1>
    <p class="page-sub">Compares this build with the latest GitHub Release for gratefulnuckle/epg.monster-studio. Silent install + relaunch is v3; this page opens the release so you can install it.</p>
    <section class="tile" style="max-width:720px">
      <h2>GitHub release</h2>
      <p class="hint" id="upd-current">This build: …</p>
      <p class="page-sub" id="upd-status">Checking GitHub…</p>
      <pre class="upd-notes" id="upd-notes" hidden></pre>
      <div style="display:flex;gap:8px;flex-wrap:wrap;margin-top:12px">
        <button class="accent" id="upd-check">Check again</button>
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

  const paint = async () => {
    status.textContent = "Checking GitHub…";
    notes.hidden = true;
    notes.textContent = "";
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
        status.textContent = `Update ${r.latest} is available.`;
        toast(`Update ${r.latest} available`);
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
