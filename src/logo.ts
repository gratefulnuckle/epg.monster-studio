import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { bindVirtualList, type VirtualList } from "./virtual";
import { bindPlayerLogo } from "./logo-src";
import { bindThreeColSplit } from "./split";

export type LogoIssue = {
  managedChannelId: string;
  channelName: string;
  groupTitle: string;
  tvgId?: string | null;
  currentLogo?: string | null;
  issue: string;
  reason: string;
};

export type SaveItem = {
  channelId: string;
  name: string;
  group: string;
  tvgId: string;
  url: string;
  destPath: string;
  status: string;
  error?: string | null;
};

const LABEL: Record<string, string> = {
  missing: "Missing",
  invalid: "Invalid URL",
  broken: "Won't load",
  "player-reject": "Players reject",
  "": "OK",
};

export function logoHtml(): string {
  return `
    <h1 class="page-title">Logo Audit</h1>
    <p class="page-sub">Scan uses a player-style GET (VLC UA) — the same fetch TiviMate, Plex, and tuners do. A logo that only opens in a browser is a fail.</p>
    <div class="editor-workspace">
    <div class="tabs-row">
      <button class="accent" id="lg-scan" title="Re-probe all logos (missing / invalid / won't load)">Scan logos</button>
      <button id="lg-save" title="Download managed logos into group\\tvg-id.png. Existing files are skipped unless the download is a different size.">Save Logos</button>
      <button id="lg-batch" title="Pick several issue channels and set the same logo URL on all of them">Batch set logos</button>
      <label class="check"><input type="checkbox" id="lg-issues" checked /> Issues only</label>
      <span class="page-sub" id="lg-summary"></span>
    </div>
    <div class="editor-grid editor-split" id="lg-split">
      <section class="groups editor-pane">
        <div class="groups-head">Groups</div>
        <div id="lg-groups" class="groups-body"></div>
      </section>
      <div class="split-handle" id="lg-split-groups" title="Drag to resize groups"></div>
      <section class="channels editor-pane">
        <div class="groups-head">Channels</div>
        <div id="lg-channels" class="editor-list"></div>
      </section>
      <div class="split-handle" id="lg-split-chans" title="Drag to resize channels"></div>
      <section class="tile editor-pane">
        <div class="groups-head">Logo finder</div>
        <p class="page-sub" id="lg-empty">Select a channel.</p>
        <div id="lg-detail" hidden>
          <div id="lg-name" class="chan-name"></div>
          <div id="lg-reason" class="chan-sub"></div>
          <div class="logo-preview" id="lg-preview"></div>
          <div class="field"><label>Logo URL (tvg-logo)</label><input id="lg-url" /></div>
          <button class="accent" id="lg-apply">Apply logo URL</button>
          <button id="lg-clear">Clear logo</button>
          <p class="page-sub">Right-click the image → Copy image address (the file, not the page). Prefer direct https PNG/JPEG.</p>
          <p class="page-sub">Players usually reject: Wikimedia/Wikipedia, Google/Bing image pages, GitHub blob URLs, SVG, WebP, anything that needs a browser login or Referer. tv-logos raw PNG links work.</p>
          <button id="lg-google">Google Images (transparent)</button>
          <button id="lg-ddg">DuckDuckGo Images</button>
          <button id="lg-tvlogos" title="Search free IPTV logos in the tv-logo/tv-logos GitHub pack">tv-logos (GitHub pack)</button>
          <p class="page-sub" id="lg-query"></p>
        </div>
      </section>
    </div>
    <p class="page-sub" id="lg-status">Scan logos to detect missing, invalid, and broken (won't load) URLs.</p>
    <div class="dialog-backdrop" id="lg-batch-dlg">
      <div class="dialog">
        <h2>Batch set logos</h2>
        <div class="field"><label>Logo URL</label><input id="lg-batch-url" /></div>
        <div id="lg-batch-list" class="editor-list" style="max-height:240px"></div>
        <div class="dialog-actions">
          <button id="lg-batch-cancel">Cancel</button>
          <button class="accent" id="lg-batch-go">Set on selected</button>
        </div>
      </div>
    </div>
    <div class="dialog-backdrop" id="lg-save-dlg">
      <div class="dialog" style="width:720px;max-height:80vh;overflow:auto">
        <h2 id="lg-save-title">Save Logos</h2>
        <p class="page-sub" id="lg-save-copy">Downloads each managed logo one at a time into group\\tvg-id.png. Files already on disk are skipped unless the download is a different size. Does not change Playlist Editor tvg-logo URLs. Run Scan logos first so URLs are live.</p>
        <p class="page-sub" id="lg-save-tracker"></p>
        <div class="field"><label>Save folder</label><input id="lg-save-dir" /></div>
        <div class="dialog-actions" style="justify-content:flex-start;margin-top:8px">
          <button id="lg-save-default">Use default</button>
        </div>
        <div id="lg-save-list" class="editor-list" style="max-height:280px;margin-top:10px"></div>
        <p class="page-sub" id="lg-save-status"></p>
        <div class="dialog-actions">
          <button id="lg-save-cancel">Cancel</button>
          <button id="lg-save-retry">Retry failed</button>
          <button class="accent" id="lg-save-go">Start / Resume</button>
        </div>
      </div>
    </div>
    </div>
  `;
}

