import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, type Channel, type Source } from "./api";
import { editorHtml, mountEditor } from "./editor";
import { epgHtml, mountEpg } from "./epg";
import { logoHtml, mountLogo } from "./logo";
import { auditHtml, mountAudit } from "./audit";
import { outputHtml, mountOutput } from "./output";
import { tunerHtml, mountTuner } from "./tuner";
import { settingsHtml, mountSettings } from "./settings";

export type NavId =
  | "audit"
  | "editor"
  | "epg"
  | "logoaudit"
  | "autoaudit"
  | "output"
  | "tuner"
  | "settings";

const NAV: { id: NavId; label: string }[] = [
  { id: "audit", label: "Add Sources" },
  { id: "editor", label: "Playlist Editor" },
  { id: "epg", label: "EPG Audit" },
  { id: "logoaudit", label: "Logo Audit" },
  { id: "autoaudit", label: "Stream Audit" },
  { id: "output", label: "Managed Output" },
  { id: "tuner", label: "TV Tuner" },
];

const SEARCH_PAGES: NavId[] = ["audit", "editor", "output"];

export function mountShell(root: HTMLElement): void {
  root.innerHTML = `
    <div class="shell">
      <header class="titlebar">
        <div class="titlebar-drag" data-tauri-drag-region>
          <span class="brand">epg.monster studio</span>
        </div>
        <input class="search" id="search" placeholder="Search name, group, tvg-id, URL…" />
        <div class="titlebar-drag titlebar-spacer" data-tauri-drag-region></div>
        <div class="caption">
          <button type="button" class="caption-btn" id="win-min" title="Minimize" aria-label="Minimize">&#xE921;</button>
          <button type="button" class="caption-btn" id="win-max" title="Maximize" aria-label="Maximize">&#xE922;</button>
          <button type="button" class="caption-btn" id="win-close" title="Close" aria-label="Close">&#xE8BB;</button>
        </div>
      </header>
      <div class="workspace">
        <aside class="nav">
          <button class="nav-logo" id="about" title="About epg.monster studio">
            <img src="/logo.png" alt="epg.monster studio" />
          </button>
          <div class="nav-items" id="nav-items"></div>
          <div class="nav-footer">
            <button class="nav-item" data-nav="settings">Settings</button>
          </div>
        </aside>
        <main class="page" id="page"></main>
      </div>
      <div class="toast" id="toast"></div>
      <div class="dialog-backdrop" id="about-dlg">
        <div class="dialog">
          <h2>About epg.monster studio</h2>
          <p>epg.monster studio</p>
          <p>GNU General Public License v3.0</p>
          <p><a href="https://www.gnu.org/licenses/gpl-3.0.html">https://www.gnu.org/licenses/gpl-3.0.html</a></p>
          <div class="dialog-actions">
            <button id="about-close">Close</button>
          </div>
        </div>
      </div>
    </div>
  `;

  const items = root.querySelector("#nav-items")!;
  for (const item of NAV) {
    const b = document.createElement("button");
    b.className = "nav-item";
    b.dataset.nav = item.id;
    b.textContent = item.label;
    items.appendChild(b);
  }

  const page = root.querySelector<HTMLElement>("#page")!;
  const search = root.querySelector<HTMLInputElement>("#search")!;
  const toast = root.querySelector<HTMLElement>("#toast")!;
  const aboutDlg = root.querySelector<HTMLElement>("#about-dlg")!;

  const showToast = (text: string) => {
    toast.textContent = text;
    toast.classList.add("open");
    window.setTimeout(() => toast.classList.remove("open"), 3000);
  };

  let current: NavId = "audit";

  const render = (id: NavId) => {
    current = id;
    root.querySelectorAll(".nav-item").forEach((el) => {
      el.classList.toggle("active", (el as HTMLElement).dataset.nav === id);
    });
    search.classList.toggle("hidden", !SEARCH_PAGES.includes(id));
    page.innerHTML = pageHtml(id);
    if (id === "audit") void mountSources(page, showToast);
    if (id === "editor") void mountEditor(page, showToast);
    if (id === "epg") void mountEpg(page, showToast);
    if (id === "logoaudit") void mountLogo(page, showToast);
    if (id === "autoaudit") void mountAudit(page, showToast);
    if (id === "output") void mountOutput(page, showToast);
    if (id === "tuner") void mountTuner(page, showToast);
    if (id === "settings") void mountSettings(page, showToast);
  };

  root.querySelectorAll<HTMLButtonElement>("[data-nav]").forEach((btn) => {
    btn.addEventListener("click", () => render(btn.dataset.nav as NavId));
  });

  root.querySelector("#about")!.addEventListener("click", () => aboutDlg.classList.add("open"));
  root.querySelector("#about-close")!.addEventListener("click", () => aboutDlg.classList.remove("open"));
  wireCaptionButtons(root);

  let searchTimer = 0;
  search.addEventListener("input", () => {
    window.clearTimeout(searchTimer);
    searchTimer = window.setTimeout(() => {
      if (SEARCH_PAGES.includes(current)) {
        page.dispatchEvent(new CustomEvent("studio-search", { detail: search.value }));
      }
    }, 200);
  });

  render("audit");
  void listen<string>("studio-navigate", (ev) => {
    const id = ev.payload as NavId;
    if (id) render(id);
  });
}

