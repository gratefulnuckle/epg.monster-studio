import { invoke } from "@tauri-apps/api/core";

export type OutputRow = {
  id: string;
  name: string;
  group: string;
  tvgId: string;
  visibleUrl: string;
  variantsSummary: string;
  auditStatus: string;
  inTuner: boolean;
  tunerNumber?: number | null;
};

export type OutputSummary = {
  rows: OutputRow[];
  recentSwaps: number;
  tunerCount: number;
  enabledTuners: number;
  hasKey: boolean;
};

export type TunerPickRow = {
  id: string;
  name: string;
  group: string;
  included: boolean;
  number?: number | null;
};

export function outputHtml(): string {
  return `
    <div class="editor-toolbar">
      <span class="editor-title">Managed Output</span>
      <button class="accent" id="out-vis">Export visible m3u8…</button>
      <button id="out-all">Export all…</button>
      <button id="out-json">Export channels.json…</button>
      <button class="accent" id="out-upload">Upload channels.json</button>
      <button id="out-undo">Undo last swap</button>
      <button id="out-lineup">Tuner lineup…</button>
      <button id="out-refresh">Refresh</button>
      <span class="page-sub" id="out-summary"></span>
    </div>
    <div class="output-table">
      <div class="output-head">
        <span>Name</span><span>Group</span><span>tvg-id</span>
        <span>Visible URL</span><span>Variants</span><span>Audit</span>
      </div>
      <div id="out-rows" class="editor-list"></div>
    </div>
    <p class="page-sub" id="out-footer">Only the visible stream of each channel is exported. Hidden backups stay in the database for manual/auto swap during audit.</p>
    <div class="dialog-backdrop" id="out-lineup-dlg">
      <div class="dialog" style="width:560px;max-height:80vh;overflow:auto">
        <h2>Tuner lineup</h2>
        <p class="page-sub">Checked channels are published to every enabled tuner (Plex / Jellyfin / Emby). Auto Populate numbers the checked rows 1, 2, 3… in playlist group order (or every row if none are checked). Typing a used number swaps the two channels.</p>
        <div class="dialog-actions" style="justify-content:flex-start">
          <button id="out-auto">Auto Populate</button>
        </div>
        <div class="field"><label>Search</label><input id="out-lq" placeholder="name or group…" /></div>
        <div id="out-lpicks" class="editor-list" style="max-height:340px"></div>
        <div class="dialog-actions">
          <button id="out-lcancel">Cancel</button>
          <button class="accent" id="out-lsave">Save</button>
        </div>
      </div>
    </div>
    <div class="dialog-backdrop" id="out-up-dlg">
      <div class="dialog">
        <h2 id="out-up-title">channels.json uploaded</h2>
        <pre id="out-up-text" class="page-sub" style="white-space:pre-wrap;user-select:text"></pre>
        <div class="dialog-actions"><button id="out-up-close">Close</button></div>
      </div>
    </div>
  `;
}

