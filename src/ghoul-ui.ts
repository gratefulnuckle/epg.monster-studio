import { invoke } from "@tauri-apps/api/core";
import {
  hhmm,
  tvgKeys,
  type Category,
  type Channel,
  type Prefs,
  type Snapshot,
} from "./ghoul-types";

const WINDOW_BACK = 10 * 60;
const WINDOW_FWD = 16 * 3600;
const PX_PER_HOUR = 240;
const NAME_COL = 220;
const GHOUL_UI_KEY = "studio-ghoul-ui";

type GhoulUiState = {
  expanded: string[];
  filter: string;
  hideGrid: boolean;
  treeScroll: number;
  guideScroll: number;
};

function loadGhoulUi(): GhoulUiState | null {
  try {
    const raw = sessionStorage.getItem(GHOUL_UI_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as GhoulUiState;
    if (!parsed || !Array.isArray(parsed.expanded)) return null;
    return parsed;
  } catch {
    return null;
  }
}

function saveGhoulUi(state: GhoulUiState): void {
  try {
    sessionStorage.setItem(GHOUL_UI_KEY, JSON.stringify(state));
  } catch {
    /* quota */
  }
}

type Host = {
  channels: Channel[];
  categories: Category[];
  prefs: Prefs;
  xmlOk: boolean;
  xmlMessage?: string | null;
};

export function mountGhoulUi(root: HTMLElement, host: Host, toast: (s: string) => void): () => void {
  let filter = "";
  let index = Math.min(host.prefs.lastChannel, Math.max(0, host.channels.length - 1));
  let hideGrid = false;
  let snap: Snapshot = { loading: true, progs: {}, now: {}, next: {} };
  let searchOpen = false;
  let zapTimer = 0;


  const groupName = (ch: Channel) => ch.group.trim() || "Other";
  const groups = host.categories.filter((c) => c.title !== "All channels");
  if (groups.length === 0 && host.channels.length) {
    groups.push({ title: "Other", count: host.channels.length });
  }
  const savedUi = loadGhoulUi();
  let selectedGroup =
    savedUi?.expanded[0] ||
    (host.channels[index] ? groupName(host.channels[index]) : "") ||
    groups[0]?.title ||
    "";
  let didPinNow = false;
  if (savedUi?.filter) {
    filter = savedUi.filter;
    searchOpen = true;
  }
  if (savedUi?.hideGrid) hideGrid = true;
  let pendingTreeScroll: number | null = savedUi?.treeScroll ?? null;
  let pendingGuideScroll: number | null = savedUi?.guideScroll ?? null;

  root.classList.add("gh-root");
  root.closest(".page-pane, .page")?.classList.add("gh-live");
  root.innerHTML = `
    <div class="gh" id="gh">
      <aside class="gh-tree" id="gh-tree"></aside>
      <div class="gh-video" id="gh-video">
        <div class="gh-zap" id="gh-zap" hidden></div>
        <div class="gh-osd" id="gh-foot"></div>
      </div>
      <div class="gh-guide">
        <div class="gh-guide-scroll" id="gh-guide-scroll">
          <div class="gh-times" id="gh-times"></div>
          <div class="gh-grid" id="gh-grid"></div>
        </div>
      </div>
      <div class="gh-search" id="gh-search" hidden>
        <input id="gh-search-in" placeholder="Search channels" />
      </div>
    </div>
  `;

  const gh = root.querySelector<HTMLElement>("#gh")!;
  const treeEl = root.querySelector<HTMLElement>("#gh-tree")!;
  const timesEl = root.querySelector<HTMLElement>("#gh-times")!;
  const gridEl = root.querySelector<HTMLElement>("#gh-grid")!;
  const videoEl = root.querySelector<HTMLElement>("#gh-video")!;
  const zapEl = root.querySelector<HTMLElement>("#gh-zap")!;
  const footEl = root.querySelector<HTMLElement>("#gh-foot")!;
  const searchBox = root.querySelector<HTMLElement>("#gh-search")!;
  const searchIn = root.querySelector<HTMLInputElement>("#gh-search-in")!;

  if (searchOpen) {
    searchBox.hidden = false;
    searchIn.value = filter;
  }

  const persistUi = () => {
    saveGhoulUi({
      expanded: selectedGroup ? [selectedGroup] : [],
      filter,
      hideGrid,
      treeScroll: treeEl.scrollTop,
      guideScroll: root.querySelector<HTMLElement>("#gh-guide-scroll")?.scrollTop ?? 0,
    });
  };

  const visible = (): number[] => {
    const needle = filter.trim().toLowerCase();
    return host.channels
      .map((_, i) => i)
      .filter((i) => {
        const ch = host.channels[i];
        if (needle && !ch.name.toLowerCase().includes(needle)) return false;
        const g = groupName(ch);
        if (needle) return true;
        return g === selectedGroup;
      });
  };

  const nowFor = (ch: Channel) => {
    for (const k of tvgKeys(ch.tvgId)) {
      if (snap.now[k]) return snap.now[k];
    }
    return undefined;
  };

  const progsFor = (ch: Channel) => {
    for (const k of tvgKeys(ch.tvgId)) {
      if (snap.progs[k]) return snap.progs[k];
    }
    return [];
  };

  function renderTree() {
    treeEl.innerHTML = "";
    const needle = filter.trim().toLowerCase();
    for (const g of groups) {
      const members = host.channels.filter(
        (ch) => groupName(ch) === g.title && (!needle || ch.name.toLowerCase().includes(needle)),
      );
      if (needle && members.length === 0) continue;
      const head = document.createElement("button");
      head.type = "button";
      head.className = "gh-tree-group" + (g.title === selectedGroup ? " open" : "");
      head.innerHTML = `<span class="gh-tree-label"></span><span class="gh-tree-count">${members.length}</span>`;
      head.querySelector(".gh-tree-label")!.textContent = g.title;
      head.addEventListener("click", () => {
        selectedGroup = g.title;
        persistUi();
        render();
      });
      treeEl.appendChild(head);
    }
    if (pendingTreeScroll != null) {
      treeEl.scrollTop = pendingTreeScroll;
      pendingTreeScroll = null;
    } else {
      treeEl.querySelector(".gh-tree-group.open")?.scrollIntoView({ block: "nearest" });
    }
  }

  function renderTimes() {
    timesEl.innerHTML = "";
    const spacer = document.createElement("span");
    spacer.className = "gh-time-pad";
    timesEl.appendChild(spacer);
    const win0 = Math.floor(Date.now() / 1000) - WINDOW_BACK;
    const hours = Math.ceil((WINDOW_BACK + WINDOW_FWD) / 3600);
    timesEl.style.width = `${NAME_COL + hours * PX_PER_HOUR}px`;
    for (let i = 0; i < hours; i++) {
      const t = document.createElement("span");
      t.className = "gh-time";
      t.textContent = hhmm(win0 + i * 3600);
      t.style.width = `${PX_PER_HOUR}px`;
      timesEl.appendChild(t);
    }
  }

  function renderGrid() {
    const rows = visible();
    const now = Math.floor(Date.now() / 1000);
    const win0 = now - WINDOW_BACK;
    gridEl.innerHTML = "";
    const hours = Math.ceil((WINDOW_BACK + WINDOW_FWD) / 3600);
    gridEl.style.width = `${NAME_COL + hours * PX_PER_HOUR}px`;
    for (const i of rows) {
      const ch = host.channels[i];
      const row = document.createElement("div");
      row.className = "gh-row" + (i === index ? " playing" : "");
      const name = document.createElement("button");
      name.type = "button";
      name.className = "gh-name";
      const letter = (ch.name.trim()[0] || "?").toUpperCase();
      name.innerHTML = ch.tvgLogo
        ? `<img alt="" src="${ch.tvgLogo}" /><span></span>`
        : `<span class="gh-tile">${letter}</span><span></span>`;
      name.querySelector("span:last-child")!.textContent = ch.name;
      name.addEventListener("click", () => void zap(i));
      const track = document.createElement("div");
      track.className = "gh-track";
      const progs = progsFor(ch);
      if (!progs.length) {
        const empty = document.createElement("div");
        empty.className = "gh-block empty";
        empty.textContent = snap.loading ? "EPG loading" : "";
        empty.style.left = "0";
        empty.style.width = `${hours * PX_PER_HOUR}px`;
        track.appendChild(empty);
      } else {
        for (const p of progs) {
          const b = document.createElement("button");
          b.type = "button";
          b.className = "gh-block";
          if (p.start <= now && now < p.stop) b.classList.add("now");
          const left = ((p.start - win0) / 3600) * PX_PER_HOUR;
          const width = Math.max(8, ((p.stop - p.start) / 3600) * PX_PER_HOUR);
          b.style.left = `${left}px`;
          b.style.width = `${width}px`;
          b.textContent = p.title;
          b.title = `${p.title} ${hhmm(p.start)}–${hhmm(p.stop)}`;
          b.addEventListener("click", () => void zap(i));
          track.appendChild(b);
        }
      }
      const line = document.createElement("div");
      line.className = "gh-nowline";
      line.style.left = `${((now - win0) / 3600) * PX_PER_HOUR}px`;
      track.appendChild(line);
      row.appendChild(name);
      row.appendChild(track);
      gridEl.appendChild(row);
    }
    const guideScroll = root.querySelector<HTMLElement>("#gh-guide-scroll");
    if (pendingGuideScroll != null && guideScroll) {
      guideScroll.scrollTop = pendingGuideScroll;
      pendingGuideScroll = null;
    } else {
      gridEl.querySelector(".gh-row.playing")?.scrollIntoView({ block: "nearest" });
    }
    if (!didPinNow && guideScroll) {
      const nowX = ((now - win0) / 3600) * PX_PER_HOUR;
      guideScroll.scrollLeft = Math.max(0, Math.round(nowX));
      didPinNow = true;
    }
  }

  function nextFor(ch: Channel) {
    for (const k of tvgKeys(ch.tvgId)) {
      if (snap.next[k]) return snap.next[k];
    }
    return undefined;
  }

  function renderFoot() {
    const ch = host.channels[index];
    if (!ch) {
      footEl.innerHTML = "";
      return;
    }
    const n = nowFor(ch);
    const nx = nextFor(ch);
    let status = "";
    if (snap.err) status = "EPG failed";
    else if (snap.loading) status = "EPG loading";
    else if (host.xmlMessage && !host.xmlOk) status = host.xmlMessage;
    const nowLine = n
      ? `${escapeHtml(ch.name)} · ${escapeHtml(n.title)} ${hhmm(n.startUnix)}–${hhmm(n.stopUnix)}`
      : escapeHtml(ch.name);
    const nextLine = nx ? ` · Next ${escapeHtml(nx.title)}` : "";
    const st = status ? ` · ${escapeHtml(status)}` : "";
    footEl.textContent = nowLine + nextLine + st;
  }

  function escapeHtml(s: string): string {
    return s
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function render() {
    gh.classList.toggle("hide-grid", hideGrid);
    renderTree();
    renderTimes();
    renderGrid();
    renderFoot();
    syncRect();
  }

  function zapDelta(delta: number) {
    const rows = visible();
    if (!rows.length) {
      void zap(index + delta);
      return;
    }
    let pos = rows.indexOf(index);
    if (pos < 0) pos = 0;
    void zap(rows[(pos + delta + rows.length) % rows.length]);
  }

  async function zap(i: number) {
    if (!host.channels.length) return;
    const len = host.channels.length;
    index = ((i % len) + len) % len;
    const ch = host.channels[index];
    selectedGroup = groupName(ch);
    const n = nowFor(ch);
    zapEl.hidden = false;
    zapEl.textContent = `${index + 1}  ${ch.name}  ${ch.group}${n ? "  " + n.title : ""}`;
    window.clearTimeout(zapTimer);
    zapTimer = window.setTimeout(() => {
      zapEl.hidden = true;
    }, 4000);
    try {
      await invoke("ghoul_play", { index });
    } catch (e) {
      toast(String(e));
    }
    render();
  }

  function syncRect() {
    const r = videoEl.getBoundingClientRect();
    void invoke("ghoul_set_rect", {
      x: Math.round(r.left),
      y: Math.round(r.top),
      w: Math.round(r.width),
      h: Math.round(r.height),
    }).catch(() => {
      /* pane may be unmounted */
    });
  }

  const ro = new ResizeObserver(() => syncRect());
  ro.observe(videoEl);
  window.addEventListener("resize", syncRect);

  gridEl.addEventListener(
    "wheel",
    (ev) => {
      if (hideGrid) {
        ev.preventDefault();
        zapDelta(ev.deltaY > 0 ? 1 : -1);
      }
    },
    { passive: false },
  );

  const onKey = (ev: KeyboardEvent) => {
    if (searchOpen && ev.key !== "Escape" && ev.key !== "s" && ev.key !== "S") return;
    switch (ev.key) {
      case "ArrowUp":
        ev.preventDefault();
        zapDelta(-1);
        break;
      case "ArrowDown":
        ev.preventDefault();
        zapDelta(1);
        break;
      case "PageUp":
        ev.preventDefault();
        zapDelta(-10);
        break;
      case "PageDown":
        ev.preventDefault();
        zapDelta(10);
        break;
      case "g":
      case "G":
        hideGrid = !hideGrid;
        persistUi();
        render();
        break;
      case "s":
      case "S":
        searchOpen = true;
        searchBox.hidden = false;
        searchIn.focus();
        break;
      case "Escape":
        if (videoEl.classList.contains("fs")) {
          videoEl.classList.remove("fs");
          syncRect();
        } else if (searchOpen) {
          searchOpen = false;
          searchBox.hidden = true;
          filter = "";
          searchIn.value = "";
          render();
        }
        break;
    }
  };
  window.addEventListener("keydown", onKey);
  searchIn.addEventListener("input", () => {
    filter = searchIn.value;
    persistUi();
    render();
  });

  let poll = window.setInterval(() => {
    void invoke<Snapshot>("ghoul_snapshot")
      .then((s) => {
        snap = s;
        renderTree();
        renderGrid();
        renderFoot();
      })
      .catch(() => {
        /* unmounted */
      });
  }, 1000);

  void invoke("ghoul_mount")
    .then(() => {
      syncRect();
      return zap(index);
    })
    .catch((e) => toast(String(e)));

  render();

  return () => {
    persistUi();
    window.clearInterval(poll);
    window.clearTimeout(zapTimer);
    window.removeEventListener("keydown", onKey);
    window.removeEventListener("resize", syncRect);
    ro.disconnect();
    root.closest(".page-pane, .page")?.classList.remove("gh-live");
    void invoke("ghoul_unmount");
  };
}
