import { invoke } from "@tauri-apps/api/core";
import { applyPlayGate, applyPlayerEngineValue, canPlay, playerEngineOptionsHtml } from "./capabilities";
import { api, type ChannelAudit } from "./api";
import { bindVirtualList, type VirtualList } from "./virtual";
import { bindPlayerLogo } from "./logo-src";
import { bindThreeColSplit } from "./split";

export type Variant = {
  id: string;
  managedChannelId: string;
  url: string;
  label?: string | null;
  originName?: string | null;
  originTvgId?: string | null;
  visibility: string;
  priority: number;
};

export type Managed = {
  id: string;
  name: string;
  groupTitle: string;
  tvgId?: string | null;
  tvgLogo?: string | null;
  notes?: string | null;
  sortOrder: number;
  tvgShiftHours: number;
  inTuner: boolean;
  tunerNumber?: number | null;
  variants: Variant[];
  hasEpgMatch: boolean;
};

const STARTED_KEY = "studio-editor-started";

const SHIFTS: { hours: number; label: string }[] = [
  { hours: 0, label: "0    ·  GMT−5  Eastern — New York, Toronto, Bogotá" },
  { hours: -1, label: "−1   ·  GMT−6  Central — Chicago, Mexico City, Winnipeg" },
  { hours: -2, label: "−2   ·  GMT−7  Mountain — Denver, Phoenix, Calgary" },
  { hours: -3, label: "−3   ·  GMT−8  Pacific — Los Angeles, Vancouver, Tijuana" },
  { hours: -4, label: "−4   ·  GMT−9  Alaska — Anchorage" },
  { hours: -5, label: "−5   ·  GMT−10 Hawaii — Honolulu, Tahiti" },
  { hours: -6, label: "−6   ·  GMT−11 Samoa, Midway" },
  { hours: -7, label: "−7   ·  GMT−12 Baker Island" },
  { hours: 1, label: "+1   ·  GMT−4  Atlantic — Halifax, Santo Domingo, La Paz" },
  { hours: 2, label: "+2   ·  GMT−3  São Paulo, Buenos Aires, Montevideo" },
  { hours: 3, label: "+3   ·  GMT−2  South Georgia / mid-Atlantic" },
  { hours: 4, label: "+4   ·  GMT−1  Azores, Cape Verde" },
  { hours: 5, label: "+5   ·  GMT+0  UTC — London, Lisbon, Reykjavik, Accra" },
  { hours: 6, label: "+6   ·  GMT+1  CET — Paris, Berlin, Rome, Lagos, Madrid" },
  { hours: 7, label: "+7   ·  GMT+2  EET — Cairo, Athens, Johannesburg, Helsinki" },
  { hours: 8, label: "+8   ·  GMT+3  Moscow, Istanbul, Riyadh, Nairobi, Kuwait" },
  { hours: 9, label: "+9   ·  GMT+4  Dubai, Baku, Tbilisi, Mauritius" },
  { hours: 10, label: "+10  ·  GMT+5  Pakistan (PKT), Maldives, Yekaterinburg" },
  { hours: 10.5, label: "+10.5 ·  GMT+5:30 India (IST) — Mumbai, Delhi, Colombo" },
  { hours: 11, label: "+11  ·  GMT+6  Bangladesh, Almaty, Omsk, Bhutan" },
  { hours: 11.5, label: "+11.5 ·  GMT+6:30 Myanmar, Cocos Islands" },
  { hours: 12, label: "+12  ·  GMT+7  Bangkok, Jakarta, Ho Chi Minh, Hanoi" },
  { hours: 13, label: "+13  ·  GMT+8  China, Singapore, Hong Kong, Perth, Manila" },
  { hours: 14, label: "+14  ·  GMT+9  Japan (JST), Korea (KST), Yakutsk" },
  { hours: 14.5, label: "+14.5 ·  GMT+9:30 Adelaide, Darwin (ACST)" },
  { hours: 15, label: "+15  ·  GMT+10 Sydney, Melbourne, Brisbane, Guam" },
  { hours: 16, label: "+16  ·  GMT+11 Magadan, Solomon Islands, New Caledonia" },
  { hours: 17, label: "+17  ·  GMT+12 Auckland, Fiji, Kamchatka, Marshall Islands" },
  { hours: 18, label: "+18  ·  GMT+13 Tonga, Samoa (DST), Phoenix Islands" },
];

