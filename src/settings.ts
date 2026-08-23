import { invoke } from "@tauri-apps/api/core";
import { applyPlayerEngineValue, loadStudioCaps, playerEngineOptionsHtml } from "./capabilities";

export type TunerProfile = {
  Kind: string;
  Enabled: boolean;
  Running: boolean;
  FriendlyName: string;
  DeviceId: string;
  TunerCount: number;
  BindAddress: string;
  Port: number;
  AllowLan: boolean;
  RemuxEnabled: boolean;
  DownspiralEnabled: boolean;
};

export type AppSettings = {
  DefaultPlayer: number;
  MpvPath: string;
  VlcPath: string;
  FfmpegPath: string;
  FfprobePath: string;
  AuditDelayMs: number;
  AuditTimeoutMs: number;
  AutoSwapOnAuditFail: boolean;
  PauseAuditWhilePlaying: boolean;
  DefaultUserAgent: string;
  PythonPath?: string | null;
  EpgShareUrl: string;
  EpgXmlUrl: string;
  EpgXmlUrls?: string[] | null;
  PlexTuner: TunerProfile;
  JellyfinTuner: TunerProfile;
  EmbyTuner: TunerProfile;
  IptvTuner: TunerProfile;
  TunerUseMemberEpg: boolean;
  DiscoveryEnabled: boolean;
  RemuxEngine: string;
  RemuxProfile: string;
  RemuxBufferKb: number;
  WeeklyAuditJson: string;
  WeeklyAuditAutoRun: boolean;
  BlackDetectEnabled: boolean;
  WeeklyAuditLastRun: string;
  LogoSaveDirectory: string;
  HostLogosOnTuner: boolean;
  UseLocalLogos: boolean;
  CacheLogos?: boolean;
  MemberEmail: string;
  MemberUsername: string;
  MemberAccessKey: string;
  MemberApiBase: string;
  MemberFeedUrl: string;
  MemberFeedUrlGz: string;
  MemberMaxChannels: number;
  MemberMaxBodyBytes: number;
  MemberLastPublishedAt: string;
  MemberLastPingAt: string;
  CheckForAppUpdates?: boolean;
};

type Ping = {
  ok: boolean;
  message: string;
  email?: string | null;
  username?: string | null;
  feedUrl?: string | null;
  feedUrlGz?: string | null;
  maxChannels?: number | null;
  maxBodyBytes?: number | null;
};

const DAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"] as const;

