import { invoke } from "@tauri-apps/api/core";
import {
  applyPlayGate,
  applyPlayerEngineValue,
  canPlay,
  canStreamAudit,
  playerEngineOptionsHtml,
} from "./capabilities";
import { listen } from "@tauri-apps/api/event";

type SourceProgress = {
  id: string;
  name: string;
  channelCount: number;
  done: boolean;
  error?: string | null;
  expiresAt?: number | null;
  op?: string;
};

let onSourceProgress: ((p: SourceProgress) => void) | null = null;
let sourceProgressHooked = false;
const pendingSourceIds = new Set<string>();
const refreshingIds = new Set<string>();

function handleSourceProgress(p: SourceProgress): void {
  if (p.done) {
    pendingSourceIds.delete(p.id);
    refreshingIds.delete(p.id);
  }
  onSourceProgress?.(p);
}
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api, type Channel, type ChannelAudit, type Group, type Source } from "./api";
import { editorHtml, mountEditor } from "./editor";
import { epgHtml, mountEpg } from "./epg";
import { logoHtml, mountLogo } from "./logo";
import { auditHtml, mountAudit } from "./audit";
import { outputHtml, mountOutput } from "./output";
import { tunerHtml, mountTuner } from "./tuner";
import { settingsHtml, mountSettings } from "./settings";
import { updatesHtml, mountUpdates } from "./updates";
import { bindVirtualList, type VirtualList } from "./virtual";
import { bindColResize } from "./split";

const AUDIT_STORE_KEY = "studio-src-audit-v1";
type ProbeEntry =
  | { state: "run" }
  | { state: "done"; result: ChannelAudit; until: number };
const probeById = new Map<string, ProbeEntry>();
const PROBE_RESULT_MS = 60_000;
let probeTtlMs = PROBE_RESULT_MS;
let probeExpireTimer = 0;
let auditInFlight = 0;
const AUDIT_MAX = 2;
let onProbeExpired: (() => void) | null = null;

function loadProbeStore(): void {
  if (probeById.size > 0) return;
  try {
    const raw = window.localStorage.getItem(AUDIT_STORE_KEY);
    if (!raw) return;
    const parsed = JSON.parse(raw) as Record<string, ChannelAudit | { result: ChannelAudit; until: number }>;
    const now = Date.now();
    for (const [id, entry] of Object.entries(parsed)) {
      const result = "grade" in entry ? entry : entry.result;
      const until = "until" in entry ? entry.until : now + probeTtlMs;
      if (result && typeof result.grade === "string" && until > now) {
        probeById.set(id, { state: "done", result, until });
      }
    }
  } catch {
    /* ignore bad cache */
  }
}

function saveProbeStore(): void {
  const out: Record<string, { result: ChannelAudit; until: number }> = {};
  let n = 0;
  const now = Date.now();
  for (const [id, v] of probeById) {
    if (v.state !== "done" || v.until <= now) continue;
    out[id] = { result: v.result, until: v.until };
    n += 1;
    if (n >= 800) break;
  }
  try {
    window.localStorage.setItem(AUDIT_STORE_KEY, JSON.stringify(out));
  } catch {
    /* quota */
  }
}

function pruneProbeStore(): boolean {
  const now = Date.now();
  let changed = false;
  for (const [id, v] of [...probeById]) {
    if (v.state === "done" && v.until <= now) {
      probeById.delete(id);
      changed = true;
    }
  }
  if (changed) saveProbeStore();
  return changed;
}

function ensureProbeExpireTimer(): void {
  if (probeExpireTimer) return;
  probeExpireTimer = window.setInterval(() => {
    if (pruneProbeStore()) onProbeExpired?.();
  }, 1000);
}

loadProbeStore();

export type NavId =
  | "audit"
  | "editor"
  | "epg"
  | "logoaudit"
  | "autoaudit"
  | "output"
  | "tuner"
  | "updates"
  | "settings";

function navItems(): { id: NavId; label: string; icon?: string; img?: string }[] {
  return [
    { id: "audit", label: "Add Sources", icon: "\uE8A5" },
    { id: "editor", label: "Playlist Editor", icon: "\uE70F" },
    { id: "epg", label: "EPG Audit", icon: "\uE787" },
    { id: "logoaudit", label: "Logo Audit", icon: "\uE91B" },
    { id: "autoaudit", label: "Stream Audit", icon: "\uE895" },
    { id: "output", label: "Managed Output", icon: "\uE8B7" },
    { id: "tuner", label: "TV Tuner", icon: "\uE7F4" },
  ];
}

const SEARCH_PAGES: NavId[] = ["audit", "editor", "output"];