export function editorHtml(): string {
  return `
    <h1 class="page-title">Playlist Editor</h1>
    <p class="page-sub">1) Load or create a curated playlist  2) Pick group → channel  3) Edit metadata / type tvg-id for EPG suggestions  4) Add stream backups manually below.  Right-click a group to rename.</p>
    <div class="source-bar">
      <fieldset class="player-field">
        <legend>Video Player</legend>
        <select id="ed-player" title="Player for Play button">
          ${playerEngineOptionsHtml()}
        </select>
      </fieldset>
    </div>
    <div class="empty" id="ed-blank">
      <div class="glyph">☰</div>
      <p id="ed-blank-msg">Load a curated playlist to get started</p>
      <div class="empty-actions">
        <button class="accent" id="ed-empty-load">Load curated playlist</button>
        <button id="ed-empty-create">Create curated playlist</button>
      </div>
    </div>
    <div class="editor-workspace" id="ed-workspace" hidden>
    <div class="tabs-row" id="ed-actions">
      <button class="accent" id="ed-load" title="Import your curated m3u/m3u8 as the base list">Load curated playlist</button>
      <button id="ed-add-from" title="Add channels that are missing from your curated list (does not auto-add stream backups)">Add channels from sources</button>
      <button id="ed-export">Export m3u8</button>
      <span class="page-sub" id="ed-count"></span>
      <div class="source-row-actions">
        <button type="button" class="tab-ico" id="ed-refresh" title="Refresh playlist">&#xE72C;</button>
        <button type="button" class="tab-ico tab-del" id="ed-clear" title="Remove all channels from the managed playlist">&#xE74D;</button>
      </div>
    </div>
    <div class="editor-grid editor-split" id="ed-split">
      <section class="groups editor-pane">
        <div class="groups-head">Groups</div>
        <div id="ed-groups" class="groups-body"></div>
      </section>
      <div class="split-handle" id="ed-split-groups" title="Drag to resize groups"></div>
      <section class="channels editor-pane">
        <div class="chan-head no-add">
          <span class="col-icon">Audit</span>
          <span class="col-icon">Play</span>
          <span>Name <button type="button" class="col-url-show" title="Show URL column">URL</button></span>
          <button type="button" class="col-url" title="Hide URL column">URL</button>
        </div>
        <div id="ed-channels" class="editor-list"></div>
      </section>
      <div class="split-handle" id="ed-split-chans" title="Drag to resize channels"></div>
      <section class="tile editor-pane" id="ed-form">
        <div class="groups-head">Edit channel</div>
        <p class="page-sub" id="ed-status"></p>
        <p class="page-sub" id="ed-empty">Select a channel.</p>
        <div id="ed-fields" hidden>
          <div class="field"><label>Name</label><input id="ed-name" /></div>
          <div class="field"><label>Group</label><input id="ed-group" placeholder="Type a group name…" list="ed-group-list" /></div>
          <datalist id="ed-group-list"></datalist>
          <div class="field">
            <label>tvg-id (type for EPG suggestions)</label>
            <div class="tvg-row">
              <input id="ed-tvg" placeholder="Start typing a channel id or name…" />
              <span id="ed-tvg-check" class="tvg-check" title="tvg-id matches the EPG catalog" hidden>&#xE73E;</span>
            </div>
            <div id="ed-suggest" class="suggest" hidden></div>
          </div>
          <div class="field">
            <label>EPG timeshift / tvg-shift (hours vs Eastern-style guide)</label>
            <select id="ed-shift"></select>
          </div>
          <div class="now-card" id="ed-now" hidden>
            <div class="now-label">NOW PLAYING</div>
            <div id="ed-now-title"></div>
            <div id="ed-now-times" class="chan-sub"></div>
          </div>
          <p class="page-sub">Suggestions + now playing load from the epg.monster catalog. Use timeshift when a West (or other delayed) feed shares an East EPG id.</p>
          <div class="field"><label>Logo URL (tvg-logo)</label><input id="ed-logo" /></div>
          <div class="logo-preview" id="ed-logo-preview"></div>
          <div class="field"><label>Primary stream URL (exported)</label><input id="ed-primary" /></div>
          <div class="field"><label>Notes</label><textarea id="ed-notes"></textarea></div>
          <button class="accent" id="ed-save">Save channel</button>
          <button id="ed-delete">Delete channel</button>
          <div class="stream-backups">
            <h2>Stream + Backups</h2>
            <p class="page-sub">Top row is exported. Use the arrows to change order. Play to preview. Info shows the source channel name / tvg-id.</p>
            <div id="ed-streams"></div>
            <div class="field"><label>Add stream URL</label><input id="ed-new-url" /></div>
            <div class="field"><label>Label</label><input id="ed-new-label" placeholder="e.g. IPTOR" /></div>
            <button id="ed-add-stream">Add stream</button>
          </div>
        </div>
      </section>
    </div>
    </div>
    <div class="dialog-backdrop" id="ed-src-dlg">
      <div class="dialog" style="width:480px;max-height:80vh;overflow:auto">
        <h2>Add missing channels from source</h2>
        <p class="page-sub" id="ed-src-sub">Only adds channels not already in your managed list. Stream backups must be added manually on each channel.</p>
        <div class="field"><label>Source</label><select id="ed-src-source"></select></div>
        <div class="field"><label>Group</label><select id="ed-src-group"></select></div>
        <div class="field"><label>Filter channels</label><input id="ed-src-filter" placeholder="name contains…" /></div>
        <div id="ed-src-list" class="editor-list" style="max-height:240px"></div>
        <div class="dialog-actions">
          <button id="ed-src-close">Cancel</button>
          <button id="ed-src-create-empty" hidden>Create empty playlist</button>
          <button class="accent" id="ed-src-add">Add selected</button>
        </div>
      </div>
    </div>
    <div class="dialog-backdrop" id="ed-del-dlg">
      <div class="dialog">
        <h2>Delete channel?</h2>
        <p class="page-sub" id="ed-del-msg">Remove this channel from the managed playlist?</p>
        <div class="dialog-actions">
          <button type="button" id="ed-del-no">Cancel</button>
          <button type="button" class="accent" id="ed-del-yes">Delete</button>
        </div>
      </div>
    </div>
    <div class="dialog-backdrop" id="ed-clear-dlg">
      <div class="dialog">
        <h2>Clear managed playlist?</h2>
        <p class="page-sub">Remove all channels from the managed playlist?</p>
        <div class="dialog-actions">
          <button type="button" id="ed-clear-no">Cancel</button>
          <button type="button" class="accent" id="ed-clear-yes">Clear</button>
        </div>
      </div>
    </div>
    <div class="dialog-backdrop" id="ed-load-dlg">
      <div class="dialog">
        <h2>Load curated playlist</h2>
        <p class="page-sub" id="ed-load-body"></p>
        <div class="dialog-actions">
          <button id="ed-load-cancel">Cancel</button>
          <button id="ed-load-merge">Merge</button>
          <button class="accent" id="ed-load-replace">Replace</button>
        </div>
      </div>
    </div>
    <div class="dialog-backdrop" id="ed-info-dlg">
      <div class="dialog" style="width:420px">
        <h2>Stream info</h2>
        <div class="field"><label>Origin name</label><p id="ed-info-name" class="page-sub"></p></div>
        <div class="field"><label>Origin tvg-id</label><p id="ed-info-tvg" class="page-sub"></p></div>
        <div class="dialog-actions">
          <button class="accent" id="ed-info-close">Close</button>
        </div>
      </div>
    </div>
    <div class="group-rename-pop" id="ed-rename" hidden>
      <div class="group-rename-title">Rename group</div>
      <input id="ed-rename-box" />
      <p class="page-sub">Enter to save · Esc or click away to cancel</p>
    </div>
  `;
}

