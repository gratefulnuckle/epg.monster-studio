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
    <div class="editor-toolbar">
      <span class="editor-title">Stream Audit</span>
      <button class="accent" id="au-start">Start (all variants)</button>
      <button id="au-visible">Visible only</button>
      <button id="au-specific">Audit specific channels…</button>
      <button id="au-today" title="Probe groups assigned to today in Settings (includes hidden backups)">Run today's groups</button>
      <button id="au-pause" disabled>Pause</button>
      <button id="au-resume" disabled>Resume</button>
      <button id="au-cancel" disabled>Cancel</button>
      <button id="au-undo">Undo last swap</button>
      <label class="check"><input type="checkbox" id="au-swap" checked /> Auto-swap on fail</label>
      <button id="au-results"># Results</button>
    </div>
    <p class="page-sub">Serial stream probes (ffmpeg + ffprobe). Live streams that only show a known “channel is offline” card fail as an offline slate.
    The full result list is kept if you leave this page. Pause / resume survives a crash via auditprocess.db.</p>
    <p class="page-sub" id="au-clock"></p>
    <div id="au-feed" class="audit-feed"></div>
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
  `;
}

export async function mountAudit(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  let feed: AuditFeedRow[] = [];
  let job: AuditJob | null = null;
  let running = false;
  let stopFlag: "none" | "pause" | "cancel" = "none";
  let delayMs = 6000;
  type PickCh = { id: string; name: string; group: string };
  let pick: PickCh[] = [];
  let results: AuditResult[] = [];
  let filterGrade = "";

  const autoSwap = () => (page.querySelector("#au-swap") as HTMLInputElement).checked;

  const buttons = () => {
    const r = running;
    const remain = !!job && (job.state === "running" || job.state === "paused") && job.currentIndex < job.total;
    (page.querySelector("#au-start") as HTMLButtonElement).disabled = r;
    (page.querySelector("#au-visible") as HTMLButtonElement).disabled = r;
    (page.querySelector("#au-specific") as HTMLButtonElement).disabled = r;
    (page.querySelector("#au-today") as HTMLButtonElement).disabled = r;
    (page.querySelector("#au-pause") as HTMLButtonElement).disabled = !r;
    (page.querySelector("#au-resume") as HTMLButtonElement).disabled = r || !remain;
    (page.querySelector("#au-cancel") as HTMLButtonElement).disabled = !r && !remain;
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

  const clock = () => {
    if (!job) {
      page.querySelector("#au-clock")!.textContent = "";
      return;
    }
    page.querySelector("#au-clock")!.textContent =
      `${job.state} · ${job.currentIndex}/${job.total} · ${job.okCount} ok · ${job.failCount} fail`;
  };

  const applySnap = (snap: AuditSnapshot) => {
    job = snap.job ?? null;
    feed = snap.feed ?? [];
    paintFeed();
    clock();
    buttons();
  };

  const loop = async () => {
    running = true;
    stopFlag = "none";
    buttons();
    try {
      while (running && stopFlag === "none") {
        const step = await invoke<AuditStep>("audit_next");
        job = step.job;
        feed.push(...step.feed);
        paintFeed();
        clock();
        if (step.done || job.state !== "running") break;
        if (job.currentIndex < job.total && delayMs > 0 && stopFlag === "none") {
          await new Promise((r) => setTimeout(r, delayMs));
        }
      }
      const reason = stopFlag as "none" | "pause" | "cancel";
      if (reason === "pause") {
        job = (await invoke<AuditJob | null>("audit_set_state", { next: "paused" })) ?? job;
        toast("Stream Audit paused");
      } else if (reason === "cancel") {
        job = (await invoke<AuditJob | null>("audit_set_state", { next: "cancelled" })) ?? job;
        toast("Stream Audit cancelled");
      } else if (job?.state === "completed") {
        toast("Stream Audit finished.");
      }
    } catch (e) {
      toast(String(e));
    } finally {
      running = false;
      buttons();
      clock();
    }
  };

  const start = async (visibleOnly: boolean, ids?: string[]) => {
    if (running) {
      toast("Stream Audit already running");
      return;
    }
    try {
      const settings = await invoke<{ AuditDelayMs?: number }>("load_settings");
      delayMs = settings.AuditDelayMs ?? 6000;
      job = await invoke<AuditJob>("audit_begin", {
        visibleOnly,
        autoSwap: autoSwap(),
        channelIds: ids ?? null,
      });
      feed = [];
      paintFeed();
      clock();
      void loop();
    } catch (e) {
      toast(String(e));
    }
  };

  page.querySelector("#au-start")!.addEventListener("click", () => void start(false));
  page.querySelector("#au-visible")!.addEventListener("click", () => void start(true));
  page.querySelector("#au-pause")!.addEventListener("click", () => {
    stopFlag = "pause";
  });
  page.querySelector("#au-cancel")!.addEventListener("click", () => {
    stopFlag = "cancel";
  });
  page.querySelector("#au-resume")!.addEventListener("click", async () => {
    try {
      job = await invoke<AuditJob>("audit_set_state", { next: "running" });
      void loop();
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#au-undo")!.addEventListener("click", async () => {
    try {
      const ok = await invoke<boolean>("audit_undo");
      toast(ok ? "Undid last swap." : "Nothing to undo.");
    } catch (e) {
      toast(String(e));
    }
  });

  page.querySelector("#au-today")!.addEventListener("click", async () => {
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
  });

  const pickDlg = page.querySelector("#au-pick-dlg")!;
  page.querySelector("#au-specific")!.addEventListener("click", async () => {
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
  });

  const paintPick = () => {
    const q = (page.querySelector("#au-pick-q") as HTMLInputElement).value.trim().toLowerCase();
    const el = page.querySelector("#au-pick-list")!;
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
        lab.innerHTML = `<input type="checkbox" data-id="${esc(c.id)}" data-g="${esc(g)}" /> ${esc(c.name)}`;
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
    const ids = [...page.querySelectorAll<HTMLInputElement>("#au-pick-list input[data-id]:checked")].map((i) => i.dataset.id!);
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
      ? `first estimate ${job.firstEtaSeconds}s`
      : "";
  };

  page.querySelector("#au-results")!.addEventListener("click", async () => {
    results = await invoke<AuditResult[]>("audit_results", { jobId: job?.id ?? null });
    filterGrade = "";
    paintResults();
    resDlg.classList.add("open");
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
      AutoSwapOnAuditFail?: boolean;
      WeeklyAuditAutoRun?: boolean;
      WeeklyAuditLastRun?: string;
    }>("load_settings");
    delayMs = settings.AuditDelayMs ?? 6000;
    const swap = page.querySelector("#au-swap") as HTMLInputElement | null;
    if (!swap) return;
    swap.checked = settings.AutoSwapOnAuditFail !== false;
    const snap = await invoke<AuditSnapshot>("audit_snapshot");
    if (!page.querySelector("#au-swap")) return;
    applySnap(snap);
    if (snap.interruptedOnLaunch || (snap.job && (snap.job.state === "paused" || snap.job.state === "running") && (snap.job.currentIndex ?? 0) < (snap.job.total ?? 0))) {
      page.querySelector("#au-resume-msg")!.textContent =
        `A Stream Audit was left at ${snap.job?.currentIndex ?? 0}/${snap.job?.total ?? 0}. Resume, start new, or wait.`;
      page.querySelector("#au-resume-dlg")!.classList.add("open");
    }
    if (settings.WeeklyAuditAutoRun) {
      const [day, groups] = await invoke<[string, string[], string[]]>("audit_today_groups");
      const last = settings.WeeklyAuditLastRun ?? "";
      if (groups.length && last.trim() !== new Date().toISOString().slice(0, 10)) {
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