function wireCaptionButtons(root: HTMLElement): void {
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
    void (async () => {
      await win.hide();
      await invoke("mark_tray_state");
    })();
  });
  maxBtn.addEventListener("click", () => {
    void win.toggleMaximize().then(() => paintMax());
  });
  root.querySelector("#win-close")!.addEventListener("click", () => {
    void (async () => {
      await win.hide();
      await invoke("mark_tray_state");
    })();
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

async function mountSources(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  const tabs = page.querySelector("#source-tabs")!;
  const groupsEl = page.querySelector("#source-groups")!;
  const channelsEl = page.querySelector("#source-channels")!;
  const empty = page.querySelector<HTMLElement>("#source-empty")!;
  const workspace = page.querySelector<HTMLElement>("#source-workspace")!;
  const urlDlg = page.querySelector<HTMLElement>("#url-dlg")!;

  let sources: Source[] = [];
  let activeId = "";
  let searching = false;

  const paintTabs = () => {
    tabs.innerHTML = "";
    for (const s of sources) {
      const b = document.createElement("button");
      b.className = "tab" + (s.id === activeId ? " active" : "");
      b.textContent = `${s.name} (${s.channelCount})`;
      b.addEventListener("click", () => {
        activeId = s.id;
        paintTabs();
        void loadGroups();
      });
      const x = document.createElement("span");
      x.className = "tab-x";
      x.textContent = "×";
      x.title = "Close tab";
      x.addEventListener("click", async (ev) => {
        ev.stopPropagation();
        try {
          await api.removeSource(s.id);
          toast("Source removed.");
          await reload();
        } catch (e) {
          toast(String(e));
        }
      });
      b.appendChild(x);
      tabs.appendChild(b);
    }
    const add = document.createElement("button");
    add.className = "tab add";
    add.textContent = "+";
    add.title = "Add source…";
    add.addEventListener("click", () => urlDlg.classList.add("open"));
    tabs.appendChild(add);
  };

  const loadGroups = async () => {
    if (!activeId) return;
    const groups = await api.listGroups(activeId);
    groupsEl.innerHTML = "";
    for (const g of groups) {
      const row = document.createElement("button");
      row.className = "group-row";
      row.textContent = `${g.title}  (${g.count})`;
      row.addEventListener("click", async () => {
        groupsEl.querySelectorAll(".group-row").forEach((el) => el.classList.remove("active"));
        row.classList.add("active");
        const chans = await api.listChannels(activeId, g.title);
        paintChannels(chans, false);
      });
      groupsEl.appendChild(row);
    }
    if (groups[0]) (groupsEl.firstElementChild as HTMLButtonElement | null)?.click();
  };

  const paintChannels = (chans: Channel[], isSearch: boolean) => {
    searching = isSearch;
    channelsEl.innerHTML = "";
    for (const c of chans) {
      const row = document.createElement("div");
      row.className = "chan-row";
      row.innerHTML = `
        <button class="play" data-url="${escapeAttr(c.url)}" data-sid="${escapeAttr(c.sourceId)}">Play</button>
        <div class="chan-meta">
          <div class="chan-name">${escapeHtml(c.name)}</div>
          <div class="chan-sub">${escapeHtml(c.groupTitle)}${c.tvgId ? " · " : ""}<span class="copy" data-copy="${escapeAttr(c.tvgId ?? "")}">${escapeHtml(c.tvgId ?? "")}</span></div>
        </div>
        <span class="copy url" data-copy="${escapeAttr(c.url)}" title="${escapeAttr(c.url)}">${escapeHtml(truncate(c.url, 64))}</span>
      `;
      channelsEl.appendChild(row);
    }
  };

  const reload = async () => {
    sources = await api.listSources();
    const has = sources.length > 0;
    empty.style.display = has ? "none" : "block";
    workspace.style.display = has ? "grid" : "none";
    if (!has) {
      activeId = "";
      return;
    }
    if (!sources.some((s) => s.id === activeId)) activeId = sources[0].id;
    paintTabs();
    await loadGroups();
  };

  page.addEventListener("studio-search", async (ev) => {
    const q = (ev as CustomEvent<string>).detail ?? "";
    if (q.trim().length < 2) {
      if (searching) await loadGroups();
      return;
    }
    try {
      const hits = await api.searchSources(q);
      paintChannels(hits, true);
      toast(`${hits.length} match${hits.length === 1 ? "" : "es"}`);
    } catch (e) {
      toast(String(e));
    }
  });

  page.querySelector("#add-file")?.addEventListener("click", async () => {
    try {
      const src = await api.pickSourceFile();
      if (src) {
        toast(`Loaded ${src.name} (${src.channelCount}).`);
        await reload();
      }
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#add-url-open")?.addEventListener("click", () => urlDlg.classList.add("open"));
  page.querySelector("#url-cancel")?.addEventListener("click", () => urlDlg.classList.remove("open"));
  page.querySelector("#url-ok")?.addEventListener("click", async () => {
    const url = (page.querySelector("#url-input") as HTMLInputElement).value.trim();
    const name = (page.querySelector("#url-name") as HTMLInputElement).value.trim();
    const ua = (page.querySelector("#url-ua") as HTMLInputElement).value.trim();
    if (!url) {
      toast("Enter an HTTP(S) URL.");
      return;
    }
    const headers: Record<string, string> = {};
    if (ua) headers["User-Agent"] = ua;
    try {
      const src = await api.addSourceUrl(url, name || undefined, headers);
      urlDlg.classList.remove("open");
      toast(`Loaded ${src.name} (${src.channelCount}).`);
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });

  channelsEl.addEventListener("click", async (ev) => {
    const t = ev.target as HTMLElement;
    if (t.classList.contains("play")) {
      try {
        await api.playUrl(t.dataset.url ?? "", t.dataset.sid);
        toast("Play");
      } catch (e) {
        toast(String(e));
      }
    }
    if (t.classList.contains("copy") && t.dataset.copy) {
      await navigator.clipboard.writeText(t.dataset.copy);
      toast("Copied.");
    }
  });

  try {
    await reload();
  } catch (e) {
    toast(String(e));
  }
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

function escapeAttr(s: string): string {
  return escapeHtml(s);
}

function truncate(s: string, n: number): string {
  return s.length <= n ? s : s.slice(0, n - 1) + "…";
}

function pageHtml(id: NavId): string {
  switch (id) {
    case "audit":
      return `
        <h1 class="page-title">Add Sources</h1>
        <p class="page-sub">Load file or URL playlists (custom headers). Groups + channels, play (mpv/VLC), copy URL/tvg-id.</p>
        <div class="source-bar">
          <button class="accent" id="add-file">Add source…</button>
          <button id="add-url-open">Add URL…</button>
        </div>
        <div class="empty" id="source-empty">
          <div class="glyph">☰</div>
          <p>Add source…</p>
        </div>
        <div class="source-workspace" id="source-workspace">
          <div class="tabs" id="source-tabs"></div>
          <div class="source-split">
            <div class="groups" id="source-groups"></div>
            <div class="channels" id="source-channels"></div>
          </div>
        </div>
        <div class="dialog-backdrop" id="url-dlg">
          <div class="dialog">
            <h2>Add source</h2>
            <div class="field"><label>HTTP(S) URL</label><input id="url-input" placeholder="https://…" /></div>
            <div class="field"><label>Tab name</label><input id="url-name" placeholder="Provider" /></div>
            <div class="field"><label>User-Agent</label><input id="url-ua" /></div>
            <div class="dialog-actions">
              <button id="url-cancel">Cancel</button>
              <button class="accent" id="url-ok">Load</button>
            </div>
          </div>
        </div>`;
    case "editor":
      return editorHtml();
    case "epg":
      return epgHtml();
    case "logoaudit":
      return logoHtml();
    case "autoaudit":
      return auditHtml();
    case "output":
      return outputHtml();
    case "tuner":
      return tunerHtml();
    case "settings":
      return settingsHtml();
  }
}
