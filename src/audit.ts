import { invoke } from "@tauri-apps/api/core";

export type AuditJob = {
  id: string;
  state: string;
  scope: string;
  autoSwap: boolean;
  total: number;
  currentIndex: number;
  okCount: number;
  failCount: number;
  startedAt: string;
  updatedAt: string;
  finishedAt?: string | null;
  pid: number;
  gradesJson: string;
  firstEtaSeconds: number;
  elapsedMs: number;
};

export type AuditFeedRow = {
  seq: number;
  isHeader: boolean;
  title: string;
  subtitle: string;
  detail: string;
  grade: string;
  statusLabel: string;
  latencyLabel: string;
  ok: boolean;
};

export type AuditSnapshot = {
  job?: AuditJob | null;
  feed: AuditFeedRow[];
  queue: unknown[];
  gradeCounts: Record<string, number>;
  interruptedOnLaunch: boolean;
};

export type AuditStep = {
  job: AuditJob;
  feed: AuditFeedRow[];
  done: boolean;
};

export type AuditResult = {
  id: string;
  ok: boolean;
  error?: string | null;
  latencyMs?: number | null;
  grade: string;
  width?: number | null;
  height?: number | null;
  fps?: number | null;
  aspectRatio?: string | null;
  videoCodec?: string | null;
  audioCodec?: string | null;
  channelName?: string | null;
  groupTitle?: string | null;
  tvgId?: string | null;
  errorClass?: string | null;
};

const GRADE_COLOR: Record<string, string> = {
  A: "#69f0ae",
  B: "#b2ff59",
  C: "#ffd54f",
  D: "#ffb74d",
  F: "#ef5350",
};

export function auditHtml(): string {
  return `
    <h1 class="page-title">Stream Audit</h1>
    <p class="page-sub">Serial stream probes (ffmpeg + ffprobe). Live streams that only show a known “channel is offline” card fail as an offline slate. The full result list is kept if you leave this page. Pause / resume survives a crash via auditprocess.db.</p>
    <div class="editor-workspace">
    <div class="tabs-row">
      <div class="au-start-wrap" id="au-start-wrap">
        <button class="accent" id="au-start" type="button">Start</button>
        <div class="au-start-menu" id="au-start-menu" hidden>
          <button type="button" data-start="all">Start All</button>
          <button type="button" data-start="visible">Start Visible</button>
          <button type="button" data-start="specific">Start Specific Channels</button>
          <button type="button" data-start="today">Start Today's Groups</button>
        </div>
      </div>
      <button class="au-icon" id="au-pause" disabled title="Pause">&#xE769;</button>
      <button class="au-icon" id="au-resume" disabled title="Resume">&#xE768;</button>
      <button class="au-icon" id="au-cancel" disabled title="Cancel">&#xE71A;</button>
      <button class="au-icon" id="au-undo" title="Undo last swap">&#xE7A7;</button>
      <label class="check"><input type="checkbox" id="au-swap" checked /> Auto-swap on fail</label>
      <button id="au-results"># Results</button>
      <button type="button" class="au-icon tab-del" id="au-clear" title="Clear current audit">&#xE74D;</button>
    </div>
    <div id="au-feed" class="audit-feed"></div>
    <div class="au-foot">
      <div class="au-grades" id="au-grades">
        <span class="au-grade" data-g="A">A: 0</span>
        <span class="au-grade" data-g="B">B: 0</span>
        <span class="au-grade" data-g="C">C: 0</span>
        <span class="au-grade" data-g="D">D: 0</span>
        <span class="au-grade" data-g="F">F: 0</span>
      </div>
      <p class="au-status" id="au-clock">Idle</p>
    </div>
    <div class="dialog-backdrop" id="au-pick-dlg">
      <div class="dialog" style="width:560px;max-height:80vh;overflow:auto">
        <h2>Audit specific channels</h2>
        <div class="field"><label>Search</label><input id="au-pick-q" placeholder="group or channel…" /></div>
        <label class="check"><input type="checkbox" id="au-pick-backups" /> Include hidden backups</label>
        <div class="dialog-actions" style="justify-content:flex-start">
          <button id="au-pick-all">Select all groups</button>
          <button id="au-pick-none">Clear</button>
        </div>
        <div id="au-pick-list" class="editor-list" style="max-height:320px;margin-top:8px"></div>
        <p class="page-sub" id="au-pick-count">No channels selected</p>
        <div class="dialog-actions">
          <button id="au-pick-cancel">Cancel</button>
          <button class="accent" id="au-pick-go">Start</button>
        </div>
      </div>
    </div>
    <div class="dialog-backdrop" id="au-clear-dlg">
      <div class="dialog">
        <h2>Clear current audit?</h2>
        <p class="page-sub">This removes the current Stream Audit job, live feed, and stored results.</p>
        <div class="dialog-actions">
          <button type="button" id="au-clear-no">Cancel</button>
          <button type="button" class="accent" id="au-clear-yes">Clear</button>
        </div>
      </div>
    </div>
    <div class="dialog-backdrop" id="au-resume-dlg">
      <div class="dialog">
        <h2>Resume Stream Audit?</h2>
        <p class="page-sub" id="au-resume-msg"></p>
        <div class="dialog-actions">
          <button id="au-resume-no">Not now</button>
          <button id="au-resume-new">Start new</button>
          <button class="accent" id="au-resume-yes">Resume</button>
        </div>
      </div>
    </div>
    <div class="dialog-backdrop" id="au-res-dlg">
      <div class="dialog" style="width:720px;max-height:85vh;overflow:auto">
        <h2>Stream Audit results</h2>
        <p class="page-sub" id="au-res-now"></p>
        <p class="page-sub" id="au-res-sum"></p>
        <p class="page-sub" id="au-res-time"></p>
        <div id="au-res-bars" class="grade-bars"></div>
        <p class="page-sub" id="au-res-header">Click a grade bar</p>
        <div id="au-res-ferrors"></div>
        <div id="au-res-list" class="editor-list" style="max-height:280px"></div>
        <div class="dialog-actions">
          <button id="au-res-export">Export F-list</button>
          <button id="au-res-close">Close</button>
        </div>
      </div>
    </div>
    </div>
  `;
}