export async function mountEditor(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  const split = page.querySelector<HTMLElement>("#ed-split");
  const splitGroups = page.querySelector<HTMLElement>("#ed-split-groups");
  const splitChans = page.querySelector<HTMLElement>("#ed-split-chans");
  if (split && splitGroups && splitChans) {
    bindThreeColSplit(split, splitGroups, splitChans, "studio-ed");
  }

  applyPlayGate(page);

  const playerSel = page.querySelector<HTMLSelectElement>("#ed-player");
  if (playerSel) {
    void invoke<Record<string, unknown>>("load_settings").then((st) => {
      applyPlayerEngineValue(
        page.querySelector<HTMLSelectElement>("#ed-player"),
        st.DefaultPlayer ?? 2,
      );
    });
    playerSel.addEventListener("change", async () => {
      const sel = page.querySelector<HTMLSelectElement>("#ed-player");
      if (!sel) return;
      try {
        const st = await invoke<Record<string, unknown>>("load_settings");
        st.DefaultPlayer = parseInt(sel.value, 10) || 0;
        await invoke("save_settings", { settings: st });
      } catch (e) {
        toast(String(e));
      }
    });
  }

  const shift = page.querySelector<HTMLSelectElement>("#ed-shift")!;
  for (const z of SHIFTS) {
    const o = document.createElement("option");
    o.value = String(z.hours);
    o.textContent = z.label;
    shift.appendChild(o);
  }

  let group = "";
  let filter = "";
  let allManaged: Managed[] = [];
  let selected: Managed | null = null;
  let draft = false;
  let chanVirt: VirtualList<Managed> | null = null;
  let groupVirt: VirtualList<{ title: string; count: number }> | null = null;
  let lastEdChans: Managed[] = [];
  type EdProbe = { state: "run" } | { state: "done"; text: string; grade: string; until: number };
  const edProbe = new Map<string, EdProbe>();
  let edProbeTimer = 0;
  const AUDIT_SHOW_MS = 20_000;

  const auditLine = (r: ChannelAudit): { text: string; grade: string } => {
    const grade = (r.grade || (r.ok ? "C" : "F")).toUpperCase();
    if (!r.ok && r.error) return { text: r.error, grade };
    const parts: string[] = [];
    if (r.width && r.height) parts.push(`${r.width}\u00d7${r.height}`);
    if (r.aspectRatio) parts.push(r.aspectRatio);
    if (r.fps && r.fps > 0) parts.push(`${Math.round(r.fps)} fps`);
    if (r.videoCodec) parts.push(r.videoCodec);
    if (r.audioCodec) parts.push(r.audioCodec);
    if (r.latencyMs != null) parts.push(`${r.latencyMs} ms`);
    return { text: parts.join(" \u00b7 ") || "OK", grade };
  };

  const urlCellHtml = (c: Managed, url: string): string => {
    const p = edProbe.get(c.id);
    if (p?.state === "run") {
      return `<span class="ed-audit-url wait">Testing\u2026</span>`;
    }
    if (p?.state === "done" && p.until > Date.now()) {
      return `<span class="ed-audit-url" data-g="${esc(p.grade)}" title="${esc(p.text)}">${esc(p.grade)} \u00b7 ${esc(p.text)}</span>`;
    }
    return `<span class="copy url" data-copy="${esc(url)}" title="${esc(url)}">${esc(truncateUrl(url, 64))}</span>`;
  };

  const refreshChanRows = () => {
    if (chanVirt) chanVirt.setItems(lastEdChans);
  };

  const scheduleProbeClear = () => {
    if (edProbeTimer) return;
    edProbeTimer = window.setInterval(() => {
      const now = Date.now();
      let changed = false;
      for (const [id, p] of [...edProbe]) {
        if (p.state === "done" && p.until <= now) {
          edProbe.delete(id);
          changed = true;
        }
      }
      if (changed) refreshChanRows();
      if (![...edProbe.values()].some((p) => p.state === "done")) {
        window.clearInterval(edProbeTimer);
        edProbeTimer = 0;
      }
    }, 250);
  };

  const filtered = () => allManaged.filter((c) => matchesEditorSearch(c, filter));

  const renamePop = page.querySelector<HTMLElement>("#ed-rename")!;
  const renameBox = page.querySelector<HTMLInputElement>("#ed-rename-box")!;
  let renameOld = "";
  const closeRename = () => {
    renamePop.hidden = true;
  };
  const openRename = (title: string, anchor: HTMLElement) => {
    renameOld = title;
    renameBox.value = title;
    renamePop.hidden = false;
    const pageRect = page.getBoundingClientRect();
    const r = anchor.getBoundingClientRect();
    const x = Math.max(8, Math.min(r.left - pageRect.left, page.clientWidth - 240));
    const y = Math.max(8, Math.min(r.bottom - pageRect.top + 4, page.clientHeight - 120));
    renamePop.style.left = `${x}px`;
    renamePop.style.top = `${y}px`;
    window.setTimeout(() => {
      renameBox.focus();
      renameBox.select();
    }, 0);
  };
  const commitRename = async () => {
    const newName = renameBox.value.trim() || "Ungrouped";
    closeRename();
    if (newName === renameOld) return;
    try {
      const updated = await invoke<number>("rename_managed_group", {
        oldName: renameOld,
        newName,
      });
      group = newName;
      if (selected && sameGroup(selected.groupTitle, renameOld)) {
        selected.groupTitle = newName;
        (page.querySelector("#ed-group") as HTMLInputElement).value = newName;
      }
      toast(
        updated > 0
          ? `Renamed group to “${newName}” (${updated} channel${updated === 1 ? "" : "s"})`
          : `Group rename: no channels matched “${renameOld}”`,
      );
      await reload();
    } catch (e) {
      toast(String(e));
    }
  };
  renameBox.addEventListener("keydown", (ev) => {
    if (ev.key === "Enter") {
      ev.preventDefault();
      void commitRename();
    } else if (ev.key === "Escape") {
      ev.preventDefault();
      closeRename();
    }
  });
  page.addEventListener("mousedown", (ev) => {
    if (renamePop.hidden) return;
    if (!renamePop.contains(ev.target as Node)) closeRename();
  });

  const playlistOpen = () =>
    allManaged.length > 0 || sessionStorage.getItem(STARTED_KEY) === "1";

  const applyEmptyState = () => {
    const open = playlistOpen();
    const blank = page.querySelector<HTMLElement>("#ed-blank");
    const work = page.querySelector<HTMLElement>("#ed-workspace");
    if (blank) blank.hidden = open;
    if (work) work.hidden = !open;
  };

  const refreshBlankCopy = async () => {
    const btn = page.querySelector<HTMLButtonElement>("#ed-empty-create");
    const msg = page.querySelector("#ed-blank-msg");
    try {
      const sources = await api.listSources();
      if (btn) {
        btn.title = sources.length
          ? "Create a curated list from loaded sources"
          : "Create an empty curated playlist";
      }
      if (msg) {
        msg.textContent = sources.length
          ? "Load a file, or create a curated playlist from your sources"
          : "Load a file, or create an empty curated playlist";
      }
    } catch {
      /* keep defaults */
    }
  };

  const reload = async () => {
    allManaged = await invoke<Managed[]>("list_managed", { group: null });
    applyEmptyState();
    const hits = filtered();
    const counts = new Map<string, number>();
    for (const c of hits) {
      counts.set(c.groupTitle, (counts.get(c.groupTitle) ?? 0) + 1);
    }
    const groups = [...counts.entries()]
      .map(([title, count]) => ({ title, count }))
      .sort((a, b) => a.title.localeCompare(b.title, undefined, { sensitivity: "base" }));
    const gEl = page.querySelector<HTMLElement>("#ed-groups");
    const dl = page.querySelector("#ed-group-list");
    if (!gEl || !dl) return;
    dl.innerHTML = "";
    let total = 0;
    for (const g of groups) {
      total += g.count;
      const opt = document.createElement("option");
      opt.value = g.title;
      dl.appendChild(opt);
    }
    if (groups.length && !groups.some((g) => sameGroup(g.title, group))) {
      group = groups[0].title;
    }
    if (!groups.length) group = "";
    groupVirt?.destroy();
    gEl.innerHTML = "";
    groupVirt = bindVirtualList({
      scroller: gEl,
      rowHeight: 36,
      renderRow: (g) => {
        const b = document.createElement("button");
        b.className = "group-row" + (g.title === group ? " active" : "");
        b.textContent = `${g.title}  (${g.count})`;
        b.addEventListener("click", () => {
          group = g.title;
          groupVirt?.setItems(groups);
          void loadChannels();
        });
        b.addEventListener("contextmenu", (ev) => {
          ev.preventDefault();
          openRename(g.title, b);
        });
        b.addEventListener("dblclick", (ev) => {
          ev.preventDefault();
          openRename(g.title, b);
        });
        return b;
      },
    });
    const count = page.querySelector("#ed-count");
    if (count) count.textContent = `${total} channels`;
    if (!group && groups[0]) group = groups[0].title;
    groupVirt.setItems(groups);
    await loadChannels();
  };

  const loadChannels = async () => {
    const list = page.querySelector<HTMLElement>("#ed-channels");
    if (!list) return;
    if (!group) {
      chanVirt?.destroy();
      chanVirt = null;
      list.innerHTML = "";
      return;
    }
    const raw = await invoke<Managed[]>("list_managed", { group, hydrate: true });
    const chans = raw.filter((c) => matchesEditorSearch(c, filter));
    lastEdChans = chans;
    if (!page.querySelector("#ed-channels")) return;
    if (!chanVirt) {
      chanVirt = bindVirtualList({
        scroller: list,
        rowHeight: 48,
        renderRow: (c) => {
          const url = primaryUrl(c);
          const row = document.createElement("div");
          row.className = "chan-row no-add" + (selected?.id === c.id ? " active" : "");
          row.dataset.id = c.id;
          row.innerHTML = `
        <button class="probe" type="button" data-id="${esc(c.id)}" data-url="${esc(url)}" title="Audit this stream">&#xE9E9;</button>
        <button class="play" type="button" data-id="${esc(c.id)}" data-url="${esc(url)}" title="${canPlay() ? "Play stream" : "Install mpv or VLC, or set a player path in Settings"}" ${canPlay() ? "" : "disabled"}>&#xE768;</button>
        <div class="chan-main">
          <span class="logo-slot">
            ${c.tvgLogo ? `<img alt="" />` : `<span class="logo-broken">&#xE7BA;</span>`}
          </span>
          <div class="chan-meta">
            <div class="chan-name" title="${esc(c.name)}">${esc(c.name)}</div>
            <div class="chan-sub">${esc(c.tvgId ?? "")}${
              c.hasEpgMatch
                ? ` <span class="tvg-check" title="tvg-id matches EPG catalog">&#xE73E;</span>`
                : ""
            }</div>
          </div>
        </div>
        ${urlCellHtml(c, url)}`;
          const img = row.querySelector("img");
          const fail = () => {
            const slot = row.querySelector(".logo-slot");
            if (!slot) return;
            slot.innerHTML = `<span class="logo-broken">&#xE7BA;</span>`;
          };
          if (img && c.tvgLogo) bindPlayerLogo(img, c.tvgLogo, fail);
          return row;
        },
      });
    }
    chanVirt.setItems(chans);
    applyPlayGate(page);
  };

  const edChannels = page.querySelector<HTMLElement>(".channels");
  if (edChannels) {
    edChannels.classList.toggle("hide-url", localStorage.getItem("studio-hide-url-col") === "1");
    edChannels.addEventListener("click", (ev) => {
      const t = ev.target as HTMLElement;
      if (!t.classList.contains("col-url") && !t.classList.contains("col-url-show")) return;
      const hide = localStorage.getItem("studio-hide-url-col") !== "1";
      localStorage.setItem("studio-hide-url-col", hide ? "1" : "0");
      edChannels.classList.toggle("hide-url", hide);
    });
  }

  page.querySelector("#ed-channels")?.addEventListener("click", async (ev) => {
    const t = ev.target as HTMLElement;
    const row = t.closest(".chan-row") as HTMLElement | null;
    if (!row?.dataset.id) return;
    const id = row.dataset.id;
    if (t.classList.contains("play")) {
      if (!canPlay()) {
        toast("Install mpv or VLC, or set a player path in Settings");
        return;
      }
      const url = t.dataset.url ?? "";
      if (!url) {
        toast("No stream URL on this channel");
        return;
      }
      try {
        toast("Checking stream…");
        await api.playUrl(url);
        toast("Playing");
      } catch (e) {
        toast(String(e));
      }
      return;
    }
    if (t.classList.contains("probe")) {
      const url = t.dataset.url ?? "";
      if (!url) {
        toast("No stream URL on this channel");
        return;
      }
      if (edProbe.get(id)?.state === "run") return;
      edProbe.set(id, { state: "run" });
      refreshChanRows();
      try {
        const result = await api.auditSourceChannel(url);
        const line = auditLine(result);
        edProbe.set(id, { state: "done", text: line.text, grade: line.grade, until: Date.now() + AUDIT_SHOW_MS });
      } catch (e) {
        edProbe.set(id, {
          state: "done",
          text: String(e),
          grade: "F",
          until: Date.now() + AUDIT_SHOW_MS,
        });
      }
      refreshChanRows();
      scheduleProbeClear();
      return;
    }
    if (t.classList.contains("copy") && t.dataset.copy) {
      await navigator.clipboard.writeText(t.dataset.copy);
      toast("Copied.");
      return;
    }
    void select(id);
  });

  const select = async (id: string) => {
    selected = (await invoke<Managed | null>("get_managed", { id })) ?? null;
    if (!page.querySelector("#ed-fields")) return;
    await paintForm();
    await loadChannels();
  };

  const paintForm = async () => {
    const empty = page.querySelector<HTMLElement>("#ed-empty")!;
    const fields = page.querySelector<HTMLElement>("#ed-fields")!;
    if (!selected) {
      empty.hidden = false;
      fields.hidden = true;
      return;
    }
    empty.hidden = true;
    fields.hidden = false;
    (page.querySelector("#ed-name") as HTMLInputElement).value = selected.name;
    (page.querySelector("#ed-group") as HTMLInputElement).value = selected.groupTitle;
    (page.querySelector("#ed-tvg") as HTMLInputElement).value = selected.tvgId ?? "";
    (page.querySelector("#ed-logo") as HTMLInputElement).value = selected.tvgLogo ?? "";
    (page.querySelector("#ed-primary") as HTMLInputElement).value =
      selected.variants.find((v) => v.visibility === "visible")?.url ?? selected.variants[0]?.url ?? "";
    (page.querySelector("#ed-notes") as HTMLTextAreaElement).value = selected.notes ?? "";
    shift.value = String(selected.tvgShiftHours);
    await updateMatch();
    paintStreams();
    paintLogo();
  };

  const updateMatch = async () => {
    const tvgEl = page.querySelector<HTMLInputElement>("#ed-tvg");
    if (!tvgEl || !page.querySelector("#ed-fields")) return;
    const tvg = tvgEl.value.trim();
    const known = tvg ? await invoke<boolean>("is_known_tvg", { tvgId: tvg }) : false;
    if (!page.querySelector("#ed-fields")) return;
    const check = page.querySelector<HTMLElement>("#ed-tvg-check");
    const input = page.querySelector<HTMLInputElement>("#ed-tvg");
    const now = page.querySelector<HTMLElement>("#ed-now");
    const title = page.querySelector<HTMLElement>("#ed-now-title");
    const times = page.querySelector<HTMLElement>("#ed-now-times");
    if (!check || !input || !now || !title || !times) return;
    check.hidden = !known;
    input.classList.toggle("tvg-ok", known);
    if (!known) {
      now.hidden = true;
      return;
    }
    const hours = Number(shift.value);
    const np = await invoke<{ title: string; startLocal: string; stopLocal: string } | null>("now_playing", {
      tvgId: tvg,
      shiftHours: hours,
    });
    if (!page.querySelector("#ed-fields")) return;
    const titleEl = page.querySelector<HTMLElement>("#ed-now-title");
    const timesEl = page.querySelector<HTMLElement>("#ed-now-times");
    const nowEl = page.querySelector<HTMLElement>("#ed-now");
    if (!titleEl || !timesEl || !nowEl) return;
    const shiftNote = Math.abs(hours) > 0.01 ? `  ·  tvg-shift ${hours > 0 ? "+" : ""}${hours}` : "";
    nowEl.hidden = false;
    if (!np) {
      titleEl.textContent = "No programme at this time";
      timesEl.textContent = shiftNote
        ? `Nothing scheduled at the shifted guide time.${shiftNote}`
        : "Guide has this tvg-id, but nothing is scheduled at the current (shifted) time.";
    } else {
      titleEl.textContent = np.title;
      timesEl.textContent = `${np.startLocal}–${np.stopLocal}${shiftNote}`;
    }
  };

  const paintStreams = () => {
    const box = page.querySelector("#ed-streams")!;
    box.innerHTML = "";
    if (!selected) return;
    for (const v of selected.variants) {
      const row = document.createElement("div");
      row.className = "stream-row";
      row.innerHTML = `
        <button data-act="play" data-id="${v.id}" ${canPlay() ? "" : "disabled"} title="${canPlay() ? "Play" : "Install mpv or VLC, or set a player path in Settings"}">Play</button>
        <div>
          <div>${esc(v.label || (v.visibility === "visible" ? "visible" : "backup"))}</div>
          <div class="chan-sub stream-url" title="${esc(v.url)}">${esc(truncateUrl(v.url))}</div>
        </div>
        <button data-act="info" data-id="${v.id}">Info</button>
        <button data-act="up" data-id="${v.id}">↑</button>
        <button data-act="down" data-id="${v.id}">↓</button>
        <button data-act="rm" data-id="${v.id}">Remove</button>
      `;
      box.appendChild(row);
    }
    applyPlayGate(page);
  };

  const paintLogo = () => {
    const logo = page.querySelector("#ed-logo") as HTMLInputElement | null;
    const slot = page.querySelector("#ed-logo-preview");
    if (!logo || !slot) return;
    const url = logo.value.trim();
    if (!url) {
      slot.innerHTML = `<span class="broken">broken logo</span>`;
      return;
    }
    slot.innerHTML = `<img alt="" />`;
    const img = slot.querySelector("img");
    const fail = () => {
      slot.innerHTML = `<span class="broken">broken logo</span>`;
    };
    if (img) bindPlayerLogo(img, url, fail);
  };

  const gather = (): Managed | null => {
    if (!selected) return null;
    return {
      ...selected,
      name: (page.querySelector("#ed-name") as HTMLInputElement).value,
      groupTitle:
        (page.querySelector("#ed-group") as HTMLInputElement).value.trim() || "Ungrouped",
      tvgId: (page.querySelector("#ed-tvg") as HTMLInputElement).value.trim() || null,
      tvgLogo: (page.querySelector("#ed-logo") as HTMLInputElement).value.trim() || null,
      notes: (page.querySelector("#ed-notes") as HTMLTextAreaElement).value || null,
      tvgShiftHours: Number(shift.value),
    };
  };

  page.querySelector("#ed-save")!.addEventListener("click", async () => {
    const ch = gather();
    if (!ch) return;
    try {
      const primary = (page.querySelector("#ed-primary") as HTMLInputElement).value.trim();
      await invoke("save_managed", { channel: ch, primaryUrl: primary || null });
      draft = false;
      toast("Channel saved");
      selected = await invoke("get_managed", { id: ch.id });
      group = selected?.groupTitle ?? group;
      const status = page.querySelector("#ed-status");
      if (status) status.textContent = "";
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });

  const delDlg = page.querySelector("#ed-del-dlg")!;
  page.querySelector("#ed-delete")!.addEventListener("click", async () => {
    if (!selected) return;
    if (draft) {
      draft = false;
      selected = null;
      const st = page.querySelector("#ed-status");
      if (st) st.textContent = "Draft discarded.";
      await paintForm();
      return;
    }
    const msg = page.querySelector("#ed-del-msg");
    if (msg) msg.textContent = `Remove “${selected.name}” from the managed playlist?`;
    delDlg.classList.add("open");
  });
  page.querySelector("#ed-del-no")!.addEventListener("click", () => delDlg.classList.remove("open"));
  page.querySelector("#ed-del-yes")!.addEventListener("click", async () => {
    delDlg.classList.remove("open");
    if (!selected) return;
    try {
      await invoke("delete_managed", { id: selected.id });
      selected = null;
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#ed-refresh")!.addEventListener("click", () => void reload());
  const runImport = async (replace: boolean) => {
    const msg = await invoke<string>("import_curated", { replace });
    if (msg === "cancelled") return;
    toast(msg);
    group = "";
    selected = null;
    await reload();
  };
  const onLoadClick = async () => {
    try {
      const n = await api.managedCount();
      if (n <= 0) {
        await runImport(true);
        return;
      }
      const dlg = page.querySelector("#ed-load-dlg")!;
      page.querySelector("#ed-load-body")!.innerHTML =
        `You already have ${n} managed channel(s).<br><br>` +
        "• Replace — clear the managed list and load this file as the new base<br>" +
        "• Merge — keep existing; add only channels that are not already present<br><br>" +
        "Stream backups are never auto-added — use “Add stream backup” on each channel.";
      dlg.classList.add("open");
    } catch (e) {
      toast(String(e));
    }
  };
  page.querySelector("#ed-load")!.addEventListener("click", () => void onLoadClick());
  page.querySelector("#ed-empty-load")!.addEventListener("click", () => void onLoadClick());
  page.querySelector("#ed-empty-create")!.addEventListener("click", async () => {
    try {
      const sources = await api.listSources();
      if (sources.length === 0) {
        sessionStorage.setItem(STARTED_KEY, "1");
        toast("Empty curated playlist. Add channels from sources, or load a file.");
        await reload();
        return;
      }
      await openAddFrom(true);
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#ed-info-close")?.addEventListener("click", () => {
    page.querySelector("#ed-info-dlg")?.classList.remove("open");
  });
  page.querySelector("#ed-info-dlg")?.addEventListener("click", (ev) => {
    if (ev.target === ev.currentTarget) {
      page.querySelector("#ed-info-dlg")?.classList.remove("open");
    }
  });
  page.querySelector("#ed-load-cancel")!.addEventListener("click", () => {
    page.querySelector("#ed-load-dlg")!.classList.remove("open");
  });
  page.querySelector("#ed-load-replace")!.addEventListener("click", () => {
    page.querySelector("#ed-load-dlg")!.classList.remove("open");
    void runImport(true).catch((e) => toast(String(e)));
  });
  page.querySelector("#ed-load-merge")!.addEventListener("click", () => {
    page.querySelector("#ed-load-dlg")!.classList.remove("open");
    void runImport(false).catch((e) => toast(String(e)));
  });
  page.querySelector("#ed-export")!.addEventListener("click", async () => {
    try {
      const msg = await invoke<string>("export_managed", { includeBackups: false });
      if (msg !== "cancelled") toast(msg);
    } catch (e) {
      toast(String(e));
    }
  });
  const clearDlg = page.querySelector("#ed-clear-dlg")!;
  page.querySelector("#ed-clear")!.addEventListener("click", () => clearDlg.classList.add("open"));
  page.querySelector("#ed-clear-no")!.addEventListener("click", () => clearDlg.classList.remove("open"));
  page.querySelector("#ed-clear-yes")!.addEventListener("click", async () => {
    clearDlg.classList.remove("open");
    try {
      await invoke("clear_managed");
      group = "";
      selected = null;
      toast("Cleared.");
      sessionStorage.removeItem(STARTED_KEY);
      await reload();
      await refreshBlankCopy();
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#ed-logo")!.addEventListener("input", paintLogo);
  shift.addEventListener("change", () => void updateMatch());

  const tvg = page.querySelector<HTMLInputElement>("#ed-tvg")!;
  const sug = page.querySelector<HTMLElement>("#ed-suggest")!;
  let suggestHits: { tvgId: string; name: string; line: string }[] = [];
  let suggestHi = 0;
  const applySuggestion = (h: { tvgId: string; name: string }) => {
    tvg.value = h.tvgId;
    sug.hidden = true;
    suggestHits = [];
    const name = page.querySelector<HTMLInputElement>("#ed-name")!;
    if (!name.value.trim()) name.value = h.name;
    void updateMatch();
  };
  const paintSuggest = () => {
    sug.innerHTML = "";
    suggestHits.forEach((h, i) => {
      const b = document.createElement("button");
      b.className = "suggest-item" + (i === suggestHi ? " active" : "");
      b.textContent = h.line;
      b.addEventListener("mousedown", (ev) => {
        ev.preventDefault();
        applySuggestion(h);
      });
      sug.appendChild(b);
    });
    sug.hidden = suggestHits.length === 0;
  };
  tvg.addEventListener("input", () => {
    void (async () => {
      await updateMatch();
      const q = tvg.value.trim();
      if (!q) {
        suggestHits = [];
        sug.hidden = true;
        return;
      }
      suggestHits = await invoke("suggest_tvg", { query: q });
      suggestHi = 0;
      paintSuggest();
    })();
  });
  tvg.addEventListener("keydown", (ev) => {
    if (sug.hidden || suggestHits.length === 0) return;
    if (ev.key === "ArrowDown") {
      ev.preventDefault();
      suggestHi = (suggestHi + 1) % suggestHits.length;
      paintSuggest();
    } else if (ev.key === "ArrowUp") {
      ev.preventDefault();
      suggestHi = (suggestHi - 1 + suggestHits.length) % suggestHits.length;
      paintSuggest();
    } else if (ev.key === "Enter") {
      ev.preventDefault();
      applySuggestion(suggestHits[suggestHi] ?? suggestHits[0]);
    } else if (ev.key === "Escape") {
      sug.hidden = true;
    }
  });

  page.querySelector("#ed-add-stream")!.addEventListener("click", async () => {
    if (!selected) return;
    const url = (page.querySelector("#ed-new-url") as HTMLInputElement).value.trim();
    const label = (page.querySelector("#ed-new-label") as HTMLInputElement).value.trim();
    if (!url) return;
    try {
      await invoke("add_stream", { managedId: selected.id, url, label: label || null });
      (page.querySelector("#ed-new-url") as HTMLInputElement).value = "";
      selected = await invoke("get_managed", { id: selected.id });
      paintStreams();
    } catch (e) {
      toast(String(e));
    }
  });

  page.querySelector("#ed-streams")!.addEventListener("click", async (ev) => {
    const btn = (ev.target as HTMLElement).closest("button") as HTMLButtonElement | null;
    if (!btn || !selected) return;
    const id = btn.dataset.id!;
    const act = btn.dataset.act;
    try {
      if (act === "play") {
        if (!canPlay()) {
          toast("Install mpv or VLC, or set a player path in Settings");
          return;
        }
        const v = selected.variants.find((x) => x.id === id);
        if (v) await api.playUrl(v.url);
      } else if (act === "info") {
        const v = selected.variants.find((x) => x.id === id);
        const dlg = page.querySelector("#ed-info-dlg");
        const nameEl = page.querySelector("#ed-info-name");
        const tvgEl = page.querySelector("#ed-info-tvg");
        if (nameEl) nameEl.textContent = v?.originName?.trim() || "—";
        if (tvgEl) tvgEl.textContent = v?.originTvgId?.trim() || "—";
        dlg?.classList.add("open");
      } else if (act === "up") {
        await invoke("move_variant", { managedId: selected.id, variantId: id, delta: -1 });
        selected = await invoke("get_managed", { id: selected.id });
        paintStreams();
      } else if (act === "down") {
        await invoke("move_variant", { managedId: selected.id, variantId: id, delta: 1 });
        selected = await invoke("get_managed", { id: selected.id });
        paintStreams();
      } else if (act === "rm") {
        if (selected.variants.length <= 1) {
          toast("Keep at least one stream");
          return;
        }
        await invoke("delete_variant", { id });
        selected = await invoke("get_managed", { id: selected.id });
        paintStreams();
      }
    } catch (e) {
      toast(String(e));
    }
  });

  const srcDlg = page.querySelector("#ed-src-dlg")!;
  const srcSource = page.querySelector<HTMLSelectElement>("#ed-src-source")!;
  const srcGroup = page.querySelector<HTMLSelectElement>("#ed-src-group")!;
  const srcFilter = page.querySelector<HTMLInputElement>("#ed-src-filter")!;
  const srcList = page.querySelector<HTMLElement>("#ed-src-list")!;

  const fillSourceGroups = async () => {
    srcGroup.innerHTML = "";
    const first = document.createElement("option");
    first.value = "*";
    first.textContent = "(first group)";
    srcGroup.appendChild(first);
    const sid = srcSource.value;
    if (!sid) return;
    const groups = await api.listGroups(sid);
    for (const g of groups) {
      const o = document.createElement("option");
      o.value = g.title;
      o.textContent = g.title;
      srcGroup.appendChild(o);
    }
    srcGroup.selectedIndex = 0;
    await fillFromSources();
  };

  async function fillFromSources() {
    srcList.innerHTML = "";
    const sid = srcSource.value;
    if (!sid) return;
    const groups = await api.listGroups(sid);
    const tag = srcGroup.value;
    const title = tag && tag !== "*" ? tag : groups[0]?.title;
    if (!title) return;
    let entries = await api.listChannels(sid, title);
    if (!tag || tag === "*") entries = entries.slice(0, 300);
    const q2 = srcFilter.value.trim().toLowerCase();
    if (q2) {
      entries = entries.filter(
        (c) =>
          c.name.toLowerCase().includes(q2) || (c.tvgId?.toLowerCase().includes(q2) ?? false),
      );
    }
    for (const c of entries) {
      const row = document.createElement("label");
      row.className = "group-row src-pick";
      row.innerHTML = `<input type="checkbox" data-id="${esc(c.id)}" /> ${esc(c.name)}  [${esc(c.tvgId ?? "")}]`;
      srcList.appendChild(row);
    }
  }

  const openAddFrom = async (createFlow: boolean) => {
    const sources = await api.listSources();
    if (sources.length === 0) {
      toast("Load a source in Add Sources first");
      return false;
    }
    srcSource.innerHTML = "";
    for (const s of sources) {
      const o = document.createElement("option");
      o.value = s.id;
      o.textContent = `${s.name} (${s.channelCount})`;
      srcSource.appendChild(o);
    }
    srcSource.selectedIndex = 0;
    srcFilter.value = "";
    const emptyBtn = page.querySelector<HTMLButtonElement>("#ed-src-create-empty");
    if (emptyBtn) emptyBtn.hidden = !createFlow;
    const title = page.querySelector("#ed-src-dlg h2");
    if (title) {
      title.textContent = createFlow
        ? "Create curated playlist from sources"
        : "Add missing channels from source";
    }
    const sub = page.querySelector("#ed-src-sub");
    if (sub) {
      sub.textContent = createFlow
        ? "Pick channels from a loaded source, or create an empty playlist. Stream backups must be added manually on each channel."
        : "Only adds channels not already in your managed list. Stream backups must be added manually on each channel.";
    }
    srcDlg.classList.add("open");
    await fillSourceGroups();
    return true;
  };

  page.querySelector("#ed-add-from")!.addEventListener("click", () => {
    void openAddFrom(false).catch((e) => toast(String(e)));
  });
  page.querySelector("#ed-src-create-empty")?.addEventListener("click", async () => {
    srcDlg.classList.remove("open");
    sessionStorage.setItem(STARTED_KEY, "1");
    toast("Empty curated playlist. Add channels from sources, or load a file.");
    await reload();
  });
  page.querySelector("#ed-src-close")!.addEventListener("click", () => {
    srcDlg.classList.remove("open");
  });
  srcSource.addEventListener("change", () => void fillSourceGroups());
  srcGroup.addEventListener("change", () => void fillFromSources());
  srcFilter.addEventListener("input", () => void fillFromSources());
  page.querySelector("#ed-src-add")!.addEventListener("click", async () => {
    const ids = [...srcList.querySelectorAll<HTMLInputElement>("input:checked")].map((el) => el.dataset.id!);
    if (ids.length === 0) {
      toast("Pick at least one group or channel");
      return;
    }
    const label = srcSource.selectedOptions[0]?.textContent ?? "source";
    try {
      const msg = await invoke<string>("add_missing_from_source", {
        entryIds: ids,
        sourceLabel: label,
      });
      srcDlg.classList.remove("open");
      sessionStorage.setItem(STARTED_KEY, "1");
      toast(msg);
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });

  const beginDraft = (entry: { id: string; name: string; groupTitle: string; tvgId?: string | null; tvgLogo?: string | null; url: string }) => {
    draft = true;
    selected = {
      id: crypto.randomUUID().replace(/-/g, ""),
      name: entry.name,
      groupTitle: "Unassigned",
      tvgId: entry.tvgId ?? null,
      tvgLogo: entry.tvgLogo ?? null,
      notes: null,
      sortOrder: 0,
      tvgShiftHours: 0,
      inTuner: false,
      tunerNumber: null,
      variants: [
        {
          id: "draft-primary",
          managedChannelId: "",
          url: entry.url,
          label: "primary",
          visibility: "visible",
          priority: 0,
        },
      ],
      hasEpgMatch: false,
    };
    page.querySelector("#ed-status")!.textContent =
      "New channel draft — set the group, then Save channel.";
    void paintForm();
    (page.querySelector("#ed-group") as HTMLInputElement).focus();
  };

  page.addEventListener("studio-search", (ev) => {
    filter = (ev as CustomEvent<string>).detail ?? "";
    void reload().catch((e) => toast(String(e)));
    void refreshBlankCopy();
  });

  try {
    await reload();
    const raw = sessionStorage.getItem("studio-editor-draft");
    if (raw) {
      sessionStorage.removeItem("studio-editor-draft");
      beginDraft(JSON.parse(raw));
    }
  } catch (e) {
    toast(String(e));
  }
  void invoke<string>("rebuild_now_playing")
    .then(() => {
      if (selected && page.querySelector("#ed-fields")) {
        return updateMatch();
      }
    })
    .catch(() => undefined);
}

function primaryUrl(c: Managed): string {
  return (
    c.variants.find((v) => v.visibility === "visible")?.url ?? c.variants[0]?.url ?? ""
  );
}

function truncateUrl(url: string, max = 52): string {
  const u = url.trim();
  if (u.length <= max) return u;
  const keep = Math.max(16, Math.floor((max - 3) / 2));
  return `${u.slice(0, keep)}...${u.slice(-keep)}`;
}

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

export function matchesEditorSearch(c: Managed, q: string): boolean {
  const f = q.trim().toLowerCase();
  if (!f) return true;
  return (
    c.name.toLowerCase().includes(f) ||
    c.groupTitle.toLowerCase().includes(f) ||
    (c.tvgId?.toLowerCase().includes(f) ?? false)
  );
}

function sameGroup(a: string, b: string): boolean {
  return a.localeCompare(b, undefined, { sensitivity: "base" }) === 0;
}
