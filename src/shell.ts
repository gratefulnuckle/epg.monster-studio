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

const NAV: { id: NavId; label: string; icon: string }[] = [
  { id: "audit", label: "Add Sources", icon: "\uE8A5" },
  { id: "editor", label: "Playlist Editor", icon: "\uE70F" },
  { id: "epg", label: "EPG Audit", icon: "\uE787" },
  { id: "logoaudit", label: "Logo Audit", icon: "\uE91B" },
  { id: "autoaudit", label: "Stream Audit", icon: "\uE895" },
  { id: "output", label: "Managed Output", icon: "\uE8B7" },
  { id: "tuner", label: "TV Tuner", icon: "\uE7F4" },
];

const SEARCH_PAGES: NavId[] = ["audit", "editor", "output"];

export function mountShell(root: HTMLElement): void {
  root.innerHTML = `
    <div class="shell">
      <header class="titlebar">
        <div class="titlebar-side titlebar-left">
          <button type="button" class="pane-toggle" id="pane-toggle" title="Navigation" aria-label="Navigation">&#xE700;</button>
          <div class="titlebar-drag" data-tauri-drag-region>
            <span class="brand">epg.monster studio</span>
          </div>
        </div>
        <div class="search-wrap" id="search-wrap">
          <input class="search" id="search" placeholder="Search name, group, tvg-id, URL…" />
          <span class="search-icon" aria-hidden="true">&#xE721;</span>
        </div>
        <div class="titlebar-side titlebar-right">
          <div class="titlebar-drag titlebar-spacer" data-tauri-drag-region></div>
          <div class="caption">
            <button type="button" class="caption-btn" id="win-min" title="Minimize" aria-label="Minimize">&#xE921;</button>
            <button type="button" class="caption-btn" id="win-max" title="Maximize" aria-label="Maximize">&#xE922;</button>
            <button type="button" class="caption-btn" id="win-close" title="Close" aria-label="Close">&#xE8BB;</button>
          </div>
        </div>
      </header>
      <div class="workspace">
        <aside class="nav">
          <button class="nav-logo" id="about" title="About epg.monster studio">
            <img src="/logo.png" alt="epg.monster studio" />
          </button>
          <div class="nav-items" id="nav-items"></div>
          <div class="nav-footer">
              <button class="nav-item" data-nav="settings"><span class="nav-icon">&#xE713;</span><span class="nav-label">Settings</span></button>
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
    b.innerHTML = `<span class="nav-icon">${item.icon}</span><span class="nav-label"></span>`;
    b.querySelector(".nav-label")!.textContent = item.label;
    items.appendChild(b);
  }

  const page = root.querySelector<HTMLElement>("#page")!;
  const search = root.querySelector<HTMLInputElement>("#search")!;
  const searchWrap = root.querySelector<HTMLElement>("#search-wrap")!;
  const workspace = root.querySelector<HTMLElement>(".workspace")!;
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
    searchWrap.classList.toggle("hidden", !SEARCH_PAGES.includes(id));
    if (!SEARCH_PAGES.includes(id)) search.value = "";
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
  root.querySelector("#pane-toggle")!.addEventListener("click", () => {
    workspace.classList.toggle("pane-closed");
  });
  wireCaptionButtons(root);

  let searchTimer = 0;
  search.addEventListener("input", () => {
    window.clearTimeout(searchTimer);
    searchTimer = window.setTimeout(() => {
      if (SEARCH_PAGES.includes(current)) {
        page.dispatchEvent(new CustomEvent("studio-search", { detail: search.value }));
      }
    }, 300);
  });
  search.addEventListener("keydown", (ev) => {
    if (ev.key !== "Enter") return;
    window.clearTimeout(searchTimer);
    if (SEARCH_PAGES.includes(current)) {
      page.dispatchEvent(new CustomEvent("studio-search", { detail: search.value }));
    }
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
  let hasManaged = false;
  let lastChans: Channel[] = [];

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
    lastChans = chans;
    const addCol = hasManaged ? "" : " no-add";
    channelsEl.innerHTML = `<div class="chan-head${addCol}"><span>Play</span>${hasManaged ? "<span>Add</span>" : ""}<span>Name</span><span>tvg-id</span><span>URL</span></div>`;
    for (const c of chans) {
      const row = document.createElement("div");
      row.className = "chan-row" + addCol;
      row.innerHTML = `
        <button class="play" data-url="${escapeAttr(c.url)}" data-sid="${escapeAttr(c.sourceId)}" title="Play stream">&#xE768;</button>
        ${hasManaged ? `<button class="add-pl" data-id="${escapeAttr(c.id)}" title="Add to managed playlist">&#xE710;</button>` : ""}
        <div class="chan-meta">
          <div class="chan-name">${escapeHtml(c.name)}</div>
          <div class="chan-sub">${escapeHtml(c.groupTitle)}</div>
        </div>
        <span class="copy" data-copy="${escapeAttr(c.tvgId ?? "")}">${escapeHtml(c.tvgId ?? "")}</span>
        <span class="copy url" data-copy="${escapeAttr(c.url)}" title="${escapeAttr(c.url)}">${escapeHtml(truncate(c.url, 64))}</span>
      `;
      channelsEl.appendChild(row);
    }
  };

  const reload = async () => {
    sources = await api.listSources();
    hasManaged = (await api.managedCount()) > 0;
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

  const choiceDlg = page.querySelector<HTMLElement>("#add-choice-dlg")!;
  const backupDlg = page.querySelector<HTMLElement>("#add-backup-dlg")!;
  let pendingAdd: Channel | null = null;
  let backupTarget: string | null = null;

  const openAddChoice = (ch: Channel) => {
    if (!hasManaged) {
      toast("Load a curated playlist in Playlist Editor first");
      return;
    }
    pendingAdd = ch;
    page.querySelector("#add-choice-text")!.textContent =
      `Add “${ch.name}” as a new channel, or as a backup stream on an existing channel?`;
    choiceDlg.classList.add("open");
  };

  const openBackupPicker = async (ch: Channel) => {
    backupTarget = null;
    page.querySelector("#add-backup-text")!.textContent =
      `Pick the managed channel that should get “${ch.name}” as a hidden backup.`;
    const tree = page.querySelector("#add-backup-tree")!;
    tree.innerHTML = "";
    const managed = await api.listManaged();
    if (managed.length === 0) {
      toast("No managed channels to attach a backup to");
      return;
    }
    const groups = new Map<string, { id: string; name: string; groupTitle: string }[]>();
    for (const m of managed) {
      const g = m.groupTitle.trim() || "Ungrouped";
      const list = groups.get(g) ?? [];
      list.push(m);
      groups.set(g, list);
    }
    const keys = [...groups.keys()].sort((a, b) => a.localeCompare(b, undefined, { sensitivity: "base" }));
    for (const key of keys) {
      const wrap = document.createElement("details");
      const sum = document.createElement("summary");
      sum.textContent = key;
      wrap.appendChild(sum);
      const kids = (groups.get(key) ?? []).slice().sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
      for (const m of kids) {
        const b = document.createElement("button");
        b.className = "group-row";
        b.textContent = m.name;
        b.addEventListener("click", () => {
          tree.querySelectorAll(".group-row").forEach((el) => el.classList.remove("active"));
          b.classList.add("active");
          backupTarget = m.id;
        });
        b.addEventListener("dblclick", () => {
          backupTarget = m.id;
          void attachBackup(ch, m.id);
        });
        wrap.appendChild(b);
      }
      tree.appendChild(wrap);
    }
    backupDlg.classList.add("open");
  };

  const attachBackup = async (ch: Channel, managedId: string) => {
    try {
      const name = await api.addBackupFromSource(managedId, ch.id);
      backupDlg.classList.remove("open");
      toast(`Backup added to ${name}`);
    } catch (e) {
      toast(String(e));
    }
  };

  page.querySelector("#add-choice-cancel")!.addEventListener("click", () => choiceDlg.classList.remove("open"));
  page.querySelector("#add-choice-new")!.addEventListener("click", () => {
    if (!pendingAdd) return;
    sessionStorage.setItem("studio-editor-draft", JSON.stringify(pendingAdd));
    choiceDlg.classList.remove("open");
    page.closest(".shell")?.querySelector<HTMLButtonElement>('[data-nav="editor"]')?.click();
  });
  page.querySelector("#add-choice-backup")!.addEventListener("click", () => {
    const ch = pendingAdd;
    choiceDlg.classList.remove("open");
    if (ch) void openBackupPicker(ch);
  });
  page.querySelector("#add-backup-cancel")!.addEventListener("click", () => backupDlg.classList.remove("open"));
  page.querySelector("#add-backup-ok")!.addEventListener("click", () => {
    if (!pendingAdd || !backupTarget) {
      toast("Select a channel (not just a group)");
      return;
    }
    void attachBackup(pendingAdd, backupTarget);
  });

  page.querySelector("#src-refresh")!.addEventListener("click", async () => {
    if (!activeId) {
      toast("No source selected");
      return;
    }
    try {
      const src = await api.refreshSource(activeId);
      toast(`Refreshed ${src.name} (${src.channelCount})`);
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });

  void invoke<Record<string, unknown>>("load_settings").then((st) => {
    const sel = page.querySelector<HTMLSelectElement>("#src-player");
    if (sel) sel.value = String(st.DefaultPlayer ?? 0);
  });
  page.querySelector("#src-player")!.addEventListener("change", async () => {
    const sel = page.querySelector<HTMLSelectElement>("#src-player")!;
    try {
      const st = await invoke<Record<string, unknown>>("load_settings");
      st.DefaultPlayer = parseInt(sel.value, 10) || 0;
      await invoke("save_settings", { settings: st });
    } catch (e) {
      toast(String(e));
    }
  });

  channelsEl.addEventListener("click", async (ev) => {
    const t = ev.target as HTMLElement;
    if (t.classList.contains("play")) {
      try {
        await api.playUrl(t.dataset.url ?? "", t.dataset.sid);
        toast("Playing");
      } catch (e) {
        toast(String(e));
      }
    }
    if (t.classList.contains("add-pl") && t.dataset.id) {
      const found = lastChans.find((c) => c.id === t.dataset.id);
      if (found) openAddChoice(found);
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
          <button id="src-refresh" title="Reload active source">Refresh</button>
          <select id="src-player" title="Player for Play button">
            <option value="0">mpv</option>
            <option value="1">VLC</option>
          </select>
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
        </div>
        <div class="dialog-backdrop" id="add-choice-dlg">
          <div class="dialog">
            <h2>Add to managed playlist</h2>
            <p id="add-choice-text"></p>
            <div class="dialog-actions">
              <button id="add-choice-cancel">Cancel</button>
              <button id="add-choice-backup">Add Backup Source</button>
              <button class="accent" id="add-choice-new">Add New Source</button>
            </div>
          </div>
        </div>
        <div class="dialog-backdrop" id="add-backup-dlg">
          <div class="dialog" style="width:520px;max-height:80vh;overflow:auto">
            <h2>Add backup source</h2>
            <p class="page-sub" id="add-backup-text"></p>
            <div id="add-backup-tree" class="backup-tree"></div>
            <div class="dialog-actions">
              <button id="add-backup-cancel">Cancel</button>
              <button class="accent" id="add-backup-ok">Add</button>
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
