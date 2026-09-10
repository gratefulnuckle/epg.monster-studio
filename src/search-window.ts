import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, type Channel } from "./api";
import { applyPlayGate, canPlay } from "./capabilities";
import { bindVirtualList, type VirtualList } from "./virtual";

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

function decodeEntities(s: string): string {
  return s
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'");
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

function initialQuery(): string {
  const w = window as Window & { __SOURCE_SEARCH_Q?: string };
  if (typeof w.__SOURCE_SEARCH_Q === "string") return w.__SOURCE_SEARCH_Q;
  try {
    return new URLSearchParams(window.location.search).get("q") ?? "";
  } catch {
    return "";
  }
}

export async function mountSourceSearchWindow(root: HTMLElement): Promise<void> {
  document.title = "Source search";
  root.innerHTML = `
    <div class="shell catalog-shell">
      <header class="titlebar">
        <div class="titlebar-side titlebar-left">
          <div class="titlebar-drag titlebar-spacer" data-tauri-drag-region></div>
        </div>
        <div class="titlebar-title" data-tauri-drag-region>Source search</div>
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
        <div class="field"><label>Search</label><input id="ss-q" placeholder="name, group, tvg-id, URL…" /></div>
        <p class="page-sub" id="ss-status">Type at least 2 characters.</p>
        <div id="ss-list" class="editor-list catalog-list"></div>
      </main>
    </div>
  `;
  wireCaption(root);
  applyPlayGate(root);

  const qEl = root.querySelector<HTMLInputElement>("#ss-q")!;
  const status = root.querySelector<HTMLElement>("#ss-status")!;
  const list = root.querySelector<HTMLElement>("#ss-list")!;
  let last: Channel[] = [];
  let virt: VirtualList<Channel> | null = null;
  let timer = 0;
  let hasManaged = false;
  void api.managedCount().then((n) => {
    hasManaged = n > 0;
  });

  const paint = (chans: Channel[], q: string) => {
    last = chans;
    virt?.destroy();
    list.innerHTML = "";
    if (q.trim().length < 2) {
      status.textContent = "Type at least 2 characters.";
      return;
    }
    status.textContent = `${chans.length.toLocaleString()} match${chans.length === 1 ? "" : "es"}`;
    const addCol = hasManaged ? "" : " no-add";
    const head = document.createElement("div");
    head.className = "chan-head" + addCol;
    head.innerHTML = `<span class="col-icon">Play</span>${hasManaged ? "<span class=\"col-icon\">Add</span>" : ""}<span>Name</span>`;
    virt = bindVirtualList({
      scroller: list,
      rowHeight: 48,
      header: head,
      renderRow: (c) => {
        const row = document.createElement("div");
        row.className = "chan-row" + addCol;
        row.innerHTML = `
          <button class="play" data-id="${esc(c.id)}" data-url="${esc(c.url)}" data-sid="${esc(c.sourceId)}" title="${canPlay() ? "Play stream" : "Install mpv or VLC"}" ${canPlay() ? "" : "disabled"}>&#xE768;</button>
          ${hasManaged ? `<button class="add-pl" data-id="${esc(c.id)}" title="Add to managed playlist">&#xE710;</button>` : ""}
          <div class="chan-main">
            <div class="chan-meta">
              <div class="chan-name" title="${esc(decodeEntities(c.name))}">${esc(decodeEntities(c.name))}</div>
              <div class="chan-sub">${esc(decodeEntities(c.groupTitle))}</div>
            </div>
          </div>`;
        return row;
      },
    });
    virt.setItems(chans);
  };

  const run = async (q: string) => {
    qEl.value = q;
    if (q.trim().length < 2) {
      paint([], q);
      return;
    }
    status.textContent = "Searching…";
    try {
      const hits = await api.searchSources(q);
      paint(hits, q);
    } catch (e) {
      status.textContent = String(e);
    }
  };

  qEl.addEventListener("input", () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(() => void run(qEl.value), 300);
  });
  qEl.addEventListener("keydown", (ev) => {
    if (ev.key !== "Enter") return;
    window.clearTimeout(timer);
    void run(qEl.value);
  });

  list.addEventListener("click", async (ev) => {
    const t = ev.target as HTMLElement;
    if (t.classList.contains("play")) {
      const found = t.dataset.id ? last.find((c) => c.id === t.dataset.id) : undefined;
      const url = decodeEntities(found?.url ?? t.dataset.url ?? "");
      const sid = found?.sourceId ?? t.dataset.sid;
      try {
        await api.playUrl(url, sid);
      } catch (e) {
        status.textContent = String(e);
      }
      return;
    }
    if (t.classList.contains("add-pl") && t.dataset.id) {
      try {
        const ch = await api.addFromSource(t.dataset.id);
        status.textContent = `Added ${ch.name}`;
      } catch (e) {
        status.textContent = String(e);
      }
    }
  });

  void listen<string>("source-search-set-query", (ev) => {
    void run(ev.payload ?? "");
  });

  const start = initialQuery();
  qEl.value = start;
  if (start.trim().length >= 2) void run(start);
  else qEl.focus();
}