export async function mountOutput(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  let filter = "";
  let picks: TunerPickRow[] = [];

  const reload = async () => {
    const sum = await invoke<OutputSummary>("output_summary", { filter: filter || null });
    const el = page.querySelector("#out-rows")!;
    el.innerHTML = "";
    for (const r of sum.rows) {
      const row = document.createElement("div");
      row.className = "output-row";
      row.innerHTML = `<span class="chan-name">${esc(r.name)}</span>
        <span>${esc(r.group)}</span>
        <span>${esc(r.tvgId)}</span>
        <span class="chan-sub" title="${esc(r.visibleUrl)}">${esc(r.visibleUrl)}</span>
        <span>${esc(r.variantsSummary)}</span>
        <span>${esc(r.auditStatus)}</span>`;
      el.appendChild(row);
    }
    page.querySelector("#out-summary")!.textContent =
      `${sum.rows.length} channels · ${sum.recentSwaps} recent swaps`;
    (page.querySelector("#out-upload") as HTMLButtonElement).disabled = !sum.hasKey;
    page.querySelector("#out-footer")!.textContent =
      `Only the visible stream of each channel is exported. Hidden backups stay in the database. ` +
      `Tuner lineup: ${sum.tunerCount} channel(s). Media servers enabled: ${sum.enabledTuners}.` +
      (sum.hasKey ? "" : " Add a my.epg.monster key in Settings to upload channels.json.");
  };

  page.querySelector("#out-refresh")!.addEventListener("click", () => void reload().catch((e) => toast(String(e))));
  page.querySelector("#out-vis")!.addEventListener("click", async () => {
    try {
      const msg = await invoke<string>("export_managed", { includeBackups: false });
      if (msg !== "cancelled") toast(msg);
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#out-all")!.addEventListener("click", async () => {
    try {
      const msg = await invoke<string>("export_managed", { includeBackups: true });
      if (msg !== "cancelled") toast(msg);
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#out-json")!.addEventListener("click", async () => {
    try {
      const msg = await invoke<string>("export_channels_json");
      if (msg !== "cancelled") toast(msg);
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#out-upload")!.addEventListener("click", async () => {
    const btn = page.querySelector("#out-upload") as HTMLButtonElement;
    btn.disabled = true;
    try {
      const r = await invoke<{ ok: boolean; text: string }>("publish_channels");
      page.querySelector("#out-up-title")!.textContent = r.ok
        ? "channels.json uploaded"
        : "Upload did not update the API";
      page.querySelector("#out-up-text")!.textContent = r.text;
      page.querySelector("#out-up-dlg")!.classList.add("open");
    } catch (e) {
      toast(String(e));
    } finally {
      await reload().catch(() => undefined);
    }
  });
  page.querySelector("#out-up-close")!.addEventListener("click", () => {
    page.querySelector("#out-up-dlg")!.classList.remove("open");
  });
  page.querySelector("#out-undo")!.addEventListener("click", async () => {
    try {
      const ok = await invoke<boolean>("audit_undo");
      toast(ok ? "Swap undone" : "Nothing to undo");
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });

  const dlg = page.querySelector("#out-lineup-dlg")!;
  const paintPicks = () => {
    const q = (page.querySelector("#out-lq") as HTMLInputElement).value.trim().toLowerCase();
    const el = page.querySelector("#out-lpicks")!;
    el.innerHTML = "";
    for (const p of picks) {
      if (q && !p.name.toLowerCase().includes(q) && !p.group.toLowerCase().includes(q)) continue;
      const row = document.createElement("div");
      row.className = "lineup-row";
      row.innerHTML = `<label class="check"><input type="checkbox" data-id="${esc(p.id)}" ${p.included ? "checked" : ""} />
        <span><span class="chan-name">${esc(p.name)}</span><span class="chan-sub">${esc(p.group)}</span></span></label>
        <input class="lineup-num" data-nid="${esc(p.id)}" placeholder="#" value="${p.number ?? ""}" />`;
      el.appendChild(row);
    }
    el.querySelectorAll<HTMLInputElement>("input[data-id]").forEach((box) => {
      box.addEventListener("change", () => {
        const p = picks.find((x) => x.id === box.dataset.id);
        if (p) p.included = box.checked;
      });
    });
    el.querySelectorAll<HTMLInputElement>("input[data-nid]").forEach((inp) => {
      inp.addEventListener("change", () => {
        const p = picks.find((x) => x.id === inp.dataset.nid);
        if (!p) return;
        const n = parseInt(inp.value.trim(), 10);
        p.number = Number.isFinite(n) && n > 0 ? n : null;
      });
    });
  };

  page.querySelector("#out-lineup")!.addEventListener("click", async () => {
    try {
      picks = await invoke<TunerPickRow[]>("lineup_candidates");
      (page.querySelector("#out-lq") as HTMLInputElement).value = "";
      paintPicks();
      dlg.classList.add("open");
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#out-lq")!.addEventListener("input", paintPicks);
  page.querySelector("#out-auto")!.addEventListener("click", () => {
    let targets = picks.filter((p) => p.included);
    if (targets.length === 0) targets = picks;
    let n = 1;
    for (const p of targets) {
      p.included = true;
      p.number = n++;
    }
    paintPicks();
  });
  page.querySelector("#out-lcancel")!.addEventListener("click", () => dlg.classList.remove("open"));
  page.querySelector("#out-lsave")!.addEventListener("click", async () => {
    try {
      const msg = await invoke<string>("save_tuner_lineup", {
        picks: picks.map((p) => ({ id: p.id, included: p.included, number: p.number ?? null })),
      });
      dlg.classList.remove("open");
      toast(msg);
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });

  page.addEventListener("studio-search", (ev) => {
    filter = (ev as CustomEvent<string>).detail ?? "";
    void reload().catch((e) => toast(String(e)));
  });

  try {
    await reload();
  } catch (e) {
    toast(String(e));
  }
}

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}
