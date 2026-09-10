import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { bindVirtualList, type VirtualList } from "./virtual";
import { bindThreeColSplit } from "./split";
import { fillTvgShiftSelect } from "./tvg-shift";
import { keepGroup, nextIssueId } from "./issue-nav";
import { notifyPhysicalSave } from "./save-status";

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
    <h1 class="page-title">EPG Audit</h1>
    <p class="page-sub">1) Pick group  2) Pick channel  3) Type tvg-id for live suggestions  4) Apply. Matched channels hidden unless checked.</p>
    <div class="editor-workspace">
    <div class="tabs-row">
      <button class="accent" id="epg-fetch" title="Download epg.monster XMLTV and rebuild the tvg-id catalog">Fetch / refresh catalog</button>
      <button id="epg-browse" title="Open the full searchable tvg-id catalog in a new window">Browse catalog</button>
      <button id="epg-reindex" title="Re-index now playing from the cached XMLTV (no download)">Rebuild now playing</button>
      <button id="epg-refresh">Refresh</button>
      <button id="epg-auto" title="Pick groups and a score level, then auto-match unique suggestions">Apply suggestions</button>
      <span class="page-sub" id="epg-count">Loading…</span>
    </div>
    <div class="field" style="max-width:720px">
      <label>XMLTV URL (epg.monster)</label>
      <input id="epg-url" />
    </div>
    <div id="epg-auditor" class="editor-grid editor-split">
      <section class="groups editor-pane">
        <div class="groups-head">Groups</div>
        <div id="epg-groups" class="groups-body"></div>
      </section>
      <div class="split-handle" id="epg-split-groups" title="Drag to resize groups"></div>
      <section class="channels editor-pane">
        <div class="groups-head">Channels <label class="check" style="display:inline-flex;font-weight:400;margin-left:8px"><input type="checkbox" id="epg-show-matched" /> Show matched</label></div>
        <div id="epg-channels" class="editor-list"></div>
      </section>
      <div class="split-handle" id="epg-split-chans" title="Drag to resize channels"></div>
      <section class="tile editor-pane" id="epg-detail">
        <div class="groups-head">EPG suggestions</div>
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
    <div class="dialog-backdrop" id="epg-auto-dlg">
      <div class="dialog" style="width:520px">
        <h2>Apply suggestions</h2>
        <p class="page-sub">Groups with unmatched or unknown tvg-ids. Auto Match applies the first suggestion. Scrolled User Approved asks Yes or No on each channel.</p>
        <div class="field"><label>Approved score level</label>
          <select id="epg-score"></select></div>
        <div class="field"><label>EPG timeshift (tvg-shift) — e.g. Vancouver / Pacific for a delayed feed</label>
          <select id="epg-shift"></select></div>
        <div id="epg-group-picks" class="editor-list" style="max-height:240px"></div>
        <p class="page-sub" id="epg-auto-preview"></p>
        <div class="dialog-actions">
          <button id="epg-auto-cancel">Cancel</button>
          <button id="epg-review-go">Scrolled User Approved</button>
          <button class="accent" id="epg-auto-go">Auto Match</button>
        </div>
      </div>
    </div>
    <div class="dialog-backdrop" id="epg-review-dlg">
      <div class="dialog epg-review-dialog">
        <h2>Approve match?</h2>
        <p class="page-sub" id="epg-review-progress"></p>
        <div class="epg-review-scroll" id="epg-review-scroll">
          <div class="epg-review-card" id="epg-review-card">
            <div class="chan-name" id="epg-review-name"></div>
            <div class="chan-sub" id="epg-review-cur"></div>
            <div class="chan-sub" id="epg-review-best"></div>
            <div class="field"><label>EPG timeshift / tvg-shift</label>
              <select id="epg-review-shift"></select></div>
            <div id="epg-review-hits" class="editor-list"></div>
          </div>
        </div>
        <div class="dialog-actions">
          <button id="epg-review-stop">Stop</button>
          <button id="epg-review-no">No</button>
          <button class="accent" id="epg-review-yes">Yes</button>
        </div>
      </div>
    </div>
    </div>
  `;
}

export async function mountEpg(page: HTMLElement, toast: (s: string) => void): Promise<() => void> {
  const split = page.querySelector<HTMLElement>("#epg-auditor");
  const splitGroups = page.querySelector<HTMLElement>("#epg-split-groups");
  const splitChans = page.querySelector<HTMLElement>("#epg-split-chans");
  if (split && splitGroups && splitChans) {
    bindThreeColSplit(split, splitGroups, splitChans, "studio-epg");
  }

  const scoreSel = page.querySelector<HTMLSelectElement>("#epg-score")!;
  for (const l of LEVELS) {
    const o = document.createElement("option");
    o.value = String(l.score);
    o.textContent = l.label;
    scoreSel.appendChild(o);
  }
  scoreSel.selectedIndex = 2;
  const applyShiftSel = page.querySelector<HTMLSelectElement>("#epg-shift")!;
  fillTvgShiftSelect(applyShiftSel, 0);
  const reviewShiftSel = page.querySelector<HTMLSelectElement>("#epg-review-shift")!;
  fillTvgShiftSelect(reviewShiftSel, 0);

  let rows: AuditRow[] = [];
  let group = "";
  let selected: AuditRow | null = null;
  let showMatched = false;
  let groupVirt: VirtualList<string> | null = null;
  let chanVirt: VirtualList<AuditRow> | null = null;

  const statusLabel = (s: string) =>
    s === "matched" ? "Matched" : s === "unknown" ? "Unknown ID" : "Missing ID";

  const paintCount = () => {
    const count = page.querySelector("#epg-count");
    if (!count) return;
    const issues = rows.filter((r) => r.status !== "matched").length;
    count.textContent = `${issues} issues`;
  };

  const visibleIdsIn = (g: string) =>
    rows
      .filter((r) => r.groupTitle === g && (showMatched || r.status !== "matched"))
      .map((r) => r.managedChannelId);

  const reload = async (keep?: { group: string; prevIds: string[]; appliedId: string }) => {
    const count = page.querySelector("#epg-count");
    if (count) count.textContent = "Loading…";
    rows = await invoke<AuditRow[]>("epg_audit");
    paintCount();
    if (keep) {
      group = keep.group;
      if (showMatched) {
        selected = rows.find((r) => r.managedChannelId === keep.appliedId) ?? null;
      } else {
        const nid = nextIssueId(keep.prevIds, keep.appliedId);
        selected = nid ? (rows.find((r) => r.managedChannelId === nid) ?? null) : null;
      }
      if (!selected && group) {
        selected =
          rows.find((r) => r.groupTitle === group && (showMatched || r.status !== "matched")) ?? null;
      }
    }
    paintGroups();
  };

  const issuesIn = (g: string) =>
    rows.filter((r) => r.groupTitle === g && r.status !== "matched").length;

  const paintGroups = () => {
    const titles = [...new Set(rows.map((r) => r.groupTitle))].filter(
      (t) => showMatched || issuesIn(t) > 0,
    );
    const el = page.querySelector<HTMLElement>("#epg-groups");
    if (!el) return;
    group = keepGroup(titles, group);
    if (selected && selected.groupTitle !== group) selected = null;
    if (selected) {
      selected = rows.find((r) => r.managedChannelId === selected!.managedChannelId) ?? selected;
    }
    if (!groupVirt) {
      groupVirt = bindVirtualList({
        scroller: el,
        rowHeight: 36,
        renderRow: (t) => {
          const issues = issuesIn(t);
          const b = document.createElement("button");
          b.className = "group-row" + (t === group ? " active" : "");
          b.innerHTML = `${esc(t)}<span class="issue-n"> ${issues} issues</span>`;
          b.addEventListener("click", () => {
            group = t;
            selected = null;
            const chans = page.querySelector<HTMLElement>("#epg-channels");
            if (chans) chans.scrollTop = 0;
            paintGroups();
            paintDetail();
          });
          return b;
        },
      });
    }
    groupVirt.setItems(titles);
    paintChannels();
    paintDetail();
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
          const st = statusLabel(r.status);
          const stHtml =
            r.status === "unknown"
              ? `<span class="issue-n">${esc(st)}</span>`
              : r.status === "matched"
                ? ""
                : `<span class="status-pill">${esc(st)}</span>`;
          b.innerHTML = `<span><span class="chan-name">${esc(r.channelName)}</span>
        <span class="chan-sub">${esc(r.currentTvgId ?? "—")}</span></span>
        ${stHtml}`;
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
    const empty = page.querySelector<HTMLElement>("#epg-detail-empty");
    const body = page.querySelector<HTMLElement>("#epg-detail-body");
    if (!empty || !body) return;
    if (!selected) {
      empty.hidden = false;
      body.hidden = true;
      return;
    }
    empty.hidden = true;
    body.hidden = false;
    const nameEl = page.querySelector("#epg-detail-name");
    if (!nameEl) return;
    nameEl.textContent = selected.channelName;
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
    void fillMoreMatches();
  };

  const fillMoreMatches = async () => {
    const more = page.querySelector("#epg-more");
    const box = page.querySelector<HTMLInputElement>("#epg-tvg");
    if (!more || !box || !selected) return;
    const q = (selected.channelName || box.value).trim();
    if (!q) {
      more.innerHTML = "";
      return;
    }
    try {
      const hits = await invoke<{ tvgId: string; name: string; line: string }[]>("suggest_tvg", { query: q });
      if (!page.querySelector("#epg-more")) return;
      const extra = hits.filter((h) => h.tvgId !== box.value.trim()).slice(0, 8);
      more.innerHTML = extra
        .map((h) => `<button type="button" class="suggest-item" data-tvg="${esc(h.tvgId)}">${esc(h.line)}</button>`)
        .join("");
      more.querySelectorAll<HTMLButtonElement>("button[data-tvg]").forEach((b) => {
        b.addEventListener("click", () => {
          box.value = b.dataset.tvg ?? "";
          void updateKnown();
        });
      });
    } catch {
      /* leave empty */
    }
  };

  const updateKnown = async () => {
    const box = page.querySelector<HTMLInputElement>("#epg-tvg");
    if (!box) return;
    const tvg = box.value.trim();
    const known = tvg ? await invoke<boolean>("is_known_tvg", { tvgId: tvg }) : false;
    const check = page.querySelector<HTMLElement>("#epg-tvg-check");
    const input = page.querySelector("#epg-tvg");
    if (!check || !input) return;
    check.hidden = !known;
    input.classList.toggle("tvg-ok", known);
  };

  page.querySelector("#epg-show-matched")!.addEventListener("change", (ev) => {
    showMatched = (ev.target as HTMLInputElement).checked;
    paintGroups();
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
  page.querySelector("#epg-refresh")!.addEventListener("click", () =>
    void reload().catch((e) => toast(String(e))),
  );
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
    const keep = {
      group,
      prevIds: visibleIdsIn(group),
      appliedId: selected.managedChannelId,
    };
    try {
      await invoke("epg_apply", {
        managedId: selected.managedChannelId,
        tvgId: tvg,
        logo: selected.suggestedLogo,
        applyLogo: false,
      });
      notifyPhysicalSave();
      toast(`Applied ${tvg}`);
      await reload(keep);
    } catch (e) {
      toast(String(e));
    }
  });

  page.querySelector("#epg-images")!.addEventListener("click", async () => {
    if (!selected) return;
    try {
      const url = await invoke<string>("epg_search_images_url", { name: selected.channelName });
      await openUrl(url);
    } catch (e) {
      toast(String(e));
    }
  });

  const sug = page.querySelector<HTMLElement>("#epg-suggest")!;
  const tvgBox = page.querySelector<HTMLInputElement>("#epg-tvg")!;
  let t = 0;
  tvgBox.addEventListener("input", () => {
    window.clearTimeout(t);
    t = window.setTimeout(async () => {
      await updateKnown();
      if (!page.querySelector("#epg-tvg")) return;
      const q = tvgBox.value.trim();
      if (!q) {
        sug.hidden = true;
        return;
      }
      let hits: { tvgId: string; name: string; line: string }[] = [];
      try {
        hits = await invoke("suggest_tvg", { query: q });
      } catch (e) {
        toast(String(e));
        return;
      }
      if (!page.querySelector("#epg-suggest")) return;
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
      const more = page.querySelector("#epg-more");
      if (more) {
        const extra = hits.filter((h) => h.tvgId !== tvgBox.value.trim()).slice(0, 8);
        more.innerHTML = extra
          .map((h) => `<button type="button" class="suggest-item" data-tvg="${esc(h.tvgId)}">${esc(h.line)}</button>`)
          .join("");
        more.querySelectorAll<HTMLButtonElement>("button[data-tvg]").forEach((b) => {
          b.addEventListener("click", () => {
            tvgBox.value = b.dataset.tvg ?? "";
            sug.hidden = true;
            void updateKnown();
          });
        });
      }
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
        !String(r.suggestedTvgId).toLowerCase().includes("dummy"),
    ).length;
    page.querySelector("#epg-auto-preview")!.textContent =
      `${n} suggestion(s) at this score in the selected groups. Logos are not changed.`;
  }

  const pickedGroups = () =>
    [...page.querySelectorAll<HTMLInputElement>("#epg-group-picks input:checked")].map((i) => i.dataset.g!);

  page.querySelector("#epg-auto-cancel")!.addEventListener("click", () => dlg.classList.remove("open"));
  page.querySelector("#epg-auto-go")!.addEventListener("click", async () => {
    const groups = pickedGroups();
    try {
      const n = await invoke<number>("epg_auto_match", {
        groups,
        minScore: Number(scoreSel.value),
        requireUnique: false,
        tvgShiftHours: Number(applyShiftSel.value),
      });
      dlg.classList.remove("open");
      toast(n ? `Applied ${n}` : "No suggestions met that score in the selected groups");
      await reload();
    } catch (e) {
      toast(String(e));
    }
  });

  type SuggestHit = { tvgId: string; name: string; line?: string };
  let reviewQueue: AuditRow[] = [];
  let reviewAt = 0;
  let reviewHits: SuggestHit[] = [];
  let reviewPick = 0;
  const reviewDlg = page.querySelector("#epg-review-dlg")!;

  const finishReview = async (msg: string) => {
    reviewDlg.classList.remove("open");
    toast(msg);
    await reload();
  };

  const paintReviewCard = async () => {
    const r = reviewQueue[reviewAt];
    if (!r) {
      await finishReview("Review finished");
      return;
    }
    const prog = page.querySelector("#epg-review-progress");
    if (prog) prog.textContent = `${reviewAt + 1} of ${reviewQueue.length}`;
    const nameEl = page.querySelector("#epg-review-name");
    if (nameEl) nameEl.textContent = r.channelName;
    const cur = page.querySelector("#epg-review-cur");
    if (cur) cur.textContent = `Current: ${r.currentTvgId || "—"}`;
    const best = page.querySelector("#epg-review-best");
    if (best) {
      best.textContent = r.suggestedTvgId
        ? `First match: ${r.suggestedTvgId}  —  ${r.suggestedName ?? ""}  (${r.score.toFixed(2)})`
        : "No first match";
    }
    reviewHits = [];
    reviewPick = 0;
    if (r.suggestedTvgId) {
      reviewHits.push({ tvgId: r.suggestedTvgId, name: r.suggestedName ?? "" });
    }
    try {
      const extra = await invoke<SuggestHit[]>("suggest_tvg", { query: r.channelName });
      for (const h of extra) {
        if (!reviewHits.some((x) => x.tvgId.toLowerCase() === h.tvgId.toLowerCase())) {
          reviewHits.push(h);
        }
      }
    } catch {
      /* keep first match */
    }
    reviewHits = reviewHits.slice(0, 6);
    fillTvgShiftSelect(reviewShiftSel, Number(applyShiftSel.value) || 0);
    const box = page.querySelector("#epg-review-hits");
    if (box) {
      box.innerHTML = "";
      reviewHits.forEach((h, i) => {
        const b = document.createElement("button");
        b.type = "button";
        b.className = "suggest-item" + (i === 0 ? " active" : "");
        b.textContent = `${h.tvgId}  —  ${h.name}`;
        b.addEventListener("click", () => {
          reviewPick = i;
          box.querySelectorAll(".suggest-item").forEach((el) => el.classList.remove("active"));
          b.classList.add("active");
        });
        box.appendChild(b);
      });
    }
    page.querySelector("#epg-review-card")?.scrollIntoView({ behavior: "smooth", block: "start" });
    page.querySelector("#epg-review-scroll")?.scrollTo({ top: 0, behavior: "smooth" });
  };

  page.querySelector("#epg-review-go")!.addEventListener("click", () => {
    const groups = pickedGroups();
    const min = Number(scoreSel.value);
    reviewQueue = rows.filter(
      (r) =>
        r.status !== "matched" &&
        groups.includes(r.groupTitle) &&
        r.suggestedTvgId &&
        r.score + 0.0001 >= min &&
        !String(r.suggestedTvgId).toLowerCase().includes("dummy"),
    );
    if (reviewQueue.length === 0) {
      toast("No suggestions at this score in the selected groups");
      return;
    }
    dlg.classList.remove("open");
    reviewAt = 0;
    reviewDlg.classList.add("open");
    void paintReviewCard();
  });
  page.querySelector("#epg-review-stop")!.addEventListener("click", () => {
    void finishReview(`Stopped after ${reviewAt} of ${reviewQueue.length}`);
  });
  page.querySelector("#epg-review-no")!.addEventListener("click", () => {
    reviewAt += 1;
    void paintReviewCard();
  });
  page.querySelector("#epg-review-yes")!.addEventListener("click", async () => {
    const r = reviewQueue[reviewAt];
    const hit = reviewHits[reviewPick] ?? reviewHits[0];
    if (r && hit) {
      try {
        await invoke("epg_apply", {
          managedId: r.managedChannelId,
          tvgId: hit.tvgId,
          logo: r.suggestedLogo,
          applyLogo: false,
          tvgShiftHours: Number(reviewShiftSel.value),
        });
      } catch (e) {
        toast(String(e));
      }
    }
    reviewAt += 1;
    void paintReviewCard();
  });

  page.querySelector("#epg-browse")!.addEventListener("click", async () => {
    try {
      await invoke("open_epg_catalog_window");
    } catch (e) {
      toast(String(e));
    }
  });

  let stopPick: (() => void) | undefined;
  void listen<{ tvgId: string; name: string }>("epg-catalog-pick", async (ev) => {
    if (!page.querySelector("#epg-auditor")) return;
    const tvg = ev.payload.tvgId?.trim();
    if (!tvg) return;
    if (!selected) {
      toast("Select a channel on EPG Audit first");
      return;
    }
    try {
      const box = page.querySelector<HTMLInputElement>("#epg-tvg");
      if (box) box.value = tvg;
      const keep = {
        group,
        prevIds: visibleIdsIn(group),
        appliedId: selected.managedChannelId,
      };
      await invoke("epg_apply", {
        managedId: selected.managedChannelId,
        tvgId: tvg,
        logo: selected.suggestedLogo,
        applyLogo: false,
      });
      notifyPhysicalSave();
      toast(`Applied ${tvg}`);
      await reload(keep);
    } catch (e) {
      toast(String(e));
    }
  }).then((un) => {
    stopPick = un;
  });

  try {
    const urlP = invoke<string>("epg_guide_url").then((url) => {
      const box = page.querySelector<HTMLInputElement>("#epg-url");
      if (box) box.value = url;
    });
    await Promise.all([urlP, reload()]);
  } catch (e) {
    toast(String(e));
  }
  return () => stopPick?.();
}

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}