export async function mountAudit(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  let feed: AuditFeedRow[] = [];
  let job: AuditJob | null = null;
  let delayMs = 6000;
  let timeoutMs = 15000;
  let pollId = 0;
  type PickCh = { id: string; name: string; group: string };
  let pick: PickCh[] = [];
  let results: AuditResult[] = [];
  let filterGrade = "";
  let gradeCounts: Record<string, number> = { A: 0, B: 0, C: 0, D: 0, F: 0 };

  const autoSwap = () => (page.querySelector("#au-swap") as HTMLInputElement).checked;

  const running = () => job?.state === "running";
  const remain = () =>
    !!job && (job.state === "running" || job.state === "paused") && job.currentIndex < job.total;

  const buttons = () => {
    const r = running();
    (page.querySelector("#au-start") as HTMLButtonElement).disabled = r;
    page.querySelector("#au-start-menu")?.querySelectorAll<HTMLButtonElement>("button").forEach((b) => {
      b.disabled = r;
    });
    (page.querySelector("#au-pause") as HTMLButtonElement).disabled = !r;
    (page.querySelector("#au-resume") as HTMLButtonElement).disabled = r || !remain();
    (page.querySelector("#au-cancel") as HTMLButtonElement).disabled = !r;
    const n = (job?.okCount ?? 0) + (job?.failCount ?? 0);
    page.querySelector("#au-results")!.textContent = n ? `# Results  (${n})` : "# Results";
  };

  const paintFeed = () => {
    const el = page.querySelector("#au-feed")!;
    el.innerHTML = "";
    for (const row of feed) {
      if (row.isHeader) {
        const h = document.createElement("div");
        h.className = "audit-header";
        h.textContent = row.title;
        el.appendChild(h);
        continue;
      }
      const card = document.createElement("div");
      card.className = "audit-card";
      const g = GRADE_COLOR[row.grade] ?? "#777";
      card.innerHTML = `<span class="grade-badge" style="background:${g}">${esc(row.grade || "?")}</span>
        <span><span class="chan-name">${esc(row.title)}</span>
        <span class="chan-sub">${esc(row.subtitle)} · ${esc(row.detail)}</span></span>
        <span class="status-pill" style="background:${row.ok ? "#2e7d32" : "#c62828"}">${esc(row.statusLabel)}</span>
        <span class="chan-sub">${esc(row.latencyLabel)}</span>`;
      el.appendChild(card);
    }
    el.scrollTop = el.scrollHeight;
  };

  const hms = (sec: number) => {
    const s = Math.max(0, Math.floor(sec));
    return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m ${s % 60}s`;
  };

  const paintGrades = () => {
    const el = page.querySelector("#au-grades");
    if (!el) return;
    el.innerHTML = ["A", "B", "C", "D", "F"]
      .map((g) => `<span class="au-grade" data-g="${g}">${g}: ${gradeCounts[g] ?? 0}</span>`)
      .join("");
  };

  const clock = () => {
    const el = page.querySelector("#au-clock");
    if (!el) return;
    if (!job) {
      el.textContent = "Idle";
      return;
    }
    const elapsed = (job.elapsedMs || 0) / 1000;
    const left =
      job.total > 0
        ? Math.max(0, job.firstEtaSeconds * ((job.total - job.currentIndex) / job.total))
        : 0;
    const head =
      job.state === "running" && job.currentIndex === 0
        ? `Starting · ${job.total} stream(s) · delay ${delayMs}ms · timeout ${timeoutMs}ms`
        : job.state === "completed"
          ? "Audit finished"
          : job.state === "paused"
            ? "Paused"
            : job.state;
    el.textContent = `${head}    elapsed  ${hms(elapsed)}    left  ${hms(left)}`;
  };

  const applySnap = (snap: AuditSnapshot) => {
    job = snap.job ?? null;
    feed = snap.feed ?? [];
    const c = snap.gradeCounts ?? {};
    gradeCounts = {
      A: c.A ?? 0,
      B: c.B ?? 0,
      C: c.C ?? 0,
      D: c.D ?? 0,
      F: c.F ?? 0,
    };
    paintFeed();
    paintGrades();
    clock();
    buttons();
  };

  const stopPoll = () => {
    if (pollId) {
      window.clearInterval(pollId);
      pollId = 0;
    }
  };

  const startPoll = () => {
    if (pollId) return;
    const tick = async () => {
      if (!page.querySelector("#au-feed")) {
        stopPoll();
        return;
      }
      try {
        const snap = await invoke<AuditSnapshot>("audit_snapshot");
        const was = job?.state;
        applySnap(snap);
        if (snap.job?.state === "completed" && was === "running") {
          toast(`Stream Audit complete — ${snap.job.okCount} OK, ${snap.job.failCount} fail`);
        }
        if (snap.job?.state === "paused" && was === "running") {
          toast("Stream Audit paused");
        }
        if (snap.job?.state === "cancelled" && was === "running") {
          toast("Stream Audit cancelled");
        }
        if (snap.job?.state !== "running") stopPoll();
      } catch {
        /* keep polling */
      }
    };
    pollId = window.setInterval(() => void tick(), 1500);
    void tick();
  };

  const start = async (visibleOnly: boolean, ids?: string[]) => {
    if (running()) {
      toast("Stream Audit already running");
      return;
    }
    try {
      const settings = await invoke<{ AuditDelayMs?: number; AuditTimeoutMs?: number }>("load_settings");
      delayMs = settings.AuditDelayMs ?? 6000;
      timeoutMs = settings.AuditTimeoutMs ?? 15000;
      job = await invoke<AuditJob>("audit_begin", {
        visibleOnly,
        autoSwap: autoSwap(),
        channelIds: ids ?? null,
      });
      feed = [];
      gradeCounts = { A: 0, B: 0, C: 0, D: 0, F: 0 };
      paintFeed();
      paintGrades();
      clock();
      buttons();
      startPoll();
    } catch (e) {
      toast(String(e));
    }
  };

  const startMenu = page.querySelector<HTMLElement>("#au-start-menu")!;
  const closeStartMenu = () => {
    startMenu.hidden = true;
  };
  page.querySelector("#au-start")!.addEventListener("click", (ev) => {
    ev.stopPropagation();
    if (running()) return;
    startMenu.hidden = !startMenu.hidden;
  });
  startMenu.addEventListener("click", (ev) => {
    ev.stopPropagation();
    const t = (ev.target as HTMLElement).closest("button[data-start]") as HTMLButtonElement | null;
    if (!t || t.disabled) return;
    closeStartMenu();
    const kind = t.dataset.start;
    if (kind === "all") void start(false);
    else if (kind === "visible") void start(true);
    else if (kind === "specific") void openSpecific();
    else if (kind === "today") void runToday();
  });
  page.addEventListener("click", () => closeStartMenu());
  page.querySelector("#au-pause")!.addEventListener("click", () => {
    void invoke("audit_interrupt", { kind: "paused" }).catch((e) => toast(String(e)));
  });
  page.querySelector("#au-cancel")!.addEventListener("click", () => {
    void invoke("audit_interrupt", { kind: "cancelled" }).catch((e) => toast(String(e)));
  });
  page.querySelector("#au-resume")!.addEventListener("click", async () => {
    try {
      job = await invoke<AuditJob>("audit_set_state", { next: "running" });
      startPoll();
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#au-undo")!.addEventListener("click", async () => {
    try {
      const ok = await invoke<boolean>("audit_undo");
      toast(ok ? "Swap undone" : "Nothing to undo");
    } catch (e) {
      toast(String(e));
    }
  });
  const clearDlg = page.querySelector("#au-clear-dlg")!;
  const runClearAudit = async () => {
    try {
      if (running()) {
        await invoke("audit_interrupt", { kind: "cancelled" });
      }
      stopPoll();
      await invoke("audit_discard");
      job = null;
      feed = [];
      results = [];
      filterGrade = "";
      gradeCounts = { A: 0, B: 0, C: 0, D: 0, F: 0 };
      paintFeed();
      paintGrades();
      paintResults();
      page.querySelector("#au-res-dlg")?.classList.remove("open");
      clock();
      buttons();
      toast("Audit cleared.");
    } catch (e) {
      toast(String(e));
    }
  };
  page.querySelector("#au-clear")!.addEventListener("click", () => {
    clearDlg.classList.add("open");
  });
  page.querySelector("#au-clear-no")!.addEventListener("click", () => {
    clearDlg.classList.remove("open");
  });
  page.querySelector("#au-clear-yes")!.addEventListener("click", () => {
    clearDlg.classList.remove("open");
    void runClearAudit();
  });
  clearDlg.addEventListener("click", (ev) => {
    if (ev.target === clearDlg) clearDlg.classList.remove("open");
  });

  const runToday = async () => {
    try {
      const [day, groups, ids] = await invoke<[string, string[], string[]]>("audit_today_groups");
      if (groups.length === 0) {
        toast(`No groups assigned to ${day} in Settings.`);
        return;
      }
      if (ids.length === 0) {
        toast("No managed channels match today's groups: " + groups.join(", "));
        return;
      }
      await start(false, ids);
      await invoke("audit_mark_today_ran");
    } catch (e) {
      toast(String(e));
    }
  };

  const pickDlg = page.querySelector("#au-pick-dlg")!;
  const openSpecific = async () => {
    try {
      const groups = await invoke<{ title: string; count: number }[]>("list_managed_groups");
      pick = [];
      for (const g of groups) {
        const chans = await invoke<{ id: string; name: string; groupTitle: string }[]>("list_managed", {
          group: g.title,
        });
        for (const c of chans) pick.push({ id: c.id, name: c.name, group: c.groupTitle });
      }
      if (pick.length === 0) {
        toast("Load a playlist in Playlist Editor first");
        return;
      }
      paintPick();
      pickDlg.classList.add("open");
    } catch (e) {
      toast(String(e));
    }
  };

  const pickedIds = new Set<string>();
  const paintPick = () => {
    const q = (page.querySelector("#au-pick-q") as HTMLInputElement).value.trim().toLowerCase();
    const el = page.querySelector("#au-pick-list")!;
    el.querySelectorAll<HTMLInputElement>("input[data-id]").forEach((box) => {
      if (box.checked && box.dataset.id) pickedIds.add(box.dataset.id);
      else if (box.dataset.id) pickedIds.delete(box.dataset.id);
    });
    el.innerHTML = "";
    const groups = [...new Set(pick.map((p) => p.group))];
    for (const g of groups) {
      const chans = pick.filter((p) => p.group === g && (!q || p.name.toLowerCase().includes(q) || g.toLowerCase().includes(q)));
      if (chans.length === 0) continue;
      const head = document.createElement("label");
      head.className = "check";
      head.innerHTML = `<input type="checkbox" data-group="${esc(g)}" /> <strong>${esc(g)}</strong>`;
      el.appendChild(head);
      for (const c of chans) {
        const lab = document.createElement("label");
        lab.className = "check";
        const on = pickedIds.has(c.id) ? " checked" : "";
        lab.innerHTML = `<input type="checkbox" data-id="${esc(c.id)}" data-g="${esc(g)}"${on} /> ${esc(c.name)}`;
        el.appendChild(lab);
      }
    }
    el.querySelectorAll<HTMLInputElement>("input[data-group]").forEach((box) => {
      box.addEventListener("change", () => {
        el.querySelectorAll<HTMLInputElement>(`input[data-g="${box.dataset.group}"]`).forEach((c) => {
          c.checked = box.checked;
        });
        countPick();
      });
    });
    el.querySelectorAll<HTMLInputElement>("input[data-id]").forEach((box) => {
      box.addEventListener("change", countPick);
    });
    countPick();
  };

  const countPick = () => {
    const n = page.querySelectorAll<HTMLInputElement>("#au-pick-list input[data-id]:checked").length;
    page.querySelector("#au-pick-count")!.textContent = n === 0 ? "No channels selected" : `${n} channel(s) selected`;
  };

  page.querySelector("#au-pick-q")!.addEventListener("input", paintPick);
  page.querySelector("#au-pick-all")!.addEventListener("click", () => {
    page.querySelectorAll<HTMLInputElement>("#au-pick-list input").forEach((i) => (i.checked = true));
    countPick();
  });
  page.querySelector("#au-pick-none")!.addEventListener("click", () => {
    page.querySelectorAll<HTMLInputElement>("#au-pick-list input").forEach((i) => (i.checked = false));
    countPick();
  });
  page.querySelector("#au-pick-cancel")!.addEventListener("click", () => pickDlg.classList.remove("open"));
  page.querySelector("#au-pick-go")!.addEventListener("click", () => {
    page.querySelectorAll<HTMLInputElement>("#au-pick-list input[data-id]").forEach((box) => {
      if (box.checked && box.dataset.id) pickedIds.add(box.dataset.id);
    });
    const ids = [...pickedIds];
    if (ids.length === 0) return;
    const backups = (page.querySelector("#au-pick-backups") as HTMLInputElement).checked;
    pickDlg.classList.remove("open");
    void start(!backups, ids);
  });

  const resDlg = page.querySelector("#au-res-dlg")!;
  const paintResults = () => {
    const counts: Record<string, number> = { A: 0, B: 0, C: 0, D: 0, F: 0 };
    for (const r of results) counts[r.grade] = (counts[r.grade] ?? 0) + 1;
    const bars = page.querySelector("#au-res-bars")!;
    bars.innerHTML = "";
    const max = Math.max(1, ...Object.values(counts));
    for (const g of ["A", "B", "C", "D", "F"]) {
      const b = document.createElement("button");
      b.className = "grade-bar";
      b.innerHTML = `<span class="grade-badge" style="background:${GRADE_COLOR[g]}">${g}</span>
        <span class="bar" style="height:${Math.round((counts[g] / max) * 100)}px;background:${GRADE_COLOR[g]}"></span>
        <span>${counts[g] ?? 0}</span>`;
      b.addEventListener("click", () => {
        filterGrade = g;
        paintResults();
      });
      bars.appendChild(b);
    }
    const list = filterGrade ? results.filter((r) => r.grade === filterGrade) : results;
    page.querySelector("#au-res-header")!.textContent = filterGrade
      ? `Grade ${filterGrade} · ${list.length}`
      : "Click a grade bar";
    const ferr = page.querySelector("#au-res-ferrors")!;
    ferr.innerHTML = "";
    if (filterGrade === "F") {
      const bag: Record<string, number> = {};
      for (const r of list) {
        const k = r.errorClass || "other";
        bag[k] = (bag[k] ?? 0) + 1;
      }
      ferr.innerHTML = Object.entries(bag)
        .map(([k, n]) => `<span class="chan-sub">${esc(k)} ${n}</span>`)
        .join(" · ");
    }
    const el = page.querySelector("#au-res-list")!;
    el.innerHTML = "";
    for (const r of list) {
      const row = document.createElement("div");
      row.className = "audit-card";
      row.innerHTML = `<span class="grade-badge" style="background:${GRADE_COLOR[r.grade] ?? "#777"}">${esc(r.grade)}</span>
        <span><span class="chan-name">${esc(r.channelName ?? "")}</span>
        <span class="chan-sub">${esc(r.groupTitle ?? "")} · ${esc(r.tvgId ?? "")}</span></span>
        <span class="chan-sub">${esc(r.error ?? `${r.width ?? "—"}×${r.height ?? "—"}`)}</span>`;
      el.appendChild(row);
    }
    page.querySelector("#au-res-now")!.textContent = job ? job.state : "Idle";
    page.querySelector("#au-res-sum")!.textContent = job
      ? `${job.okCount} ok · ${job.failCount} fail · ${job.currentIndex}/${job.total}`
      : "";
    page.querySelector("#au-res-time")!.textContent = job
      ? `elapsed  ${hms((job.elapsedMs || 0) / 1000)}  ·  first estimate  ${hms(job.firstEtaSeconds)}`
      : "";
  };

  page.querySelector("#au-results")!.addEventListener("click", async () => {
    try {
      results = await invoke<AuditResult[]>("audit_results", { jobId: job?.id ?? null });
      filterGrade = "";
      paintResults();
      resDlg.classList.add("open");
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#au-res-close")!.addEventListener("click", () => resDlg.classList.remove("open"));
  page.querySelector("#au-res-export")!.addEventListener("click", async () => {
    const fails = results.filter((r) => r.grade === "F");
    const text = fails
      .map((r) => `${r.channelName ?? ""}	${r.groupTitle ?? ""}	${r.errorClass ?? ""}	${r.error ?? ""}`)
      .join("\n");
    try {
      await navigator.clipboard.writeText(text);
      toast(`Copied ${fails.length} F rows.`);
    } catch {
      toast("Could not copy F-list.");
    }
  });

  try {
    const settings = await invoke<{
      AuditDelayMs?: number;
      AuditTimeoutMs?: number;
      AutoSwapOnAuditFail?: boolean;
      WeeklyAuditAutoRun?: boolean;
      WeeklyAuditLastRun?: string;
    }>("load_settings");
    delayMs = settings.AuditDelayMs ?? 6000;
    timeoutMs = settings.AuditTimeoutMs ?? 15000;
    const swap = page.querySelector("#au-swap") as HTMLInputElement | null;
    if (!swap) return;
    swap.checked = settings.AutoSwapOnAuditFail !== false;
    const snap = await invoke<AuditSnapshot>("audit_snapshot");
    if (!page.querySelector("#au-swap")) return;
    applySnap(snap);
    if (snap.job?.state === "running") {
      startPoll();
    } else if (
      snap.interruptedOnLaunch ||
      (snap.job &&
        snap.job.state === "paused" &&
        (snap.job.currentIndex ?? 0) < (snap.job.total ?? 0))
    ) {
      page.querySelector("#au-resume-msg")!.textContent =
        `A Stream Audit did not finish (${snap.job?.currentIndex ?? 0} of ${snap.job?.total ?? 0} probed, ${snap.job?.okCount ?? 0} OK / ${snap.job?.failCount ?? 0} fail). Resume from where it stopped, or start a new run?`;
      page.querySelector("#au-resume-dlg")!.classList.add("open");
    }
    if (settings.WeeklyAuditAutoRun) {
      const [day, groups] = await invoke<[string, string[], string[]]>("audit_today_groups");
      const last = settings.WeeklyAuditLastRun ?? "";
      const now = new Date();
      const today = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
      if (groups.length && last.trim() !== today) {
        toast(`${day} groups ready (${groups.join(", ")}). Click Run today's groups when you want to probe.`);
      }
    }
  } catch (e) {
    toast(String(e));
  }

  page.querySelector("#au-resume-no")!.addEventListener("click", () => page.querySelector("#au-resume-dlg")!.classList.remove("open"));
  page.querySelector("#au-resume-yes")!.addEventListener("click", () => {
    page.querySelector("#au-resume-dlg")!.classList.remove("open");
    (page.querySelector("#au-resume") as HTMLButtonElement).click();
  });
  page.querySelector("#au-resume-new")!.addEventListener("click", async () => {
    page.querySelector("#au-resume-dlg")!.classList.remove("open");
    await invoke("audit_discard");
    job = null;
    feed = [];
    paintFeed();
    clock();
    buttons();
  });
}

function esc(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}
