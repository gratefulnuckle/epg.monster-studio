import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { bindVirtualList, type VirtualList } from "./virtual";

export type AuditRow = {
  managedChannelId: string;
  channelName: string;
  groupTitle: string;
  currentTvgId?: string | null;
  status: string;
  suggestedTvgId?: string | null;
  suggestedName?: string | null;
  suggestedLogo?: string | null;
  score: number;
  secondScore: number;
  matchKind?: string | null;
};

export type CatalogEntry = {
  tvgId: string;
  name: string;
  logo?: string | null;
  section: string;
};

const LEVELS: { score: number; label: string }[] = [
  { score: 0.98, label: "Strict — exact normalized name (0.98)" },
  { score: 0.9, label: "High — unique suggestion ≥ 0.90" },
  { score: 0.85, label: "Recommended — unique suggestion ≥ 0.85" },
  { score: 0.75, label: "Broad — unique suggestion ≥ 0.75" },
];

export function epgHtml(): string {
  return `
    <div class="editor-toolbar">
      <span class="editor-title">EPG Audit</span>
      <button class="accent" id="epg-fetch" title="Download epg.monster XMLTV and rebuild the tvg-id catalog">Fetch / refresh catalog</button>
      <button id="epg-browse" title="Replace the audit panes with a full searchable tvg-id catalog">Browse catalog</button>
      <button id="epg-reindex" title="Re-index now playing from the cached XMLTV (no download)">Rebuild now playing</button>
      <button id="epg-refresh">Refresh</button>
      <button id="epg-auto" title="Pick groups and a score level, then auto-match unique suggestions">Apply suggestion</button>
      <span class="page-sub" id="epg-count"></span>
    </div>
    <div class="field" style="max-width:720px">
      <label>XMLTV URL (epg.monster)</label>
      <input id="epg-url" />
    </div>
    <p class="page-sub">1) Pick group  2) Pick channel  3) Type tvg-id for live suggestions  4) Apply. Matched channels hidden unless checked.</p>
    <div id="epg-auditor" class="editor-grid">
      <section class="tile editor-pane"><h2>Groups</h2><div id="epg-groups" class="editor-list"></div></section>
      <section class="tile editor-pane">
        <h2>Channels <label class="check" style="display:inline-flex;font-weight:400"><input type="checkbox" id="epg-show-matched" /> Show matched</label></h2>
        <div id="epg-channels" class="editor-list"></div>
      </section>
      <section class="tile editor-pane" id="epg-detail">
        <h2>EPG suggestions</h2>
        <p class="page-sub" id="epg-detail-empty">Select a channel.</p>
        <div id="epg-detail-body" hidden>
          <div id="epg-detail-name" class="chan-name"></div>
          <div id="epg-detail-status" class="chan-sub"></div>
          <div id="epg-detail-score" class="chan-sub"></div>
          <div class="field">
            <label>tvg-id (type for catalog suggestions)</label>
            <div class="tvg-row">
              <input id="epg-tvg" placeholder="Start typing a channel id or name…" />
              <span id="epg-tvg-check" class="tvg-check" hidden>✓</span>
            </div>
            <div id="epg-suggest" class="suggest" hidden></div>
          </div>
          <div class="chan-sub">Best suggestion</div>
          <div id="epg-best"></div>
          <div>
            <button class="accent" id="epg-apply">Apply suggestion</button>
            <button id="epg-images" title="Google Images (transparent)">Search images</button>
          </div>
          <div class="chan-sub" style="margin-top:10px">More catalog matches</div>
          <div id="epg-more" class="editor-list"></div>
          <p class="page-sub">Suggestions + now-playing use the catalog built from epg.monster XMLTV channel ids.</p>
        </div>
      </section>
    </div>
    <div id="epg-browser" hidden>
      <h2>Full tvg-id catalog</h2>
      <p class="page-sub">Search every tvg-id parsed from the XMLTV guide. Select a row to apply it to the channel you had selected.</p>
      <button id="epg-browse-back">Back to EPG audit</button>
      <input id="epg-browse-q" placeholder="Filter catalog by id or name…" />
      <div id="epg-browse-list" class="editor-list" style="max-height:60vh"></div>
    </div>
    <div class="dialog-backdrop" id="epg-auto-dlg">
      <div class="dialog" style="width:520px">
        <h2>Apply suggestion</h2>
        <p class="page-sub">Groups with unmatched or unknown tvg-ids. Pick a score level, select groups, then auto match.</p>
        <div class="field"><label>Approved score level</label>
          <select id="epg-score"></select></div>
        <div id="epg-group-picks" class="editor-list" style="max-height:240px"></div>
        <p class="page-sub" id="epg-auto-preview"></p>
        <div class="dialog-actions">
          <button id="epg-auto-cancel">Cancel</button>
          <button class="accent" id="epg-auto-go">Auto match</button>
        </div>
      </div>
    </div>
  `;
}

