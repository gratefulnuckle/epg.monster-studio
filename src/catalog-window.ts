import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { CatalogEntry } from "./epg";
import { bindVirtualList, type VirtualList } from "./virtual";

type CatRow = { kind: "sec"; title: string } | { kind: "ch"; entry: CatalogEntry };

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

function sectionDisplay(section: string): string {
  if (!section.trim()) return "Other";
  return section.replace(/\d+$/, "") || section;
}

function wireCaption(root: HTMLElement): void {
  const win = getCurrentWindow();
  const maxBtn = root.querySelector<HTMLButtonElement>("#win-max")!;
  const paintMax = async () => {
    try {
      const max = await win.isMaximized();
      maxBtn.innerHTML = max ? "&#xE923;" : "&#xE922;";
      maxBtn.title = max ? "Restore" : "Maximize";
      maxBtn.setAttribute("aria-label", maxBtn.title);
    } catch {
      /* ignore */
    }
  };
  root.querySelector("#win-min")!.addEventListener("click", () => {
    void win.minimize();
  });
  maxBtn.addEventListener("click", () => {
    void win.toggleMaximize().then(() => paintMax());
  });
  root.querySelector("#win-close")!.addEventListener("click", () => {
    void win.close();
  });
  root.querySelectorAll<HTMLElement>("[data-tauri-drag-region]").forEach((el) => {
    el.addEventListener("dblclick", () => {
      void win.toggleMaximize().then(() => paintMax());
    });
  });
  void win.onResized(() => {
    void paintMax();
  });
  void paintMax();
}

export async function mountCatalogWindow(root: HTMLElement): Promise<void> {
  document.title = "EPG catalog";
  root.innerHTML = `
    <div class="shell catalog-shell">
      <header class="titlebar">
        <div class="titlebar-side titlebar-left">
          <div class="titlebar-drag titlebar-spacer" data-tauri-drag-region></div>
        </div>
        <div class="titlebar-title" data-tauri-drag-region>EPG catalog</div>
        <div class="titlebar-side titlebar-right">
          <div class="titlebar-drag titlebar-spacer" data-tauri-drag-region></div>
          <div class="caption">
            <button type="button" class="caption-btn" id="win-min" title="Minimize" aria-label="Minimize">&#xE921;</button>
            <button type="button" class="caption-btn" id="win-max" title="Maximize" aria-label="Maximize">&#xE922;</button>
            <button type="button" class="caption-btn" id="win-close" title="Close" aria-label="Close">&#xE8BB;</button>
          </div>
        </div>
      </header>
      <main class="page catalog-win">
        <p class="page-sub">Search tvg-ids from the XMLTV guide. Click a row to apply it to the selected channel on EPG Audit.</p>
        <div class="field"><label>Filter</label><input id="cat-q" placeholder="id or name…" /></div>
        <p class="page-sub" id="cat-status">Loading catalog…</p>
        <div class="catalog-meter" id="cat-meter"><div class="catalog-meter-fill"></div></div>
        <div id="cat-list" class="editor-list catalog-list"></div>
      </main>
    </div>
  `;
  wireCaption(root);

  const list = root.querySelector<HTMLElement>("#cat-list")!;
  const status = root.querySelector("#cat-status")!;
  const meter = root.querySelector<HTMLElement>("#cat-meter")!;
  const box = root.querySelector<HTMLInputElement>("#cat-q")!;

  const flatten = (rows: CatalogEntry[]): CatRow[] => {
    const out: CatRow[] = [];
    let last = "";
    for (const c of rows) {
      const sec = sectionDisplay(c.section);
      if (sec !== last) {
        out.push({ kind: "sec", title: sec });
        last = sec;
      }
      out.push({ kind: "ch", entry: c });
    }
    return out;
  };

  const virt: VirtualList<CatRow> = bindVirtualList({
    scroller: list,
    rowHeight: 44,
    renderRow: (row) => {
      if (row.kind === "sec") {
        const h = document.createElement("div");
        h.className = "issue-n catalog-sec";
        h.textContent = row.title;
        return h;
      }
      const c = row.entry;
      const b = document.createElement("button");
      b.type = "button";
      b.className = "catalog-row";
      b.innerHTML = `<span class="chan-name">${esc(c.tvgId)}</span><span class="chan-sub">${esc(c.name)}</span>`;
      b.addEventListener("click", () => {
        void emit("epg-catalog-pick", { tvgId: c.tvgId, name: c.name });
        status.textContent = `Sent ${c.tvgId} to EPG Audit`;
      });
      return b;
    },
  });

  const load = async (q: string) => {
    status.textContent = "Loading catalog…";
    meter.hidden = false;
    try {
      const needle = q.trim();
      const rows = needle
        ? await invoke<CatalogEntry[]>("epg_browse_catalog", { query: needle })
        : await invoke<CatalogEntry[]>("epg_browse_catalog", {});
      let total = 0;
      try {
        total = await invoke<number>("epg_catalog_count");
      } catch {
        /* count is optional */
      }
      virt.setItems(flatten(rows));
      if (!rows.length) {
        status.textContent = total
          ? "No matches — try a different id or name"
          : "Catalog is empty. Fetch / refresh catalog on EPG Audit first.";
      } else if (!needle && total > rows.length) {
        status.textContent = `${rows.length} shown of ${total.toLocaleString()} — type to search`;
      } else if (total) {
        status.textContent = `${rows.length} shown of ${total.toLocaleString()}`;
      } else {
        status.textContent = `${rows.length} shown`;
      }
    } catch (e) {
      virt.setItems([]);
      status.textContent = String(e);
    } finally {
      meter.hidden = true;
    }
  };

  let t = 0;
  box.addEventListener("input", () => {
    window.clearTimeout(t);
    t = window.setTimeout(() => void load(box.value), 200);
  });

  await load("");
  box.focus();
}