export function mountShell(root: HTMLElement): { toast: (s: string) => void } {
  root.innerHTML = `
    <div class="shell">
      <header class="titlebar">
        <div class="titlebar-side titlebar-left">
          <button type="button" class="pane-toggle" id="pane-toggle" title="Navigation" aria-label="Navigation">&#xE700;</button>
          <div class="titlebar-drag titlebar-spacer" data-tauri-drag-region></div>
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
          <div class="nav-spacer" aria-hidden="true"></div>
          <button class="nav-logo" id="about" title="About epg.monster studio">
            <img src="/logo.png" alt="epg.monster studio" />
          </button>
          <div class="nav-items" id="nav-items"></div>
          <div class="nav-footer">
              <button class="nav-item" data-nav="updates"><span class="nav-icon">&#xE896;</span><span class="nav-label">Check For Updates</span></button>
              <button class="nav-item" data-nav="settings"><span class="nav-icon">&#xE713;</span><span class="nav-label">Settings</span></button>
          </div>
        </aside>
        <main class="page" id="page"></main>
      </div>
      <div class="toast" id="toast" data-sev="success">
        <div class="toast-body">
          <div class="toast-title" id="toast-title"></div>
          <div class="toast-msg" id="toast-msg"></div>
        </div>
        <button type="button" class="toast-close" id="toast-close" title="Close" aria-label="Close">&#xE711;</button>
      </div>
      <div class="dialog-backdrop" id="about-dlg">
        <div class="dialog" style="width:520px;max-height:80vh;overflow:auto;text-align:center">
          <h2>About</h2>
          <img src="/logo.png" alt="epg.monster studio" style="width:96px;height:96px" />
          <p class="chan-name" style="font-size:22px">epg.monster studio</p>
          <p class="page-sub" id="about-ver">2026 edition · v2.0.2 (dev)</p>
          <div style="text-align:left">
            <p><strong>What it is</strong><br />A Windows workspace for a legal IPTV lineup: import sources, curate channels and backups, match EPG from epg.monster, check logos and streams, then publish a managed playlist or a local virtual tuner.</p>
            <p><strong>Built with</strong><br />Rust and TypeScript, Tauri v2, SQLite (rusqlite). Play uses mpv or VLC from the paths in Settings. Probes use ffmpeg / ffprobe.</p>
            <p><strong>License</strong><br />GNU General Public License v3.0. ffmpeg and mpv are also GNU GPL. You may run, study, share, and change this software under the GPL. Full text: <a href="https://www.gnu.org/licenses/gpl-3.0.html">https://www.gnu.org/licenses/gpl-3.0.html</a></p>
          </div>
          <div class="dialog-actions">
            <button id="about-close">Close</button>
          </div>
        </div>
      </div>
    </div>
  `;

  const items = root.querySelector("#nav-items")!;
  for (const item of navItems()) {
    const b = document.createElement("button");
    b.className = "nav-item";
    b.dataset.nav = item.id;
    if (item.img) {
      b.innerHTML = `<img class="nav-icon-img" alt="" /><span class="nav-label"></span>`;
      (b.querySelector("img") as HTMLImageElement).src = item.img;
    } else {
      b.innerHTML = `<span class="nav-icon"></span><span class="nav-label"></span>`;
      b.querySelector(".nav-icon")!.textContent = item.icon ?? "";
    }
    b.querySelector(".nav-label")!.textContent = item.label;
    items.appendChild(b);
  }

  const applyNavGates = () => {
    const auditBtn = items.querySelector<HTMLButtonElement>('[data-nav="autoaudit"]');
    if (auditBtn) {
      const ok = canStreamAudit();
      auditBtn.disabled = !ok;
      auditBtn.classList.toggle("is-disabled", !ok);
      auditBtn.title = ok
        ? ""
        : "Install ffmpeg and ffprobe (studio.ps1 / studio.sh --install), or set paths in Settings";
    }
  };
  applyNavGates();
  window.addEventListener("studio-tools-changed", applyNavGates);

  const page = root.querySelector<HTMLElement>("#page")!;
  const search = root.querySelector<HTMLInputElement>("#search")!;
  const searchWrap = root.querySelector<HTMLElement>("#search-wrap")!;
  const workspace = root.querySelector<HTMLElement>(".workspace")!;
  const toast = root.querySelector<HTMLElement>("#toast")!;
  const aboutDlg = root.querySelector<HTMLElement>("#about-dlg")!;

  let toastTimer = 0;
  const inferSev = (m: string): "success" | "warning" | "error" => {
    const t = m.toLowerCase();
    if (
      t.includes("error") ||
      t.includes("failed") ||
      t.includes("panic") ||
      t.includes("exception") ||
      t.includes("not found at")
    ) {
      return "error";
    }
    if (
      t.includes("warning") ||
      t.startsWith("load a ") ||
      t.startsWith("pick ") ||
      t.startsWith("select ") ||
      t.startsWith("paste ") ||
      t.includes("first")
    ) {
      return "warning";
    }
    return "success";
  };
  const showToast = (text: string, severity?: "success" | "warning" | "error") => {
    // Stale page updates after nav throw this; don't flash it as a studio message.
    if (/Cannot set propert(?:ies|y) of null/i.test(text)) return;
    const sev = severity ?? inferSev(text);
    toast.dataset.sev = sev;
    const title = toast.querySelector("#toast-title");
    const msg = toast.querySelector("#toast-msg");
    if (title) title.textContent = sev === "error" ? "Error" : "epg.monster studio";
    if (msg) msg.textContent = text;
    else toast.textContent = text;
    toast.classList.add("open");
    window.clearTimeout(toastTimer);
    toastTimer = window.setTimeout(() => toast.classList.remove("open"), 3000);
  };
  toast.querySelector("#toast-close")?.addEventListener("click", () => {
    toast.classList.remove("open");
    window.clearTimeout(toastTimer);
  });

  let current: NavId = "audit";
  let disposePage: (() => void) | undefined;

  const render = (id: NavId) => {
    if (id === "autoaudit" && !canStreamAudit()) {
      showToast("Stream Audit needs ffmpeg and ffprobe. Run studio.ps1 / studio.sh --install or set paths in Settings.");
      return;
    }
    current = id;
    disposePage?.();
    disposePage = undefined;
    root.querySelectorAll(".nav-item").forEach((el) => {
      el.classList.toggle("active", (el as HTMLElement).dataset.nav === id);
    });
    const hideSearch = !SEARCH_PAGES.includes(id);
    searchWrap.classList.toggle("hidden", hideSearch);
    if (hideSearch) {
      search.value = "";
      searchWrap.setAttribute("data-tauri-drag-region", "");
    } else {
      searchWrap.removeAttribute("data-tauri-drag-region");
    }
    page.innerHTML = pageHtml(id);
    if (id === "audit") {
      void mountSources(page, showToast).then(() => applyPlayGate(page));
    }
    if (id === "editor") {
      void mountEditor(page, showToast).then(() => applyPlayGate(page));
    }
    if (id === "epg") {
      let stop: (() => void) | undefined;
      disposePage = () => stop?.();
      void mountEpg(page, showToast).then((s) => {
        stop = s;
      });
    }
    if (id === "logoaudit") void mountLogo(page, showToast);
    if (id === "autoaudit") void mountAudit(page, showToast);
    if (id === "output") void mountOutput(page, showToast);
    if (id === "tuner") {
      let stop: (() => void) | undefined;
      disposePage = () => stop?.();
      void mountTuner(page, showToast).then((s) => {
        stop = s;
      });
    }
    if (id === "updates") void mountUpdates(page, showToast);
    if (id === "settings") void mountSettings(page, showToast);
  };

  root.querySelectorAll<HTMLButtonElement>("[data-nav]").forEach((btn) => {
    btn.addEventListener("click", () => {
      if (btn.disabled) return;
      render(btn.dataset.nav as NavId);
    });
  });

  root.querySelector("#about")!.addEventListener("click", () => {
    aboutDlg.classList.add("open");
    void invoke<{ version: string; displayVersion?: string; displayName: string; edition?: string }>("get_studio_info")
      .then((info) => {
        const el = root.querySelector("#about-ver");
        if (el) el.textContent = info.displayVersion || `${info.edition || "2026"} edition · ${info.version}`;
      })
      .catch(() => {
        /* keep fallback */
      });
  });
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
  window.addEventListener("studio-tools-changed", () => {
    applyPlayGate(page);
    if (current === "autoaudit" && !canStreamAudit()) render("audit");
  });
  return { toast: showToast };
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
  const searchDrag = root.querySelector<HTMLElement>("#search-wrap");
  searchDrag?.addEventListener("dblclick", () => {
    if (!searchDrag.classList.contains("hidden")) return;
    void win.toggleMaximize().then(() => paintMax());
  });
  void win.onResized(() => {
    void paintMax();
  });
  void paintMax();
}