export async function mountLogo(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  const split = page.querySelector<HTMLElement>("#lg-split");
  const splitGroups = page.querySelector<HTMLElement>("#lg-split-groups");
  const splitChans = page.querySelector<HTMLElement>("#lg-split-chans");
  if (split && splitGroups && splitChans) {
    bindThreeColSplit(split, splitGroups, splitChans, "studio-lg");
  }

  let rows: LogoIssue[] = [];
  let group = "";
  let selected: LogoIssue | null = null;
  let issuesOnly = true;
  let groupVirt: VirtualList<string> | null = null;
  let chanVirt: VirtualList<LogoIssue> | null = null;
  let saveItems: SaveItem[] = [];
  let saveRoot = "";
  let saving = false;

  const setStatus = (s: string) => {
    const el = page.querySelector("#lg-status");
    if (el) el.textContent = s;
  };

  const summarize = () => {
    const issues = rows.filter((r) => r.issue);
    const missing = issues.filter((r) => r.issue === "missing").length;
    const invalid = issues.filter((r) => r.issue === "invalid").length;
    const broken = issues.filter((r) => r.issue === "broken").length;
    const reject = issues.filter((r) => r.issue === "player-reject").length;
    const summary = page.querySelector("#lg-summary");
    if (summary) summary.textContent = `${issues.length} issues`;
    setStatus(
      issues.length === 0
        ? "All logos pass a player-style GET (PNG/JPEG/GIF)."
        : `${issues.length} issue(s): ${missing} missing · ${invalid} invalid · ${broken} won't load · ${reject} players reject`,
    );
  };

  const reload = async (probe: boolean) => {
    setStatus(probe ? "Probing logos…" : "Loading channels…");
    const unlisten = probe
      ? await listen<{ current: number; total: number; issues: number; name: string }>(
          "logo-scan-progress",
          (ev) => {
            if (!page.querySelector("#lg-status")) return;
            const p = ev.payload;
            setStatus(`Probing ${p.current}/${p.total} — ${p.name} (${p.issues} issues)`);
          },
        )
      : null;
    try {
      rows = await invoke<LogoIssue[]>("logo_scan", { probe });
    } finally {
      unlisten?.();
    }
    if (!page.querySelector("#lg-groups")) return;
    summarize();
    paintGroups();
  };

  const paintGroups = () => {
    const titles = [...new Set(rows.map((r) => r.groupTitle))].filter((t) =>
      !issuesOnly || rows.some((r) => r.groupTitle === t && r.issue),
    );
    const el = page.querySelector<HTMLElement>("#lg-groups");
    if (!el) return;
    if (!titles.includes(group)) group = titles[0] ?? "";
    if (selected && !titles.includes(selected.groupTitle)) selected = null;
    groupVirt?.destroy();
    el.innerHTML = "";
    groupVirt = bindVirtualList({
      scroller: el,
      rowHeight: 36,
      renderRow: (t) => {
        const n = rows.filter((r) => r.groupTitle === t && r.issue).length;
        const b = document.createElement("button");
        b.className = "group-row" + (t === group ? " active" : "");
        b.innerHTML = `${esc(t)}<span class="issue-n"> ${n} issues</span>`;
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
    paintDetail();
  };

  const paintChannels = () => {
    const el = page.querySelector<HTMLElement>("#lg-channels");
    if (!el) return;
    const list = rows.filter((r) => r.groupTitle === group && (!issuesOnly || r.issue));
    if (!chanVirt) {
      chanVirt = bindVirtualList({
        scroller: el,
        rowHeight: 48,
        renderRow: (r) => {
          const b = document.createElement("button");
          b.className = "chan-pick" + (selected?.managedChannelId === r.managedChannelId ? " active" : "");
          b.innerHTML = `<span class="lg-thumb">${r.issue ? `<span class="broken">!</span>` : ""}</span>
        <span><span class="chan-name">${esc(r.channelName)}</span>
        <span class="chan-sub">${esc(r.tvgId ?? "")}</span></span>
        ${
            r.issue === "player-reject"
              ? `<span class="issue-n">${esc(LABEL[r.issue])}</span>`
              : r.issue
                ? `<span class="status-pill">${esc(LABEL[r.issue] ?? r.issue)}</span>`
                : ""
          }`;
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
    const empty = page.querySelector<HTMLElement>("#lg-empty")!;
    const body = page.querySelector<HTMLElement>("#lg-detail")!;
    if (!selected) {
      empty.hidden = false;
      body.hidden = true;
      return;
    }
    empty.hidden = true;
    body.hidden = false;
    page.querySelector("#lg-name")!.textContent = selected.channelName;
    const reason = page.querySelector("#lg-reason")!;
    reason.className = "chan-sub";
    reason.textContent = selected.reason || LABEL[selected.issue] || "";
    (page.querySelector("#lg-url") as HTMLInputElement).value = selected.currentLogo ?? "";
    const q = selected.channelName.trim() ? `${selected.channelName.trim()} logo` : "channel logo";
    page.querySelector("#lg-query")!.textContent = `Search query: ${q}`;
    paintPreview();
  };

  const paintPreview = () => {
    const url = (page.querySelector("#lg-url") as HTMLInputElement).value.trim();
    const slot = page.querySelector("#lg-preview")!;
    if (!url) {
      slot.innerHTML = `<span class="broken">broken logo</span>`;
      return;
    }
    slot.innerHTML = `<img alt="" />`;
    const img = slot.querySelector("img");
    if (img) {
      bindPlayerLogo(img, url, () => {
        slot.innerHTML = `<span class="broken">broken logo</span>`;
      });
    }
  };

  page.querySelector("#lg-issues")!.addEventListener("change", (ev) => {
    issuesOnly = (ev.target as HTMLInputElement).checked;
    paintGroups();
  });
  page.querySelector("#lg-scan")!.addEventListener("click", async () => {
    const btn = page.querySelector<HTMLButtonElement>("#lg-scan")!;
    btn.disabled = true;
    toast("Scanning logos in the background…");
    try {
      await reload(true);
      toast("Scan complete.");
    } finally {
      btn.disabled = false;
    }
  });
  page.querySelector("#lg-url")!.addEventListener("change", paintPreview);
  page.querySelector("#lg-apply")!.addEventListener("click", async () => {
    if (!selected) return;
    const url = (page.querySelector("#lg-url") as HTMLInputElement).value.trim();
    if (!url) {
      toast("Paste a logo URL first");
      return;
    }
    try {
      await invoke("logo_set", { managedId: selected.managedChannelId, url });
      toast(`Logo saved for ${selected.channelName}`);
      await reload(false);
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#lg-clear")!.addEventListener("click", async () => {
    if (!selected) return;
    try {
      await invoke("logo_set", { managedId: selected.managedChannelId, url: null });
      (page.querySelector("#lg-url") as HTMLInputElement).value = "";
      toast("Logo cleared.");
      await reload(false);
    } catch (e) {
      toast(String(e));
    }
  });

  const openSearch = async (which: "google" | "ddg" | "tv") => {
    if (!selected) return;
    const [g, d, t] = await invoke<[string, string, string]>("logo_search_urls", {
      name: selected.channelName,
    });
    const url = which === "google" ? g : which === "ddg" ? d : t;
    await openUrl(url);
    toast("Opened browser search");
  };
  page.querySelector("#lg-google")!.addEventListener("click", () => void openSearch("google"));
  page.querySelector("#lg-ddg")!.addEventListener("click", () => void openSearch("ddg"));
  page.querySelector("#lg-tvlogos")!.addEventListener("click", () => void openSearch("tv"));

  const batchDlg = page.querySelector("#lg-batch-dlg")!;
  page.querySelector("#lg-batch")!.addEventListener("click", () => {
    const issues = rows.filter((x) => x.issue);
    if (issues.length === 0) {
      toast("No logo issues to batch-set (scan first)");
      return;
    }
    const list = page.querySelector("#lg-batch-list")!;
    list.innerHTML = "";
    for (const r of issues) {
      const lab = document.createElement("label");
      lab.className = "check";
      lab.innerHTML = `<input type="checkbox" data-id="${esc(r.managedChannelId)}" /> ${esc(r.channelName)}`;
      list.appendChild(lab);
    }
    batchDlg.classList.add("open");
  });
  page.querySelector("#lg-batch-cancel")!.addEventListener("click", () => batchDlg.classList.remove("open"));
  page.querySelector("#lg-batch-go")!.addEventListener("click", async () => {
    const url = (page.querySelector("#lg-batch-url") as HTMLInputElement).value.trim();
    const ids = [...page.querySelectorAll<HTMLInputElement>("#lg-batch-list input:checked")].map((i) => i.dataset.id!);
    if (!url || ids.length === 0) {
      toast("Paste a logo URL first");
      return;
    }
    try {
      const n = await invoke<number>("logo_batch_set", { ids, url });
      batchDlg.classList.remove("open");
      toast(`Set logo on ${n} channels.`);
      await reload(false);
    } catch (e) {
      toast(String(e));
    }
  });

  const saveDlg = page.querySelector("#lg-save-dlg")!;
  const dirBox = page.querySelector("#lg-save-dir") as HTMLInputElement;

  const loadPlan = async (root?: string) => {
    const [dir, items] = await invoke<[string, SaveItem[]]>("logo_save_plan", { root: root ?? null });
    saveRoot = dir;
    saveItems = items;
    dirBox.value = dir;
    paintSave();
  };

  const persistTracker = async () => {
    if (!saveRoot) return;
    await invoke("logo_save_tracker", { root: saveRoot, items: saveItems });
  };

  const openSaveDlg = async () => {
    await loadPlan();
    saveDlg.classList.add("open");
  };
  page.querySelector("#lg-save")!.addEventListener("click", () => void openSaveDlg());
  page.querySelector("#lg-save-default")!.addEventListener("click", async () => {
    const def = await invoke<string>("logo_default_dir");
    await loadPlan(def);
  });
  page.querySelector("#lg-save-cancel")!.addEventListener("click", async () => {
    if (saving) {
      saving = false;
      page.querySelector("#lg-save-status")!.textContent = "Paused. Start / Resume to continue.";
      await persistTracker();
      return;
    }
    saveDlg.classList.remove("open");
  });

  const runSave = async (retryFailed: boolean) => {
    saveRoot = dirBox.value.trim();
    if (!saveRoot) {
      toast("Pick a save folder first.");
      return;
    }
    if (saveItems.length === 0) await loadPlan(saveRoot);
    if (saveItems.length === 0) return;
    if (retryFailed) {
      for (const it of saveItems) {
        if (it.status === "failed") {
          it.status = "pending";
          it.error = null;
        }
      }
    }
    saving = true;
    (page.querySelector("#lg-save-go") as HTMLButtonElement).disabled = true;
    (page.querySelector("#lg-save-retry") as HTMLButtonElement).disabled = true;
    for (let i = 0; i < saveItems.length && saving; i++) {
      const it = saveItems[i];
      if (it.status === "saved" || it.status === "skip") continue;
      if (it.status !== "pending" && it.status !== "cached") continue;
      saveItems[i] = await invoke<SaveItem>("logo_save_one", { item: it });
      await persistTracker();
      if (!page.querySelector("#lg-save-list")) return;
      paintSave();
    }
    const paused = !saving;
    saving = false;
    (page.querySelector("#lg-save-go") as HTMLButtonElement).disabled = false;
    (page.querySelector("#lg-save-retry") as HTMLButtonElement).disabled = false;
    await persistTracker();
    paintSave();
    const saved = saveItems.filter((x) => x.status === "saved").length;
    const skipped = saveItems.filter((x) => x.status === "skip" || x.status === "cached").length;
    const fail = saveItems.filter((x) => x.status === "failed").length;
    const st = page.querySelector("#lg-save-status");
    if (st) st.textContent = paused
      ? `Paused. ${saved} saved · ${skipped} cached/skipped · ${fail} failed. Start / Resume to continue.`
      : `Done. ${saved} saved · ${skipped} cached/skipped · ${fail} failed → ${saveRoot}`;
    if (!paused) toast("Logo download finished.");
  };

  page.querySelector("#lg-save-go")!.addEventListener("click", () => void runSave(false));
  page.querySelector("#lg-save-retry")!.addEventListener("click", () => void runSave(true));

  function paintSave() {
    const el = page.querySelector("#lg-save-list");
    if (!el) return;
    el.innerHTML = "";
    saveItems.forEach((it, i) => {
      const row = document.createElement("div");
      row.className = "chan-sub";
      row.textContent = `${i + 1}. ${it.name} · ${it.group} · ${it.tvgId} → ${it.status}${it.error ? " · " + it.error : ""}`;
      el.appendChild(row);
    });
    const saved = saveItems.filter((x) => x.status === "saved").length;
    const cached = saveItems.filter((x) => x.status === "cached").length;
    const fail = saveItems.filter((x) => x.status === "failed" || x.status === "skip").length;
    const pending = saveItems.filter((x) => x.status === "pending" || x.status === "saving").length;
    const total = saveItems.length;
    page.querySelector("#lg-save-tracker")!.textContent =
      total === 0
        ? "No logos with a tvg-id to save."
        : `Tracker: ${saved} saved · ${cached} cached · ${pending} remaining · ${fail} failed/skipped of ${total}`;
    if (!saving) {
      page.querySelector("#lg-save-status")!.textContent =
        total === 0
          ? ""
          : pending === 0 && cached === total
            ? "All of these are already cached. Start / Resume re-checks size and skips matches."
            : pending === 0
              ? "Nothing left to download. Retry failed to try again."
              : "Start / Resume skips cached files unless the download is a different size.";
    }
  }

  try {
    await reload(false);
  } catch (e) {
    toast(String(e));
  }
}

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}
