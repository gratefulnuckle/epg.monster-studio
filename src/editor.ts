import { invoke } from "@tauri-apps/api/core";
import { api } from "./api";

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
    <div class="editor-toolbar">
      <span class="editor-title">Managed playlist</span>
      <button class="accent" id="ed-load" title="Import your curated m3u/m3u8 as the base list">Load curated playlist…</button>
      <button id="ed-add-from" title="Add channels that are missing from your curated list (does not auto-add stream backups)">Add channels from sources…</button>
      <button id="ed-export">Export m3u8…</button>
      <button id="ed-refresh">Refresh</button>
      <button id="ed-clear" title="Remove all channels from the managed playlist">Clear</button>
      <span class="page-sub" id="ed-count"></span>
    </div>
    <p class="page-sub">1) Load curated playlist  2) Pick group → channel  3) Edit metadata / type tvg-id for EPG suggestions  4) Add stream backups manually below.  Right-click a group to rename.</p>
    <div class="editor-grid">
      <section class="tile editor-pane">
        <h2>Groups</h2>
        <div id="ed-groups" class="editor-list"></div>
      </section>
      <section class="tile editor-pane">
        <h2>Channels</h2>
        <div id="ed-channels" class="editor-list"></div>
      </section>
      <section class="tile editor-pane" id="ed-form">
        <h2>Edit</h2>
        <p class="page-sub" id="ed-empty">Select a channel.</p>
        <div id="ed-fields" hidden>
          <div class="field"><label>Name</label><input id="ed-name" /></div>
          <div class="field"><label>Group</label><input id="ed-group" placeholder="Type a group name…" list="ed-group-list" /></div>
          <datalist id="ed-group-list"></datalist>
          <div class="field">
            <label>tvg-id (type for EPG suggestions)</label>
            <div class="tvg-row">
              <input id="ed-tvg" placeholder="Start typing a channel id or name…" />
              <span id="ed-tvg-check" class="tvg-check" hidden>✓</span>
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
          <div class="field"><label>Logo URL (tvg-logo)</label><input id="ed-logo" /></div>
          <div class="logo-preview" id="ed-logo-preview"></div>
          <div class="field"><label>Primary stream URL (exported)</label><input id="ed-primary" /></div>
          <div class="field"><label>Notes</label><textarea id="ed-notes"></textarea></div>
          <button class="accent" id="ed-save">Save</button>
          <h2 style="margin-top:16px">Stream + backups</h2>
          <div id="ed-streams"></div>
          <div class="field"><label>Add stream URL</label><input id="ed-new-url" /></div>
          <div class="field"><label>Label</label><input id="ed-new-label" placeholder="e.g. IPTOR" /></div>
          <button id="ed-add-stream">Add stream</button>
        </div>
      </section>
    </div>
    <div class="dialog-backdrop" id="ed-src-dlg">
      <div class="dialog" style="width:640px;max-height:80vh;overflow:auto">
        <h2>Add channels from sources…</h2>
        <input id="ed-src-filter" placeholder="Filter by name / tvg-id" />
        <div id="ed-src-list" class="editor-list" style="max-height:360px"></div>
        <div class="dialog-actions">
          <button id="ed-src-close">Close</button>
        </div>
      </div>
    </div>
  `;
}

export async function mountEditor(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  const shift = page.querySelector<HTMLSelectElement>("#ed-shift")!;
  for (const z of SHIFTS) {
    const o = document.createElement("option");
    o.value = String(z.hours);
    o.textContent = z.label;
    shift.appendChild(o);
  }

  let group = "";
  let selected: Managed | null = null;

  const reload = async () => {
    const groups = await invoke<{ title: string; count: number }[]>("list_managed_groups");
    const gEl = page.querySelector("#ed-groups")!;
    gEl.innerHTML = "";
    const dl = page.querySelector("#ed-group-list")!;
    dl.innerHTML = "";
    let total = 0;
    for (const g of groups) {
      total += g.count;
      const b = document.createElement("button");
      b.className = "group-row" + (g.title === group ? " active" : "");
      b.textContent = `${g.title}  (${g.count})`;
      b.addEventListener("click", () => {
        group = g.title;
        void loadChannels();
      });
      b.addEventListener("contextmenu", (ev) => {
        ev.preventDefault();
        const name = window.prompt("Rename group", g.title);
        if (name == null) return;
        void invoke("rename_managed_group", { oldName: g.title, newName: name }).then(reload);
      });
      b.addEventListener("dblclick", () => b.dispatchEvent(new Event("contextmenu")));
      gEl.appendChild(b);
      const opt = document.createElement("option");
      opt.value = g.title;
      dl.appendChild(opt);
    }
    page.querySelector("#ed-count")!.textContent = `${total} channels`;
    if (!group && groups[0]) group = groups[0].title;
    await loadChannels();
  };

  const loadChannels = async () => {
    const list = page.querySelector("#ed-channels")!;
    list.innerHTML = "";
    if (!group) return;
    const chans = await invoke<Managed[]>("list_managed", { group });
    for (const c of chans) {
      const row = document.createElement("button");
      row.className = "chan-pick" + (selected?.id === c.id ? " active" : "");
      row.innerHTML = `
        <span class="logo-slot">${c.hasEpgMatch ? `<span class="tvg-check">✓</span>` : ""}</span>
        <span>
          <span class="chan-name">${esc(c.name)}</span>
          <span class="chan-sub">${esc(c.tvgId ?? "")}</span>
        </span>`;
      row.addEventListener("click", () => void select(c.id));
      list.appendChild(row);
    }
  };

  const select = async (id: string) => {
    selected = (await invoke<Managed | null>("get_managed", { id })) ?? null;
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
    const tvg = (page.querySelector("#ed-tvg") as HTMLInputElement).value.trim();
    const check = page.querySelector<HTMLElement>("#ed-tvg-check")!;
    const input = page.querySelector<HTMLInputElement>("#ed-tvg")!;
    const known = tvg ? await invoke<boolean>("is_known_tvg", { tvgId: tvg }) : false;
    check.hidden = !known;
    input.classList.toggle("tvg-ok", known);
    const now = page.querySelector<HTMLElement>("#ed-now")!;
    if (!known) {
      now.hidden = true;
      return;
    }
    const hours = Number(shift.value);
    const np = await invoke<{ title: string; startLocal: string; stopLocal: string } | null>("now_playing", {
      tvgId: tvg,
      shiftHours: hours,
    });
    if (!np) {
      now.hidden = false;
      page.querySelector("#ed-now-title")!.textContent = "No programme at this time";
      page.querySelector("#ed-now-times")!.textContent = "";
    } else {
      now.hidden = false;
      page.querySelector("#ed-now-title")!.textContent = np.title;
      page.querySelector("#ed-now-times")!.textContent = `${np.startLocal} – ${np.stopLocal}`;
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
        <button data-act="play" data-id="${v.id}">Play</button>
        <div>
          <div>${esc(v.label || (v.visibility === "visible" ? "visible" : "backup"))}</div>
          <div class="chan-sub">${esc(v.url)}</div>
        </div>
        <button data-act="info" data-id="${v.id}">Info</button>
        <button data-act="up" data-id="${v.id}">↑</button>
        <button data-act="down" data-id="${v.id}">↓</button>
        <button data-act="rm" data-id="${v.id}">Remove</button>
      `;
      box.appendChild(row);
    }
  };

  const paintLogo = () => {
    const url = (page.querySelector("#ed-logo") as HTMLInputElement).value.trim();
    const slot = page.querySelector("#ed-logo-preview")!;
    if (!url) {
      slot.innerHTML = `<span class="broken">broken logo</span>`;
      return;
    }
    slot.innerHTML = `<img src="${esc(url)}" alt="" />`;
    const img = slot.querySelector("img");
    img?.addEventListener("error", () => {
      slot.innerHTML = `<span class="broken">broken logo</span>`;
    });
  };

  const gather = (): Managed | null => {
    if (!selected) return null;
    return {
      ...selected,
      name: (page.querySelector("#ed-name") as HTMLInputElement).value,
      groupTitle: (page.querySelector("#ed-group") as HTMLInputElement).value || "Unassigned",
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
      await invoke("save_managed", { channel: ch });
      toast("Saved.");
      selected = await invoke("get_managed", { id: ch.id });
      group = selected?.groupTitle ?? group;
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });

  page.querySelector("#ed-refresh")!.addEventListener("click", () => void reload());
  page.querySelector("#ed-load")!.addEventListener("click", async () => {
    try {
      const msg = await invoke<string>("import_curated", { replace: true });
      if (msg !== "cancelled") toast(msg);
      group = "";
      selected = null;
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#ed-export")!.addEventListener("click", async () => {
    try {
      const msg = await invoke<string>("export_managed", { includeBackups: false });
      if (msg !== "cancelled") toast(msg);
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#ed-clear")!.addEventListener("click", async () => {
    if (!window.confirm("Remove all channels from the managed playlist?")) return;
    try {
      await invoke("clear_managed");
      group = "";
      selected = null;
      toast("Cleared.");
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#ed-logo")!.addEventListener("input", paintLogo);
  shift.addEventListener("change", () => void updateMatch());

  const tvg = page.querySelector<HTMLInputElement>("#ed-tvg")!;
  const sug = page.querySelector<HTMLElement>("#ed-suggest")!;
  let t = 0;
  tvg.addEventListener("input", () => {
    window.clearTimeout(t);
    t = window.setTimeout(async () => {
      await updateMatch();
      const q = tvg.value.trim();
      if (!q) {
        sug.hidden = true;
        return;
      }
      const hits = await invoke<{ tvgId: string; name: string; line: string }[]>("suggest_tvg", { query: q });
      sug.innerHTML = "";
      for (const h of hits) {
        const b = document.createElement("button");
        b.className = "suggest-item";
        b.textContent = h.line;
        b.addEventListener("click", () => {
          tvg.value = h.tvgId;
          sug.hidden = true;
          const name = page.querySelector<HTMLInputElement>("#ed-name")!;
          if (!name.value) name.value = h.name;
          void updateMatch();
        });
        sug.appendChild(b);
      }
      sug.hidden = hits.length === 0;
    }, 120);
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
        const v = selected.variants.find((x) => x.id === id);
        if (v) await api.playUrl(v.url);
      } else if (act === "info") {
        const v = selected.variants.find((x) => x.id === id);
        toast(`${v?.originName ?? "—"} / ${v?.originTvgId ?? "—"}`);
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

  page.querySelector("#ed-add-from")!.addEventListener("click", () => {
    page.querySelector("#ed-src-dlg")!.classList.add("open");
    void fillFromSources("");
  });
  page.querySelector("#ed-src-close")!.addEventListener("click", () => {
    page.querySelector("#ed-src-dlg")!.classList.remove("open");
  });
  page.querySelector("#ed-src-filter")!.addEventListener("input", (ev) => {
    void fillFromSources((ev.target as HTMLInputElement).value);
  });

  async function fillFromSources(q: string) {
    const list = page.querySelector("#ed-src-list")!;
    list.innerHTML = "";
    const hits = q.trim().length >= 2 ? await api.searchSources(q) : [];
    for (const c of hits) {
      const row = document.createElement("button");
      row.className = "group-row";
      row.textContent = `${c.name}  ·  ${c.tvgId ?? ""}`;
      row.addEventListener("click", async () => {
        try {
          const ch = await invoke<Managed>("add_from_source", { entryId: c.id });
          toast(`Added ${ch.name} to Unassigned.`);
          group = ch.groupTitle;
          await reload();
          await select(ch.id);
        } catch (e) {
          toast(String(e));
        }
      });
      list.appendChild(row);
    }
  }

  try {
    await reload();
  } catch (e) {
    toast(String(e));
  }
}

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}