async function mountSources(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  applyPlayGate(page);
  const tabs = page.querySelector("#source-tabs")!;
  const groupsEl = page.querySelector<HTMLElement>("#source-groups")!;
  const channelsEl = page.querySelector<HTMLElement>("#source-channels")!;
  const empty = page.querySelector<HTMLElement>("#source-empty")!;
  const workspace = page.querySelector<HTMLElement>("#source-workspace")!;
  const srcDlg = page.querySelector<HTMLElement>("#src-dlg")!;
  const split = page.querySelector<HTMLElement>("#source-split")!;
  const splitHandle = page.querySelector<HTMLElement>("#source-split-handle")!;
  bindColResize({
    grid: split,
    handle: splitHandle,
    cssVar: "--src-groups-w",
    storageKey: "studio-src-groups-w",
    min: 140,
    max: 560,
    measure: (x, rect) => x - rect.left,
  });

  let sources: Source[] = [];
  let activeId = "";
  let searching = false;
  let hasManaged = false;
  let lastChans: Channel[] = [];
  let chanVirt: VirtualList<Channel> | null = null;
  let groupVirt: VirtualList<Group> | null = null;
  let activeGroup = "";
  onProbeExpired = () => chanVirt?.setItems(lastChans);
  ensureProbeExpireTimer();
  probeTtlMs = PROBE_RESULT_MS;

  const applySourceProgress = (p: SourceProgress) => {
    if (!page.querySelector("#source-tabs")) return;
    if (!p.done) return;
    const isRefresh = p.op === "refresh";
    if (p.error) {
      if (!isRefresh) {
        sources = sources.filter((s) => s.id !== p.id);
        if (activeId === p.id) activeId = sources[0]?.id ?? "";
        const has = sources.length > 0;
        empty.style.display = has ? "none" : "block";
        workspace.style.display = has ? "grid" : "none";
        paintTabs();
        if (has) void loadGroups();
      }
      toast((isRefresh ? "Refresh failed: " : "Add source failed: ") + p.error);
      return;
    }
    const src: Source = {
      id: p.id,
      name: p.name,
      kind: sources.find((s) => s.id === p.id)?.kind ?? "url",
      location: sources.find((s) => s.id === p.id)?.location ?? "",
      channelCount: p.channelCount,
      expiresAt: p.expiresAt,
    };
    const i = sources.findIndex((s) => s.id === p.id);
    if (i >= 0) sources[i] = { ...sources[i], ...src };
    else sources.push(src);
    empty.style.display = "none";
    workspace.style.display = "grid";
    const stayOnCurrent = Boolean(activeId && activeId !== p.id && sources.some((s) => s.id === activeId));
    if (!stayOnCurrent) activeId = p.id;
    paintTabs();
    if (activeId === p.id) void loadGroups();
    void hydrateXtreamExpiry();
    toast(
      `${isRefresh ? "Refreshed" : "Loaded"} ${p.channelCount.toLocaleString()} channels from ${p.name}`,
    );
  };

  onSourceProgress = applySourceProgress;
  if (!sourceProgressHooked) {
    sourceProgressHooked = true;
    void listen<SourceProgress>("source-progress", (ev) => handleSourceProgress(ev.payload));
  }

  const tabIco = (glyph: string, title: string, cls: string, onClick: () => void) => {
    const i = document.createElement("button");
    i.type = "button";
    i.className = "tab-ico " + cls;
    i.title = title;
    i.textContent = glyph;
    i.addEventListener("click", (ev) => {
      ev.stopPropagation();
      onClick();
    });
    return i;
  };

  const refreshOne = async (id: string, quiet = false) => {
    if (pendingSourceIds.has(id) || refreshingIds.has(id)) {
      if (!quiet) toast("Already loading this source.");
      return;
    }
    try {
      refreshingIds.add(id);
      if (!quiet) toast("Source will update when parsing is complete.");
      await api.refreshSource(id);
    } catch (e) {
      refreshingIds.delete(id);
      toast(String(e));
    }
  };

  let pendingRemove: Source | null = null;
  const removeOne = async (s: Source) => {
    pendingRemove = s;
    const msg = page.querySelector("#src-del-msg");
    if (msg) msg.textContent = `Remove source “${s.name}” and its channels?`;
    page.querySelector("#src-del-dlg")?.classList.add("open");
  };
  const finishRemoveOne = async (s: Source) => {
    const removingId = s.id;
    sources = sources.filter((x) => x.id !== removingId);
    if (activeId === removingId) activeId = sources[0]?.id ?? "";
    const has = sources.length > 0;
    empty.style.display = has ? "none" : "block";
    workspace.style.display = has ? "grid" : "none";
    paintTabs();
    if (has) void loadGroups();
    toast("Removing source…");
    try {
      await api.removeSource(removingId);
      toast("Source removed.");
    } catch (e) {
      toast(String(e));
      await reload();
    }
  };

  const paintTabs = () => {
    tabs.innerHTML = "";
    for (const s of sources) {
      const b = document.createElement("div");
      b.className = "tab" + (s.id === activeId ? " active" : "");
      b.dataset.sid = s.id;
      b.title = sourceTabTitle(s);
      b.setAttribute("role", "tab");
      b.tabIndex = 0;
      const label = document.createElement("span");
      label.className = "tab-label";
      label.textContent = `${s.name} (${s.channelCount})`;
      b.appendChild(label);
      const actions = document.createElement("span");
      actions.className = "tab-actions";
      actions.appendChild(
        tabIco("\uE72C", "Refresh this source", "tab-refresh", () => void refreshOne(s.id)),
      );
      actions.appendChild(tabIco("\uE70F", "Edit source", "tab-edit", () => openEditSource(s)));
      actions.appendChild(tabIco("\uE74D", "Remove source", "tab-del", () => void removeOne(s)));
      b.appendChild(actions);
      const selectTab = () => {
        activeId = s.id;
        paintTabs();
        void loadGroups();
      };
      b.addEventListener("click", selectTab);
      b.addEventListener("keydown", (ev) => {
        if (ev.key === "Enter" || ev.key === " ") {
          ev.preventDefault();
          selectTab();
        }
      });
      tabs.appendChild(b);
    }
    const add = document.createElement("button");
    add.className = "tab add";
    add.textContent = "+";
    add.title = "Add source…";
    add.addEventListener("click", () => openAddSource());
    tabs.appendChild(add);
  };

  const loadGroups = async () => {
    if (!activeId) return;
    const groups = await api.listGroups(activeId);
    if (!page.querySelector("#source-groups")) return;
    if (!activeGroup || !groups.some((g) => g.title === activeGroup)) {
      activeGroup = groups[0]?.title ?? "";
    }
    groupVirt?.destroy();
    groupsEl.innerHTML = "";
    groupVirt = bindVirtualList({
      scroller: groupsEl,
      rowHeight: 36,
      renderRow: (g) => {
        const row = document.createElement("button");
        row.className = "group-row" + (g.title === activeGroup ? " active" : "");
        row.title = `${decodeEntities(g.title)}  (${g.count})`;
        row.textContent = `${decodeEntities(g.title)}  (${g.count})`;
        row.addEventListener("click", async () => {
          activeGroup = g.title;
          groupVirt?.setItems(groups);
          const chans = await api.listChannels(activeId, g.title);
          paintChannels(chans, false, g.count);
        });
        return row;
      },
    });
    groupVirt.setItems(groups);
    if (activeGroup) {
      const gCount = groups.find((g) => g.title === activeGroup)?.count;
      paintChannels(await api.listChannels(activeId, activeGroup), false, gCount);
    }
  };

  const paintChannels = (chans: Channel[], isSearch: boolean, groupCount?: number) => {
    searching = isSearch;
    lastChans = chans;
    const addCol = hasManaged ? "" : " no-add";
    chanVirt?.destroy();
    channelsEl.innerHTML = "";
    const head = document.createElement("div");
    head.className = "chan-head" + addCol;
    const truncated =
      !isSearch && groupCount != null && groupCount > chans.length
        ? `<span class="chan-sub" style="grid-column:1/-1">Showing ${chans.length.toLocaleString()} of ${groupCount.toLocaleString()}</span>`
        : "";
    channelsEl.classList.toggle("hide-url", localStorage.getItem("studio-hide-url-col") === "1");
    head.innerHTML = `<span class="col-icon">Audit</span><span class="col-icon">Play</span>${hasManaged ? "<span class=\"col-icon\">Add</span>" : ""}<span>Name <button type="button" class="col-url-show" title="Show URL column">URL</button></span><button type="button" class="col-url" title="Hide URL column">URL</button>${truncated}`;
    const srcName = (id: string) => sources.find((s) => s.id === id)?.name ?? "";
    chanVirt = bindVirtualList({
      scroller: channelsEl,
      rowHeight: 48,
      header: head,
      renderRow: (c) => {
        const row = document.createElement("div");
        row.className = "chan-row" + addCol;
        const sub = isSearch
          ? `${srcName(c.sourceId)} · ${decodeEntities(c.groupTitle)}`
          : decodeEntities(c.groupTitle);
        row.innerHTML = `
        <button class="probe" data-id="${escapeAttr(c.id)}" data-url="${escapeAttr(c.url)}" data-sid="${escapeAttr(c.sourceId)}" title="Audit this stream">&#xE9E9;</button>
        <button class="play" data-id="${escapeAttr(c.id)}" data-url="${escapeAttr(c.url)}" data-sid="${escapeAttr(c.sourceId)}" title="${canPlay() ? "Play stream" : "Install mpv or VLC, or set a player path in Settings"}" ${canPlay() ? "" : "disabled"}>&#xE768;</button>
        ${hasManaged ? `<button class="add-pl" data-id="${escapeAttr(c.id)}" title="Add to managed playlist">&#xE710;</button>` : ""}
        <div class="chan-main">
          <div class="chan-meta">
            <div class="chan-name" title="${escapeAttr(decodeEntities(c.name))}">${escapeHtml(decodeEntities(c.name))}</div>
            <div class="chan-sub">${escapeHtml(sub)}</div>
          </div>
          ${auditCellHtml(probeById.get(c.id))}
        </div>
        <span class="copy url" data-copy="${escapeAttr(c.url)}" title="${escapeAttr(c.url)}">${escapeHtml(truncate(c.url, 64))}</span>
      `;
        return row;
      },
    });
    chanVirt.setItems(chans);
    applyPlayGate(page);
  };

  const reload = async () => {
    sources = (await api.listSources()).filter((s) => !pendingSourceIds.has(s.id));
    hasManaged = (await api.managedCount()) > 0;
    if (!page.querySelector("#source-tabs")) return;
    const has = sources.length > 0;
    empty.style.display = has ? "none" : "block";
    workspace.style.display = has ? "grid" : "none";
    if (!has) {
      activeId = "";
      return;
    }
    if (!sources.some((s) => s.id === activeId)) activeId = sources[0].id;
    paintTabs();
    void hydrateXtreamExpiry();
    await loadGroups();
  };

  const refreshAll = async () => {
    const ids = sources
      .map((s) => s.id)
      .filter((id) => !pendingSourceIds.has(id) && !refreshingIds.has(id));
    if (ids.length === 0) {
      toast(sources.length === 0 ? "No sources to refresh." : "Already loading.");
      return;
    }
    toast(ids.length === 1 ? "Source will update when parsing is complete." : `Refreshing ${ids.length} sources…`);
    for (const id of ids) {
      await refreshOne(id, true);
    }
  };

  const removeAll = async () => {
    if (sources.length === 0) {
      toast("No sources to remove.");
      return;
    }
    const n = sources.length;
    const msg = page.querySelector("#src-del-all-msg");
    if (msg) msg.textContent = `Remove all ${n} sources and their channels?`;
    page.querySelector("#src-del-all-dlg")?.classList.add("open");
  };
  const finishRemoveAll = async () => {
    const n = sources.length;
    const ids = sources.map((s) => s.id);
    sources = [];
    activeId = "";
    empty.style.display = "block";
    workspace.style.display = "none";
    toast("Removing sources…");
    try {
      for (const id of ids) {
        await api.removeSource(id);
      }
      toast(n === 1 ? "Source removed." : "All sources removed.");
    } catch (e) {
      toast(String(e));
      await reload();
    }
  };

  const hydrateXtreamExpiry = async () => {
    const missing = sources.filter((s) => s.kind === "xtream" && (s.expiresAt == null || s.expiresAt === 0));
    if (missing.length === 0) return;
    await Promise.all(
      missing.map(async (s) => {
        try {
          s.expiresAt = await api.probeXtreamExpiry(s.id);
        } catch {
          /* panel may block player_api */
        }
      }),
    );
    if (!page.querySelector("#source-tabs")) return;
    paintTabs();
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

  let editingId: string | null = null;
  let editingOriginal: { location: string; headers: string; kind: string } | null = null;

  const val = (id: string) => (page.querySelector(`#${id}`) as HTMLInputElement | HTMLTextAreaElement).value;
  const setVal = (id: string, v: string) => {
    (page.querySelector(`#${id}`) as HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement).value = v;
  };

  const applySourceMode = () => {
    const mode = (page.querySelector("#src-mode") as HTMLSelectElement).value;
    page.querySelector<HTMLElement>("#src-file-panel")!.hidden = mode !== "file";
    page.querySelector<HTMLElement>("#src-url-panel")!.hidden = mode !== "url";
    page.querySelector<HTMLElement>("#src-xtream-panel")!.hidden = mode !== "xtream";
    page.querySelector<HTMLElement>("#src-http-headers")!.hidden = mode === "file";
  };

  const buildExtraHeaders = (ua: string, auth: string, cookie: string, extra: string) => {
    const headers: Record<string, string> = {};
    if (ua) headers["User-Agent"] = ua;
    if (auth) headers["Authorization"] = auth;
    if (cookie) headers["Cookie"] = cookie;
    for (const line of extra.split(/\r?\n/)) {
      const idx = line.indexOf(":");
      if (idx <= 0) continue;
      const key = line.slice(0, idx).trim();
      const val = line.slice(idx + 1).trim();
      if (key) headers[key] = val;
    }
    return headers;
  };

  const currentHeaders = () =>
    buildExtraHeaders(val("src-ua").trim(), val("src-auth").trim(), val("src-cookie").trim(), val("src-extra"));

  const parseXtreamLocation = (location: string) => {
    try {
      const u = new URL(location);
      return {
        server: `${u.protocol}//${u.host}`,
        username: u.searchParams.get("username") ?? "",
        password: u.searchParams.get("password") ?? "",
        output: u.searchParams.get("output") || "ts",
      };
    } catch {
      return { server: location, username: "", password: "", output: "ts" };
    }
  };

  const fillHeaderFields = (raw?: string) => {
    let h: Record<string, string> = {};
    try {
      h = JSON.parse(raw || "{}") as Record<string, string>;
    } catch {
      h = {};
    }
    const extra: string[] = [];
    setVal("src-ua", "");
    setVal("src-auth", "");
    setVal("src-cookie", "");
    setVal("src-extra", "");
    for (const [k, v] of Object.entries(h)) {
      if (k.toLowerCase() === "user-agent") setVal("src-ua", v);
      else if (k.toLowerCase() === "authorization") setVal("src-auth", v);
      else if (k.toLowerCase() === "cookie") setVal("src-cookie", v);
      else extra.push(`${k}: ${v}`);
    }
    setVal("src-extra", extra.join("\n"));
  };

  const resetSourceForm = () => {
    editingId = null;
    editingOriginal = null;
    page.querySelector("#src-dlg-title")!.textContent = "Add playlist source";
    page.querySelector("#src-load")!.textContent = "Load";
    (page.querySelector("#src-mode") as HTMLSelectElement).disabled = false;
    setVal("src-mode", "file");
    setVal("src-file", "");
    setVal("src-url", "");
    setVal("src-name", "");
    setVal("src-xtream-server", "");
    setVal("src-xtream-user", "");
    setVal("src-xtream-pass", "");
    setVal("src-xtream-output", "ts");
    fillHeaderFields("{}");
    applySourceMode();
  };

  const openAddSource = () => {
    resetSourceForm();
    srcDlg.classList.add("open");
  };

  const openEditSource = (s: Source) => {
    resetSourceForm();
    editingId = s.id;
    editingOriginal = {
      location: s.location,
      headers: s.headersJson ?? "{}",
      kind: s.kind,
    };
    page.querySelector("#src-dlg-title")!.textContent = "Edit source";
    page.querySelector("#src-load")!.textContent = "Save";
    const kind = s.kind === "xtream" ? "xtream" : s.kind === "url" ? "url" : "file";
    setVal("src-mode", kind);
    setVal("src-name", s.name);
    if (kind === "file") setVal("src-file", s.location);
    if (kind === "url") setVal("src-url", s.location);
    if (kind === "xtream") {
      const x = parseXtreamLocation(s.location);
      setVal("src-xtream-server", x.server);
      setVal("src-xtream-user", x.username);
      setVal("src-xtream-pass", x.password);
      setVal("src-xtream-output", x.output === "m3u8" ? "m3u8" : "ts");
    }
    fillHeaderFields(s.headersJson);
    applySourceMode();
    srcDlg.classList.add("open");
  };

  page.querySelector("#empty-add")?.addEventListener("click", openAddSource);
  page.querySelector("#src-mode")?.addEventListener("change", applySourceMode);
  page.querySelector("#src-cancel")?.addEventListener("click", () => srcDlg.classList.remove("open"));
  page.querySelector("#src-browse")?.addEventListener("click", async () => {
    try {
      const path = await invoke<string | null>("pick_playlist_path");
      if (!path) return;
      (page.querySelector("#src-file") as HTMLInputElement).value = path;
      const name = page.querySelector<HTMLInputElement>("#src-name")!;
      if (!name.value.trim()) {
        const base = path.replace(/^.*[\\/]/, "").replace(/\.[^.]+$/, "");
        if (base) name.value = base;
      }
    } catch (e) {
      const label = page.querySelector("#src-file-label")!;
      label.textContent = `Browse failed — paste full path`;
      toast(String(e));
    }
  });
  page.querySelector("#src-load")?.addEventListener("click", async () => {
    const mode = (page.querySelector("#src-mode") as HTMLSelectElement).value;
    const display = val("src-name").trim() || undefined;
    const loadBtn = page.querySelector<HTMLButtonElement>("#src-load")!;
    loadBtn.disabled = true;
    const prev = loadBtn.textContent;
    loadBtn.textContent = editingId ? "Saving…" : "Loading…";
    try {
      let src: Source;
      if (editingId) {
        let location = "";
        let kind = mode;
        if (mode === "file") {
          location = val("src-file").trim().replace(/^"|"$/g, "");
          if (!location) {
            page.querySelector("#src-file-label")!.textContent = "File path (required — browse or paste a valid path)";
            return;
          }
          kind = "file";
        } else if (mode === "xtream") {
          const server = val("src-xtream-server").trim();
          const username = val("src-xtream-user").trim();
          const password = val("src-xtream-pass").trim();
          if (!server || !username || !password) {
            toast("Xtream server, username, and password are required.");
            return;
          }
          location = xtreamPlaylistUrl(
            server,
            username,
            password,
            val("src-xtream-output").trim() || "ts",
          );
          kind = "xtream";
        } else {
          location = val("src-url").trim();
          if (!location || !/^https?:\/\//i.test(location)) {
            page.querySelector("#src-url-label")!.textContent = "Playlist URL (required — valid http/https)";
            return;
          }
          kind = "url";
        }
        const headers = mode === "file" ? {} : currentHeaders();
        const headersJson = JSON.stringify(headers);
        const refetch =
          location !== (editingOriginal?.location ?? "") ||
          headersJson !== (editingOriginal?.headers ?? "{}") ||
          kind !== (editingOriginal?.kind ?? "");
        src = await api.updateSource({
          id: editingId,
          name: display ?? "Source",
          kind,
          location,
          headers,
          refetch,
        });
        srcDlg.classList.remove("open");
        if (refetch) {
          refreshingIds.add(src.id);
          toast("Source will update when parsing is complete.");
        } else {
          toast(`Saved ${src.name} (${src.channelCount.toLocaleString()} channels)`);
        }
        await reload();
      } else if (mode === "file") {
        const path = val("src-file").trim().replace(/^"|"$/g, "");
        if (!path) {
          page.querySelector("#src-file-label")!.textContent = "File path (required — browse or paste a valid path)";
          return;
        }
        srcDlg.classList.remove("open");
        toast("Source button will load when parsing is complete.");
        src = await invoke<Source>("add_source_file", { args: { path, name: display } });
        pendingSourceIds.add(src.id);
      } else if (mode === "xtream") {
        const server = val("src-xtream-server").trim();
        const username = val("src-xtream-user").trim();
        const password = val("src-xtream-pass").trim();
        if (!server || !username || !password) {
          toast("Xtream server, username, and password are required.");
          return;
        }
        srcDlg.classList.remove("open");
        toast("Source button will load when parsing is complete.");
        src = await api.addSourceXtream({
          server,
          username,
          password,
          output: val("src-xtream-output").trim() || "ts",
          name: display,
          headers: currentHeaders(),
        });
        pendingSourceIds.add(src.id);
      } else {
        const url = val("src-url").trim();
        if (!url || !/^https?:\/\//i.test(url)) {
          page.querySelector("#src-url-label")!.textContent = "Playlist URL (required — valid http/https)";
          return;
        }
        srcDlg.classList.remove("open");
        toast("Source button will load when parsing is complete.");
        src = await api.addSourceUrl(url, display, currentHeaders());
        pendingSourceIds.add(src.id);
      }
    } catch (e) {
      toast((editingId ? "Save source failed: " : "Add source failed: ") + String(e));
    } finally {
      loadBtn.disabled = false;
      loadBtn.textContent = prev || (editingId ? "Save" : "Load");
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

  void invoke<Record<string, unknown>>("load_settings").then((st) => {
    applyPlayerEngineValue(
      page.querySelector<HTMLSelectElement>("#src-player"),
      st.DefaultPlayer ?? 2,
    );
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
    if (t.classList.contains("col-url") || t.classList.contains("col-url-show")) {
      const hide = localStorage.getItem("studio-hide-url-col") !== "1";
      localStorage.setItem("studio-hide-url-col", hide ? "1" : "0");
      channelsEl.classList.toggle("hide-url", hide);
      return;
    }
    if (t.classList.contains("probe")) {
      const found = t.dataset.id ? lastChans.find((c) => c.id === t.dataset.id) : undefined;
      const id = found?.id ?? t.dataset.id ?? "";
      if (!id || probeById.get(id)?.state === "run") return;
      if (auditInFlight >= AUDIT_MAX) {
        toast("Two audits already running.");
        return;
      }
      const url = decodeEntities(found?.url ?? t.dataset.url ?? "");
      const sid = found?.sourceId ?? t.dataset.sid;
      probeById.set(id, { state: "run" });
      auditInFlight += 1;
      chanVirt?.setItems(lastChans);
      try {
        const result = await api.auditSourceChannel(url, sid);
        probeById.set(id, { state: "done", result, until: Date.now() + probeTtlMs });
        saveProbeStore();
      } catch (e) {
        probeById.set(id, {
          state: "done",
          result: { ok: false, grade: "F", error: String(e) },
          until: Date.now() + probeTtlMs,
        });
        saveProbeStore();
        toast(String(e));
      } finally {
        auditInFlight = Math.max(0, auditInFlight - 1);
      }
      chanVirt?.setItems(lastChans);
    }
    if (t.classList.contains("play")) {
      if (!canPlay()) {
        toast("Install mpv or VLC, or set a player path in Settings");
        return;
      }
      try {
        const found = t.dataset.id ? lastChans.find((c) => c.id === t.dataset.id) : undefined;
        const url = decodeEntities(found?.url ?? t.dataset.url ?? "");
        const sid = found?.sourceId ?? t.dataset.sid;
        toast("Checking stream…");
        await api.playUrl(url, sid);
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

  page.querySelector("#src-refresh-all")?.addEventListener("click", () => void refreshAll());
  page.querySelector("#src-delete-all")?.addEventListener("click", () => void removeAll());
  page.querySelector("#src-del-no")?.addEventListener("click", () => {
    page.querySelector("#src-del-dlg")?.classList.remove("open");
    pendingRemove = null;
  });
  page.querySelector("#src-del-yes")?.addEventListener("click", () => {
    page.querySelector("#src-del-dlg")?.classList.remove("open");
    const s = pendingRemove;
    pendingRemove = null;
    if (s) void finishRemoveOne(s);
  });
  page.querySelector("#src-del-all-no")?.addEventListener("click", () => {
    page.querySelector("#src-del-all-dlg")?.classList.remove("open");
  });
  page.querySelector("#src-del-all-yes")?.addEventListener("click", () => {
    page.querySelector("#src-del-all-dlg")?.classList.remove("open");
    void finishRemoveAll();
  });

  try {
    await reload();
  } catch (e) {
    toast(String(e));
  }
}

function decodeEntities(s: string): string {
  return s
    .replace(/&amp;/gi, "&")
    .replace(/&lt;/gi, "<")
    .replace(/&gt;/gi, ">")
    .replace(/&quot;/gi, '"')
    .replace(/&#39;/g, "'")
    .replace(/&apos;/gi, "'")
    .replace(/&nbsp;/gi, " ");
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

function auditDetail(r: ChannelAudit): string {
  if (!r.ok) return r.error?.trim() || "Failed";
  const parts: string[] = [];
  if (r.width && r.height) parts.push(`${r.width}×${r.height}`);
  if (r.aspectRatio) parts.push(r.aspectRatio);
  if (r.fps && r.fps > 0) parts.push(`${Math.round(r.fps)} fps`);
  if (r.videoCodec) parts.push(r.videoCodec);
  if (r.audioCodec) parts.push(r.audioCodec);
  if (r.latencyMs != null) parts.push(`${r.latencyMs} ms`);
  return parts.join(" · ") || "OK";
}

function auditCellHtml(probe?: ProbeEntry): string {
  if (!probe) return `<div class="chan-audit"></div>`;
  if (probe.state === "run") {
    return `<div class="chan-audit"><span class="audit-detail">Testing…</span><span class="audit-grade wait">…</span></div>`;
  }
  const r = probe.result;
  const g = (r.grade || (r.ok ? "C" : "F")).toUpperCase();
  const detail = auditDetail(r);
  return `<div class="chan-audit" title="${escapeAttr(detail)}"><span class="audit-detail">${escapeHtml(detail)}</span><span class="audit-grade" data-g="${escapeAttr(g)}">${escapeHtml(g)}</span></div>`;
}

function xtreamExpiryLabel(exp: number | null | undefined, now = Date.now() / 1000): string | null {
  if (exp == null || exp <= 0) return null;
  const secs = exp - now;
  if (secs <= 0) {
    const days = Math.floor(-secs / 86400);
    return days <= 0 ? "expired" : `expired ${days}d ago`;
  }
  const days = Math.floor(secs / 86400);
  if (days < 1) {
    const hours = Math.max(1, Math.ceil(secs / 3600));
    return `expiry in ${hours}h`;
  }
  return `expiry in ${days}d`;
}

function sourceTabTitle(s: Source): string {
  if (s.kind !== "xtream") return s.name;
  const exp = xtreamExpiryLabel(s.expiresAt);
  return exp ? `${s.name}\n${exp}` : s.name;
}

function xtreamPlaylistUrl(server: string, username: string, password: string, output: string): string {
  const raw = server.trim();
  const withScheme = /:\/\//.test(raw) ? raw : `http://${raw}`;
  const noQuery = withScheme.split(/[?#]/)[0].replace(/\/+$/, "");
  const stripped = noQuery.replace(/\/(get\.php|player_api\.php|panel_api\.php|xmltv\.php)$/i, "");
  let host = stripped;
  try {
    const u = new URL(stripped);
    host = `${u.protocol}//${u.host}`;
  } catch {
    host = stripped;
  }
  const out = output.trim() || "ts";
  return `${host}/get.php?username=${encodeURIComponent(username.trim())}&password=${encodeURIComponent(password)}&type=m3u_plus&output=${encodeURIComponent(out)}`;
}

function pageHtml(id: NavId): string {
  switch (id) {
    case "audit":
      return `
        <h1 class="page-title">Add Sources</h1>
        <p class="page-sub">Load files, set URL playlists, or setup Xtream Codes API by adding a source below.</p>
        <div class="source-bar">
          <fieldset class="player-field">
            <legend>Video Player</legend>
            <select id="src-player" title="Player for Play button">
              ${playerEngineOptionsHtml()}
            </select>
          </fieldset>
        </div>
        <div class="empty" id="source-empty">
          <div class="glyph">☰</div>
          <p>Add a playlist source to get started</p>
          <button class="accent" id="empty-add">Add source…</button>
        </div>
        <div class="source-workspace" id="source-workspace">
          <div class="tabs-row">
            <div class="tabs" id="source-tabs"></div>
            <div class="source-row-actions">
              <button type="button" class="tab-ico" id="src-refresh-all" title="Refresh all sources">&#xE72C;</button>
              <button type="button" class="tab-ico tab-del" id="src-delete-all" title="Remove all sources">&#xE74D;</button>
            </div>
          </div>
          <div class="source-split" id="source-split">
            <div class="groups">
              <div class="groups-head">Groups</div>
              <div class="groups-body" id="source-groups"></div>
            </div>
            <div class="source-split-handle" id="source-split-handle" title="Drag to resize groups"></div>
            <div class="channels" id="source-channels"></div>
          </div>
        </div>
        <div class="dialog-backdrop" id="src-dlg">
          <div class="dialog" style="width:480px;max-height:80vh;overflow:auto">
            <h2 id="src-dlg-title">Add playlist source</h2>
            <div class="field"><label>Source type</label>
              <select id="src-mode">
                <option value="file">Local file (.m3u / .m3u8)</option>
                <option value="url">HTTP(S) URL</option>
                <option value="xtream">Xtream Codes API</option>
              </select></div>
            <div id="src-file-panel">
              <div class="field"><label id="src-file-label">File path</label>
                <input id="src-file" placeholder="Full path or use Browse…" /></div>
              <button id="src-browse">Browse…</button>
            </div>
            <div id="src-url-panel" hidden>
              <div class="field"><label id="src-url-label">Playlist URL</label>
                <input id="src-url" placeholder="https://…" /></div>
            </div>
            <div id="src-xtream-panel" hidden>
              <div class="field"><label>Server URL</label>
                <input id="src-xtream-server" placeholder="http://host:port" /></div>
              <div class="field"><label>Username</label>
                <input id="src-xtream-user" autocomplete="off" /></div>
              <div class="field"><label>Password</label>
                <input id="src-xtream-pass" type="password" autocomplete="off" /></div>
              <div class="field"><label>Stream output</label>
                <select id="src-xtream-output">
                  <option value="ts">MPEG-TS</option>
                  <option value="m3u8">HLS (m3u8)</option>
                </select></div>
            </div>
            <div id="src-http-headers" hidden>
              <div class="field"><label>User-Agent (optional)</label>
                <input id="src-ua" placeholder="Leave empty — Play/Audit use VLC" /></div>
              <div class="field"><label>Authorization (optional)</label>
                <input id="src-auth" placeholder="Bearer …" /></div>
              <div class="field"><label>Cookie (optional)</label>
                <input id="src-cookie" /></div>
              <div class="field"><label>Extra headers (one Key: Value per line)</label>
                <textarea id="src-extra" rows="3"></textarea></div>
            </div>
            <div class="field"><label>Display name (optional)</label>
              <input id="src-name" placeholder="Defaults to file name or host" /></div>
            <div class="dialog-actions">
              <button id="src-cancel">Cancel</button>
              <button class="accent" id="src-load">Load</button>
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
        </div>
        <div class="dialog-backdrop" id="src-del-dlg">
          <div class="dialog">
            <h2>Remove source?</h2>
            <p class="page-sub" id="src-del-msg"></p>
            <div class="dialog-actions">
              <button type="button" id="src-del-no">Cancel</button>
              <button type="button" class="accent" id="src-del-yes">Remove</button>
            </div>
          </div>
        </div>
        <div class="dialog-backdrop" id="src-del-all-dlg">
          <div class="dialog">
            <h2>Remove all sources?</h2>
            <p class="page-sub" id="src-del-all-msg">Remove all sources and their channels?</p>
            <div class="dialog-actions">
              <button type="button" id="src-del-all-no">Cancel</button>
              <button type="button" class="accent" id="src-del-all-yes">Remove</button>
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
    case "updates":
      return updatesHtml();
    case "settings":
      return settingsHtml();
  }
}