export function settingsHtml(): string {
  return `
    <div class="settings-head">
      <div>
        <h1 class="page-title">Settings</h1>
        <p class="page-sub">epg.monster studio · dark theme · bundled tools</p>
      </div>
      <div>
        <button id="detect-tools">Detect bundled tools</button>
        <button class="accent" id="save-settings">Save</button>
      </div>
    </div>
    <p class="page-sub" id="set-status"></p>
    <div class="settings-grid">
      <section class="tile">
        <h2>Players</h2>
        <p class="hint">Play uses mpv or VLC from the paths below. Set those paths if Play does not find a player.</p>
        <div class="field"><label>Default player</label>
          <select id="set-player">${playerEngineOptionsHtml()}</select></div>
        <div class="field"><label id="lbl-mpv">mpv.exe path</label><input id="set-mpv" /></div>
        <div class="field"><label id="lbl-vlc">vlc.exe path</label><input id="set-vlc" /></div>
      </section>
      <section class="tile">
        <h2>Stream Audit</h2>
        <p class="hint">ffmpeg / ffprobe used for probes and the HDHomeRun remux.</p>
        <div class="field"><label id="lbl-ffmpeg">ffmpeg.exe path</label><input id="set-ffmpeg" /></div>
        <div class="field"><label id="lbl-ffprobe">ffprobe.exe path</label><input id="set-ffprobe" /></div>
        <div class="field"><label>Delay between probes (ms)</label><input id="set-delay" type="number" /></div>
        <div class="field"><label>Probe timeout (ms)</label><input id="set-timeout" type="number" /></div>
        <label class="check"><input type="checkbox" id="set-autoswap" /> Auto-swap visible stream to working backup on fail</label>
        <label class="check"><input type="checkbox" id="set-pauseplay" /> Pause auto-audit while external player is active</label>
      </section>
      <section class="tile">
        <h2>Guide</h2>
        <p class="hint">XMLTV catalog. Built from tvg-ids in this file.</p>
        <div class="field"><label>Default User-Agent for URL sources</label><input id="set-ua" /></div>
        <div class="field"><label>XMLTV guide URL (epg.monster)</label>
          <textarea id="set-xml" rows="3"></textarea></div>
      </section>
      <section class="tile">
        <h2>my.epg.monster</h2>
        <p class="hint">Access key from Keys. Upload sends curated tvg-ids only — never stream URLs.</p>
        <div class="field"><label>Email</label><input id="set-email" placeholder="you@example.com" /></div>
        <div class="field"><label>Access key (epgm_…)</label><input id="set-key" type="password" /></div>
        <div class="field"><label>API base</label><input id="set-api" placeholder="https://epg.monster" /></div>
        <div>
          <button id="set-test">Test key</button>
          <button class="accent" id="set-upload">Upload channels.json</button>
        </div>
        <p class="page-sub" id="set-member-status"></p>
        <p class="page-sub" id="set-member-feed" style="user-select:text"></p>
        <p class="page-sub" id="set-member-pub"></p>
      </section>
      <section class="tile">
        <h2>Remux</h2>
        <p class="hint">Spawn ffmpeg or VLC, buffer MPEG-TS, then serve Plex. MPEG2+AC3 is the Plex-safe default. VLC is always copy-to-TS.</p>
        <div class="field"><label>Engine</label>
          <select id="set-reng"><option value="ffmpeg">ffmpeg</option><option value="vlc">VLC</option></select></div>
        <div class="field"><label>ffmpeg profile</label>
          <select id="set-rprof">
            <option value="mpeg2_ac3">Plex MPEG2 + AC3 (recommended)</option>
            <option value="copy_aac">Threadfin copy (H264 + AAC stereo)</option>
          </select></div>
        <div class="field"><label>Buffer before send (KB)</label><input id="set-rbuf" type="number" /></div>
      </section>
      <section class="tile">
        <h2>Logos</h2>
        <p class="hint">Download logos from Logo Audit → Save Logos into this folder. Existing files are skipped unless the download is a different size. Optional hosting on the tuner uses the same pack.</p>
        <div class="field"><label>Logo save directory</label><input id="set-logodir" placeholder="{app}/data/logo" /></div>
        <label class="check"><input type="checkbox" id="set-hostlogos" /> Host the logos folder on the tuner</label>
        <label class="check"><input type="checkbox" id="set-locallogos" /> Use local logos in tuner playlists and EPG</label>
      </section>
      <section class="tile">
        <h2>Weekly Stream Audit</h2>
        <p class="hint">Group names, comma-separated. Stream Audit → Run today's groups. Skip groups with no match.</p>
        <div class="settings-grid">
          <div class="field"><label>Monday</label><input id="set-mon" /></div>
          <div class="field"><label>Tuesday</label><input id="set-tue" /></div>
          <div class="field"><label>Wednesday</label><input id="set-wed" /></div>
          <div class="field"><label>Thursday</label><input id="set-thu" /></div>
          <div class="field"><label>Friday</label><input id="set-fri" /></div>
          <div class="field"><label>Saturday</label><input id="set-sat" /></div>
          <div class="field" style="grid-column:1/-1"><label>Sunday</label><input id="set-sun" /></div>
        </div>
        <label class="check"><input type="checkbox" id="set-weekauto" /> Remind me when today's groups have not run (does not start a probe)</label>
        <label class="check"><input type="checkbox" id="set-black" /> Fail fully black screens (ffmpeg blackdetect)</label>
      </section>
      <section class="tile">
        <h2>Screen matches</h2>
        <p class="hint">After a stream decodes, hash one frame against these stills (offline / slate cards).</p>
        <div id="set-slates" class="editor-list" style="max-height:140px"></div>
        <div>
          <button id="set-slate-add">Add screen…</button>
          <button id="set-slate-del">Remove selected</button>
          <button id="set-slate-open">Open folder</button>
        </div>
        <p class="page-sub" id="set-slate-status"></p>
        <h2 style="margin-top:16px">Diagnostics</h2>
        <p class="hint">Daily logs and crash reports live under local app data. A crash opens a report on the next launch. GitHub releases: nav → Check For Updates.</p>
        <div>
          <button id="set-logs">Open logs folder</button>
          <button id="set-crashes">Open crash reports</button>
        </div>
        <p class="page-sub" id="set-logpath"></p>
        <label class="check"><input type="checkbox" id="set-updates" /> Check for app updates on splash</label>
        <div class="field"><label>Optional Python path</label><input id="set-py" placeholder="python.exe" /></div>
      </section>
    </div>
  `;
}