export async function mountEpg(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  const scoreSel = page.querySelector<HTMLSelectElement>("#epg-score")!;
  for (const l of LEVELS) {
    const o = document.createElement("option");
    o.value = String(l.score);
    o.textContent = l.label;
    scoreSel.appendChild(o);
  }
  scoreSel.selectedIndex = 2;

  let rows: AuditRow[] = [];
  let group = "";
  let selected: AuditRow | null = null;
  let showMatched = false;
  let groupVirt: VirtualList<string> | null = null;
  let chanVirt: VirtualList<AuditRow> | null = null;

  try {
    const url = await invoke<string>("epg_guide_url");
    (page.querySelector("#epg-url") as HTMLInputElement).value = url;
  } catch { /* ignore */ }

  const statusLabel = (s: string) =>
    s === "matched" ? "Matched" : s === "unknown" ? "Unknown ID" : "Missing ID";

  const reload = async () => {
    rows = await invoke<AuditRow[]>("epg_audit");
    const n = await invoke<number>("epg_catalog_count");
    const count = page.querySelector("#epg-count");
    if (!count) return;
    count.textContent = `${n} catalog ids`;
    paintGroups();
  };

  const issuesIn = (g: string) =>
    rows.filter((r) => r.groupTitle === g && r.status !== "matched").length;

  const paintGroups = () => {
    const titles = [...new Set(rows.map((r) => r.groupTitle))];
    const el = page.querySelector<HTMLElement>("#epg-groups");
    if (!el) return;
    if (!group && titles[0]) group = titles[0];
    groupVirt?.destroy();
    el.innerHTML = "";
    groupVirt = bindVirtualList({
      scroller: el,
      rowHeight: 36,
      renderRow: (t) => {
        const issues = issuesIn(t);
        const b = document.createElement("button");
        b.className = "group-row" + (t === group ? " active" : "");
        b.innerHTML = `${esc(t)}${issues ? `<span class="issue-n"> ${issues} issues</span>` : ""}`;
        b.addEventListener("click", () => {
          group = t;
          selected = null;
          paintGroups();
          paintDetail();
        });
        return b;
      },
    });
    groupVirt.setItems(titles);
    paintChannels();
  };

  const paintChannels = () => {
    const el = page.querySelector<HTMLElement>("#epg-channels");
    if (!el) return;
    const list = rows.filter((r) => r.groupTitle === group && (showMatched || r.status !== "matched"));
    if (!chanVirt) {
      chanVirt = bindVirtualList({
        scroller: el,
        rowHeight: 48,
        renderRow: (r) => {
          const b = document.createElement("button");
          b.className = "chan-pick" + (selected?.managedChannelId === r.managedChannelId ? " active" : "");
          b.innerHTML = `<span><span class="chan-name">${esc(r.channelName)}</span>
        <span class="chan-sub">${esc(r.currentTvgId ?? "—")}</span></span>
        <span class="status-pill">${statusLabel(r.status)}</span>`;
          b.addEventListener("click", () => {
            selected = r;
            paintChannels();
            paintDetail();
          });
          return b;
        },
      });
    }
    chanVirt.setItems(list);
  };

  const paintDetail = () => {
    const empty = page.querySelector<HTMLElement>("#epg-detail-empty")!;
    const body = page.querySelector<HTMLElement>("#epg-detail-body")!;
    if (!selected) {
      empty.hidden = false;
      body.hidden = true;
      return;
    }
    empty.hidden = true;
    body.hidden = false;
    page.querySelector("#epg-detail-name")!.textContent = selected.channelName;
    page.querySelector("#epg-detail-status")!.textContent = statusLabel(selected.status);
    page.querySelector("#epg-detail-score")!.textContent = selected.suggestedTvgId
      ? `Suggestion score: ${selected.score.toFixed(2)} (${selected.matchKind ?? "fuzzy"})`
      : "Suggestion score: —";
    const tvg = page.querySelector<HTMLInputElement>("#epg-tvg")!;
    tvg.value = selected.suggestedTvgId || selected.currentTvgId || "";
    page.querySelector("#epg-best")!.textContent = selected.suggestedTvgId
      ? `${selected.suggestedTvgId}  —  ${selected.suggestedName ?? ""}  (score ${selected.score.toFixed(2)})`
      : "(no suggestion)";
    void updateKnown();
  };

  const updateKnown = async () => {
    const tvg = (page.querySelector("#epg-tvg") as HTMLInputElement).value.trim();
    const known = tvg ? await invoke<boolean>("is_known_tvg", { tvgId: tvg }) : false;
    page.querySelector<HTMLElement>("#epg-tvg-check")!.hidden = !known;
    page.querySelector("#epg-tvg")!.classList.toggle("tvg-ok", known);
  };

  page.querySelector("#epg-show-matched")!.addEventListener("change", (ev) => {
    showMatched = (ev.target as HTMLInputElement).checked;
    paintChannels();
  });

  page.querySelector("#epg-fetch")!.addEventListener("click", async () => {
    const url = (page.querySelector("#epg-url") as HTMLInputElement).value.trim();
    toast("Fetching XMLTV…");
    try {
      const msg = await invoke<string>("fetch_epg_catalog", { url: url || null });
      toast(msg);
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#epg-refresh")!.addEventListener("click", () => void reload());
  page.querySelector("#epg-reindex")!.addEventListener("click", async () => {
    try {
      toast(await invoke<string>("rebuild_now_playing"));
    } catch (e) {
      toast(String(e));
    }
  });

  page.querySelector("#epg-apply")!.addEventListener("click", async () => {
    if (!selected) return;
    const tvg = (page.querySelector("#epg-tvg") as HTMLInputElement).value.trim();
    if (!tvg) return;
    try {
      await invoke("epg_apply", {
        managedId: selected.managedChannelId,
        tvgId: tvg,
        logo: selected.suggestedLogo,
        applyLogo: false,
      });
      toast(`Applied ${tvg}`);
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });

  page.querySelector("#epg-images")!.addEventListener("click", async () => {
    if (!selected) return;
    const url = await invoke<string>("epg_search_images_url", { name: selected.channelName });
    await openUrl(url);
  });

  const sug = page.querySelector<HTMLElement>("#epg-suggest")!;
  const tvgBox = page.querySelector<HTMLInputElement>("#epg-tvg")!;
  let t = 0;
  tvgBox.addEventListener("input", () => {
    window.clearTimeout(t);
    t = window.setTimeout(async () => {
      await updateKnown();
      const q = tvgBox.value.trim();
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
          tvgBox.value = h.tvgId;
          sug.hidden = true;
          void updateKnown();
        });
        sug.appendChild(b);
      }
      sug.hidden = hits.length === 0;
    }, 120);
  });

  const dlg = page.querySelector("#epg-auto-dlg")!;
  page.querySelector("#epg-auto")!.addEventListener("click", () => {
    const issueRows = rows.filter((r) => r.status !== "matched");
    if (issueRows.length === 0) {
      toast("No groups with EPG issues");
      return;
    }
    const picks = page.querySelector("#epg-group-picks")!;
    picks.innerHTML = "";
    const groups = [...new Set(issueRows.map((r) => r.groupTitle))];
    for (const g of groups) {
      const lab = document.createElement("label");
      lab.className = "check";
      lab.innerHTML = `<input type="checkbox" checked data-g="${esc(g)}" /> ${esc(g)}`;
      picks.appendChild(lab);
    }
    dlg.classList.add("open");
    refreshPreview();
  });
  scoreSel.addEventListener("change", refreshPreview);
  page.querySelector("#epg-group-picks")!.addEventListener("change", refreshPreview);

  function refreshPreview() {
    const min = Number(scoreSel.value);
    const selectedGroups = [...page.querySelectorAll<HTMLInputElement>("#epg-group-picks input:checked")].map(
      (i) => i.dataset.g!,
    );
    const n = rows.filter(
      (r) =>
        r.status !== "matched" &&
        selectedGroups.includes(r.groupTitle) &&
        r.suggestedTvgId &&
        r.score + 0.0001 >= min &&
        (r.score >= 0.98 || r.score - (r.secondScore ?? 0) >= 0.1) &&
        !String(r.suggestedTvgId).toLowerCase().includes("dummy"),
    ).length;
    page.querySelector("#epg-auto-preview")!.textContent =
      `${n} unique suggestion(s) ready at this score in the selected groups. Logos are not changed.`;
  }

  page.querySelector("#epg-auto-cancel")!.addEventListener("click", () => dlg.classList.remove("open"));
  page.querySelector("#epg-auto-go")!.addEventListener("click", async () => {
    const groups = [...page.querySelectorAll<HTMLInputElement>("#epg-group-picks input:checked")].map(
      (i) => i.dataset.g!,
    );
    try {
      const n = await invoke<number>("epg_auto_match", { groups, minScore: Number(scoreSel.value) });
      dlg.classList.remove("open");
      toast(n ? `Applied ${n}` : "No unique suggestions met that score in the selected groups");
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });

  const auditor = page.querySelector<HTMLElement>("#epg-auditor")!;
  const browser = page.querySelector<HTMLElement>("#epg-browser")!;
  page.querySelector("#epg-browse")!.addEventListener("click", async () => {
    auditor.hidden = true;
    browser.hidden = false;
    const catalog = await invoke<CatalogEntry[]>("epg_browse_catalog");
    paintBrowse(catalog, "");
    page.querySelector("#epg-browse-q")!.addEventListener("input", (ev) => {
      paintBrowse(catalog, (ev.target as HTMLInputElement).value);
    });
  });
  page.querySelector("#epg-browse-back")!.addEventListener("click", () => {
    browser.hidden = true;
    auditor.hidden = false;
  });

  function paintBrowse(catalog: CatalogEntry[], q: string) {
    const el = page.querySelector("#epg-browse-list")!;
    el.innerHTML = "";
    const ql = q.trim().toLowerCase();
    let last = "";
    for (const c of catalog) {
      if (ql && !c.tvgId.toLowerCase().includes(ql) && !c.name.toLowerCase().includes(ql)) continue;
      const sec = sectionDisplay(c.section);
      if (sec !== last) {
        const h = document.createElement("div");
        h.className = "issue-n";
        h.style.padding = "8px 4px";
        h.textContent = sec;
        el.appendChild(h);
        last = sec;
      }
      const row = document.createElement("div");
      row.className = "chan-sub";
      row.style.padding = "4px";
      row.textContent = `${c.tvgId}  —  ${c.name}`;
      el.appendChild(row);
    }
  }

  try {
    await reload();
  } catch (e) {
    toast(String(e));
  }
}

function sectionDisplay(section: string): string {
  if (!section.trim()) return "Other";
  return section.replace(/\d+$/, "") || section;
}

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}
