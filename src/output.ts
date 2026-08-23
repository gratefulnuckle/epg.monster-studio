import { invoke } from "@tauri-apps/api/core";
import { bindVirtualList, type VirtualList } from "./virtual";

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

export function outputHtml(): string {
  return `
    <h1 class="page-title">Managed Output</h1>
    <p class="page-sub">Only the visible stream of each channel is exported. Hidden backups stay in the database for manual or auto swap during audit.</p>
    <div class="editor-workspace">
    <div class="tabs-row">
      <div class="split-menu-wrap" id="out-export-wrap">
        <button class="accent" id="out-export" type="button">Export</button>
        <div class="split-menu" id="out-export-menu" hidden>
          <button type="button" data-export="vis">Export visible m3u8</button>
          <button type="button" data-export="all">Export all</button>
          <button type="button" data-export="json">Export channels.json</button>
        </div>
      </div>
      <button class="accent" id="out-upload">Upload channels.json</button>
      <button id="out-undo">Undo last swap</button>
      <span class="page-sub" id="out-summary"></span>
      <div class="source-row-actions">
        <button type="button" class="tab-ico" id="out-refresh" title="Refresh">&#xE72C;</button>
      </div>
    </div>
    <div class="output-table">
      <div class="output-head">
        <span>Name</span><span>Group</span><span>tvg-id</span>
        <span>Visible URL</span><span>Variants</span><span>Audit</span>
      </div>
      <div id="out-rows" class="editor-list"></div>
    </div>
    <p class="page-sub" id="out-footer"></p>
    <div class="dialog-backdrop" id="out-up-dlg">
      <div class="dialog">
        <h2 id="out-up-title">channels.json uploaded</h2>
        <pre id="out-up-text" class="page-sub" style="white-space:pre-wrap;user-select:text"></pre>
        <div class="dialog-actions"><button id="out-up-close">Close</button></div>
      </div>
    </div>
    </div>
  `;
}

export async function mountOutput(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  let filter = "";
  let rowVirt: VirtualList<OutputRow> | null = null;

  const reload = async () => {
    const sum = await invoke<OutputSummary>("output_summary", { filter: filter || null });
    const el = page.querySelector<HTMLElement>("#out-rows");
    if (!el) return;
    rowVirt?.destroy();
    el.innerHTML = "";
    const table = page.querySelector<HTMLElement>(".output-table");
    if (table) {
      const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));
      const maxes = [4, 5, 6, 11, 8, 5];
      for (const r of sum.rows) {
        maxes[0] = Math.max(maxes[0], r.name.length);
        maxes[1] = Math.max(maxes[1], r.group.length);
        maxes[2] = Math.max(maxes[2], r.tvgId.length);
        maxes[3] = Math.max(maxes[3], r.visibleUrl.length);
        maxes[4] = Math.max(maxes[4], r.variantsSummary.length);
        maxes[5] = Math.max(maxes[5], r.auditStatus.length);
      }
      const cols = [
        clamp(maxes[0], 8, 28),
        clamp(maxes[1], 6, 16),
        clamp(maxes[2], 6, 18),
        clamp(maxes[3], 10, 36),
        clamp(maxes[4], 6, 12),
        clamp(maxes[5], 5, 10),
      ]
        .map((w) => `minmax(0, ${w}fr)`)
        .join(" ");
      table.style.setProperty("--out-cols", cols);
    }
    rowVirt = bindVirtualList({
      scroller: el,
      rowHeight: 36,
      renderRow: (r) => {
        const row = document.createElement("div");
        row.className = "output-row";
        const cell = (text: string, extra = "") =>
          `<span class="${extra}" title="${esc(text)}">${esc(text)}</span>`;
        row.innerHTML = `${cell(r.name, "chan-name")}${cell(r.group)}${cell(r.tvgId)}${cell(r.visibleUrl, "chan-sub")}${cell(r.variantsSummary)}${cell(r.auditStatus)}`;
        return row;
      },
    });
    rowVirt.setItems(sum.rows);
    page.querySelector("#out-summary")!.textContent =
      `${sum.rows.length} channels · ${sum.recentSwaps} recent swaps`;
    (page.querySelector("#out-upload") as HTMLButtonElement).disabled = !sum.hasKey;
    page.querySelector("#out-footer")!.textContent =
      `Only the visible stream of each channel is exported. Hidden backups stay in the database. ` +
      `TV Tuner channel list: ${sum.tunerCount} channel(s). Media servers enabled: ${sum.enabledTuners}.` +
      (sum.hasKey ? "" : " Add a my.epg.monster key in Settings to upload channels.json.");
  };

  page.querySelector("#out-refresh")!.addEventListener("click", () => void reload().catch((e) => toast(String(e))));
  const exportMenu = page.querySelector<HTMLElement>("#out-export-menu")!;
  const closeExportMenu = () => {
    exportMenu.hidden = true;
  };
  page.querySelector("#out-export")!.addEventListener("click", (ev) => {
    ev.stopPropagation();
    exportMenu.hidden = !exportMenu.hidden;
  });
  exportMenu.addEventListener("click", (ev) => {
    ev.stopPropagation();
    const t = (ev.target as HTMLElement).closest("button[data-export]") as HTMLButtonElement | null;
    if (!t) return;
    closeExportMenu();
    const kind = t.dataset.export;
    void (async () => {
      try {
        if (kind === "vis") {
          const msg = await invoke<string>("export_managed", { includeBackups: false });
          if (msg !== "cancelled") toast(msg);
        } else if (kind === "all") {
          const msg = await invoke<string>("export_managed", { includeBackups: true });
          if (msg !== "cancelled") toast(msg);
        } else if (kind === "json") {
          const msg = await invoke<string>("export_channels_json");
          if (msg !== "cancelled") toast(msg);
        }
      } catch (e) {
        toast(String(e));
      }
    })();
  });
  page.addEventListener("click", () => closeExportMenu());
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