export async function mountSettings(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  let s: AppSettings | null = null;
  let folders = { logs: "", crashes: "", slates: "", currentLog: "", logoDir: "" };
  let selectedSlate = "";

  const $ = <T extends HTMLElement>(id: string) => page.querySelector<T>(`#${id}`)!;
  const val = (id: string) => (page.querySelector(`#${id}`) as HTMLInputElement).value;
  const setVal = (id: string, v: string) => {
    (page.querySelector(`#${id}`) as HTMLInputElement).value = v;
  };
  const chk = (id: string) => (page.querySelector(`#${id}`) as HTMLInputElement).checked;
  const setChk = (id: string, v: boolean) => {
    (page.querySelector(`#${id}`) as HTMLInputElement).checked = v;
  };

  const feedLabel = (xml?: string | null, gz?: string | null) => {
    const parts = [];
    if (xml) parts.push("XML: " + xml);
    if (gz) parts.push("gzip: " + gz);
    return parts.join("  ·  ");
  };
  const publishLabel = (st: AppSettings) => {
    const parts = [];
    if (st.MemberLastPingAt) parts.push("Last ping: " + st.MemberLastPingAt);
    if (st.MemberLastPublishedAt) parts.push("Last upload: " + st.MemberLastPublishedAt);
    return parts.join("  ·  ");
  };
  const parseWeek = (json: string): Record<string, string[]> => {
    const empty: Record<string, string[]> = {};
    for (const d of DAYS) empty[d] = [];
    if (!json.trim()) return empty;
    try {
      const obj = JSON.parse(json) as Record<string, string[]>;
      for (const d of DAYS) empty[d] = obj[d] ?? [];
    } catch {
      /* keep empty */
    }
    return empty;
  };
  const splitGroups = (text: string) =>
    text
      .split(/[,;\r\n]+/)
      .map((x) => x.trim())
      .filter(Boolean);

  const paintSlates = async () => {
    const names = await invoke<string[]>("list_slates");
    const el = $("set-slates");
    el.innerHTML = "";
    for (const n of names) {
      const b = document.createElement("button");
      b.className = "group-row" + (n === selectedSlate ? " active" : "");
      b.textContent = n;
      b.addEventListener("click", () => {
        selectedSlate = n;
        void paintSlates();
      });
      el.appendChild(b);
    }
    $("set-slate-status").textContent =
      names.length === 0
        ? "No stills yet — add a screenshot of an offline / slate screen."
        : `${names.length} match still(s) in ${folders.slates}`;
  };

  const applyPing = async (key: string, ping: Ping) => {
    if (!s) return;
    s.MemberEmail = val("set-email").trim() || s.MemberEmail;
    if (ping.username) s.MemberUsername = ping.username.trim();
    s.MemberAccessKey = key;
    s.MemberApiBase = val("set-api").trim() || s.MemberApiBase;
    if (ping.feedUrl) s.MemberFeedUrl = ping.feedUrl;
    if (ping.feedUrlGz) s.MemberFeedUrlGz = ping.feedUrlGz;
    if (ping.maxChannels && ping.maxChannels > 0) s.MemberMaxChannels = ping.maxChannels;
    if (ping.maxBodyBytes && ping.maxBodyBytes > 0) s.MemberMaxBodyBytes = ping.maxBodyBytes;
    s.MemberLastPingAt = new Date().toISOString();
    await invoke("save_settings", { settings: s });
    $("set-member-feed").textContent = feedLabel(s.MemberFeedUrl, s.MemberFeedUrlGz);
    $("set-member-pub").textContent = publishLabel(s);
  };

  const collect = (): AppSettings => {
    if (!s) throw new Error("settings not loaded");
    const xmlLines = val("set-xml")
      .split(/[\r\n;]+/)
      .map((u) => u.trim())
      .filter(Boolean);
    const week: Record<string, string[]> = {
      Monday: splitGroups(val("set-mon")),
      Tuesday: splitGroups(val("set-tue")),
      Wednesday: splitGroups(val("set-wed")),
      Thursday: splitGroups(val("set-thu")),
      Friday: splitGroups(val("set-fri")),
      Saturday: splitGroups(val("set-sat")),
      Sunday: splitGroups(val("set-sun")),
    };
    const key = val("set-key").trim() || s.MemberAccessKey;
    return {
      ...s,
      DefaultPlayer: parseInt((page.querySelector("#set-player") as HTMLSelectElement).value, 10) || 0,
      MpvPath: val("set-mpv").trim(),
      VlcPath: val("set-vlc").trim(),
      FfmpegPath: val("set-ffmpeg").trim(),
      FfprobePath: val("set-ffprobe").trim(),
      AuditDelayMs: parseInt(val("set-delay"), 10) || 6000,
      AuditTimeoutMs: parseInt(val("set-timeout"), 10) || 15000,
      AutoSwapOnAuditFail: chk("set-autoswap"),
      PauseAuditWhilePlaying: chk("set-pauseplay"),
      DefaultUserAgent: val("set-ua").trim() || s.DefaultUserAgent,
      EpgShareUrl: "",
      EpgXmlUrl: xmlLines[0] ?? "https://epg.monster/epg.xml",
      EpgXmlUrls: xmlLines,
      PythonPath: val("set-py").trim() || null,
      MemberEmail: val("set-email").trim(),
      MemberAccessKey: key,
      MemberApiBase: val("set-api").trim() || "https://epg.monster",
      WeeklyAuditJson: JSON.stringify(week),
      WeeklyAuditAutoRun: chk("set-weekauto"),
      BlackDetectEnabled: chk("set-black"),
      CheckForAppUpdates: chk("set-updates"),
      LogoSaveDirectory: val("set-logodir").trim() || folders.logoDir,
      HostLogosOnTuner: chk("set-hostlogos") || chk("set-locallogos"),
      UseLocalLogos: chk("set-locallogos"),
      RemuxEngine: (page.querySelector("#set-reng") as HTMLSelectElement).value,
      RemuxProfile: (page.querySelector("#set-rprof") as HTMLSelectElement).value,
      RemuxBufferKb: parseInt(val("set-rbuf"), 10) || 2048,
    };
  };

  const fill = (st: AppSettings) => {
    applyPlayerEngineValue(
      page.querySelector("#set-player") as HTMLSelectElement,
      st.DefaultPlayer ?? 2,
    );
    setVal("set-mpv", st.MpvPath ?? "");
    setVal("set-vlc", st.VlcPath ?? "");
    setVal("set-ffmpeg", st.FfmpegPath ?? "");
    setVal("set-ffprobe", st.FfprobePath ?? "");
    setVal("set-delay", String(st.AuditDelayMs ?? 6000));
    setVal("set-timeout", String(st.AuditTimeoutMs ?? 15000));
    setChk("set-autoswap", st.AutoSwapOnAuditFail !== false);
    setChk("set-pauseplay", st.PauseAuditWhilePlaying !== false);
    setVal("set-ua", st.DefaultUserAgent ?? "");
    const urls = st.EpgXmlUrls && st.EpgXmlUrls.length ? st.EpgXmlUrls : [st.EpgXmlUrl ?? ""];
    (page.querySelector("#set-xml") as HTMLTextAreaElement).value = urls.join("\n");
    setVal("set-email", st.MemberEmail ?? "");
    setVal("set-key", st.MemberAccessKey ?? "");
    setVal("set-api", st.MemberApiBase || "https://epg.monster");
    $("set-member-status").textContent = st.MemberAccessKey ? "Key saved locally (not logged)" : "No key saved";
    $("set-member-feed").textContent = feedLabel(st.MemberFeedUrl, st.MemberFeedUrlGz);
    $("set-member-pub").textContent = publishLabel(st);
    ($("set-upload") as HTMLButtonElement).disabled = !st.MemberAccessKey;
    (page.querySelector("#set-reng") as HTMLSelectElement).value = st.RemuxEngine === "vlc" ? "vlc" : "ffmpeg";
    (page.querySelector("#set-rprof") as HTMLSelectElement).value =
      st.RemuxProfile === "copy_aac" ? "copy_aac" : "mpeg2_ac3";
    setVal("set-rbuf", String(st.RemuxBufferKb || 2048));
    const week = parseWeek(st.WeeklyAuditJson ?? "");
    setVal("set-mon", week.Monday.join(", "));
    setVal("set-tue", week.Tuesday.join(", "));
    setVal("set-wed", week.Wednesday.join(", "));
    setVal("set-thu", week.Thursday.join(", "));
    setVal("set-fri", week.Friday.join(", "));
    setVal("set-sat", week.Saturday.join(", "));
    setVal("set-sun", week.Sunday.join(", "));
    setChk("set-weekauto", !!st.WeeklyAuditAutoRun);
    setChk("set-black", !!st.BlackDetectEnabled);
    setChk("set-updates", !!st.CheckForAppUpdates);
    setVal("set-logodir", st.LogoSaveDirectory || folders.logoDir);
    setChk("set-hostlogos", !!st.HostLogosOnTuner);
    setChk("set-locallogos", !!st.UseLocalLogos);
    setVal("set-py", st.PythonPath ?? "");
  };

  page.querySelector("#save-settings")!.addEventListener("click", async () => {
    try {
      const next = collect();
      await invoke("save_settings", { settings: next });
      await loadStudioCaps();
      s = next;
      ($("set-upload") as HTMLButtonElement).disabled = !next.MemberAccessKey;
      if (next.MemberAccessKey) {
        $("set-status").textContent = "Saved. Checking access key…";
        const ping = await invoke<Ping>("members_ping", {
          apiBase: next.MemberApiBase,
          accessKey: next.MemberAccessKey,
        });
        if (ping.ok) {
          await applyPing(next.MemberAccessKey, ping);
          $("set-status").textContent = "Saved. " + ping.message;
          toast("Settings saved · key OK");
          return;
        }
        $("set-status").textContent = "Saved. " + ping.message;
        toast(ping.message);
        return;
      }
      $("set-status").textContent = "Saved.";
      toast("Settings saved");
    } catch (e) {
      toast(String(e));
    }
  });

  page.querySelector("#detect-tools")!.addEventListener("click", async () => {
    try {
      const p = await invoke<{ mpv: string; vlc: string; ffmpeg: string; ffprobe: string }>("detect_tool_paths");
      setVal("set-mpv", p.mpv);
      setVal("set-vlc", p.vlc);
      setVal("set-ffmpeg", p.ffmpeg);
      setVal("set-ffprobe", p.ffprobe);
      $("set-status").textContent = "Detected bundled / common install paths. Save to apply.";
    } catch (e) {
      toast(String(e));
    }
  });

  page.querySelector("#set-test")!.addEventListener("click", async () => {
    const key = val("set-key").trim() || s?.MemberAccessKey || "";
    $("set-member-status").textContent = "Testing…";
    let ping: Ping;
    try {
      ping = await invoke<Ping>("members_ping", { apiBase: val("set-api"), accessKey: key });
    } catch (e) {
      $("set-member-status").textContent = String(e);
      toast(String(e));
      return;
    }
    $("set-member-status").textContent = ping.message;
    if (ping.ok) {
      if (ping.email && !val("set-email").trim()) setVal("set-email", ping.email);
      await applyPing(key, ping);
    }
    toast(ping.ok ? "Access key OK" : ping.message);
  });

  page.querySelector("#set-upload")!.addEventListener("click", async () => {
    const key = val("set-key").trim() || s?.MemberAccessKey || "";
    if (!key) {
      $("set-member-status").textContent = "Paste an access key first, then Test, then Upload.";
      toast($("set-member-status").textContent);
      return;
    }
    ($("set-upload") as HTMLButtonElement).disabled = true;
    $("set-member-status").textContent = "Uploading full curated list…";
    try {
      await invoke("save_settings", { settings: collect() });
      const r = await invoke<{ ok: boolean; text: string }>("publish_channels");
      $("set-member-status").textContent = r.ok ? "Uploaded." : r.text;
      toast(r.ok ? r.text.split("\n")[0] : r.text);
      s = await invoke<AppSettings>("load_settings");
      $("set-member-feed").textContent = feedLabel(s.MemberFeedUrl, s.MemberFeedUrlGz);
      $("set-member-pub").textContent = publishLabel(s);
    } catch (e) {
      toast(String(e));
    } finally {
      ($("set-upload") as HTMLButtonElement).disabled = false;
    }
  });

  page.querySelector("#set-slate-add")!.addEventListener("click", async () => {
    const name = await invoke<string>("add_slate");
    if (name !== "cancelled") {
      toast("Screen match added: " + name);
      await paintSlates();
    }
  });
  page.querySelector("#set-slate-del")!.addEventListener("click", async () => {
    if (!selectedSlate) {
      toast("Select a still first.");
      return;
    }
    await invoke("remove_slate", { name: selectedSlate });
    toast("Removed " + selectedSlate);
    selectedSlate = "";
    await paintSlates();
  });
  page.querySelector("#set-slate-open")!.addEventListener("click", () => void invoke("open_folder", { path: folders.slates }));
  page.querySelector("#set-logs")!.addEventListener("click", () => void invoke("open_folder", { path: folders.logs }));
  page.querySelector("#set-crashes")!.addEventListener("click", () => void invoke("open_folder", { path: folders.crashes }));

  try {
    const host = await invoke<{ os: string; exeSuffix: string }>("host_info");
    const win = host.os === "windows";
    $("lbl-mpv").textContent = win ? "mpv.exe path" : "mpv path";
    $("lbl-vlc").textContent = win ? "vlc.exe path" : "vlc path";
    $("lbl-ffmpeg").textContent = win ? "ffmpeg.exe path" : "ffmpeg path";
    $("lbl-ffprobe").textContent = win ? "ffprobe.exe path" : "ffprobe path";
    ($("set-mpv") as HTMLInputElement).placeholder = win ? "mpv.exe" : "mpv";
    ($("set-vlc") as HTMLInputElement).placeholder = win ? "vlc.exe" : "vlc";
    ($("set-ffmpeg") as HTMLInputElement).placeholder = win ? "ffmpeg.exe" : "ffmpeg";
    ($("set-ffprobe") as HTMLInputElement).placeholder = win ? "ffprobe.exe" : "ffprobe";
    ($("set-py") as HTMLInputElement).placeholder = win ? "python.exe" : "python3";
  } catch {
    /* labels stay Windows-shaped */
  }

  try {
    folders = await invoke("settings_folders");
    $("set-logpath").textContent = `Log: ${folders.currentLog}`;
    s = await invoke<AppSettings>("load_settings");
    fill(s);
    await paintSlates();
  } catch (e) {
    toast(String(e));
  }
}
