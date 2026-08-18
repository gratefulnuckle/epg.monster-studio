import { invoke } from "@tauri-apps/api/core";

type TunerProfile = {
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

type AppSettings = {
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
        <p class="hint">External player used from Playlist Editor and Stream.</p>
        <div class="field"><label>Default player</label>
          <select id="set-player"><option value="0">mpv</option><option value="1">VLC</option></select></div>
        <div class="field"><label>mpv.exe path</label><input id="set-mpv" /></div>
        <div class="field"><label>vlc.exe path</label><input id="set-vlc" /></div>
      </section>
      <section class="tile">
        <h2>Stream Audit</h2>
        <p class="hint">ffmpeg / ffprobe used for probes and the HDHomeRun remux.</p>
        <div class="field"><label>ffmpeg.exe path</label><input id="set-ffmpeg" /></div>
        <div class="field"><label>ffprobe.exe path</label><input id="set-ffprobe" /></div>
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
      <section class="tile" style="grid-column:1/-1">
        <h2>TV Tuner</h2>
        <p class="hint">IPTV is on for new installs. Plex, Jellyfin, and Emby stay off until you enable them. Ports 8080–8083. Start/stop is on the TV Tuner panel.</p>
        <div class="settings-grid">
          ${tunerCardHtml("plex", "Plex")}
          ${tunerCardHtml("jelly", "Jellyfin", true)}
          ${tunerCardHtml("emby", "Emby")}
          ${tunerCardHtml("iptv", "IPTV (TiviMate / Smarters)", false, true)}
        </div>
        <label class="check"><input type="checkbox" id="set-disco" /> Advertise tuners on the network (HDHomeRun UDP 65001 + SSDP). Turn on Allow LAN if Plex is another PC.</label>
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
        <p class="hint">Local PNG pack and optional hosting on the tuner.</p>
        <div class="field"><label>Logo save directory</label><input id="set-logodir" placeholder="%LocalAppData%\\epg.monster-studio\\logo" /></div>
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
        <p class="hint">Daily logs and crash reports live under local app data. A crash opens a report on the next launch.</p>
        <div>
          <button id="set-logs">Open logs folder</button>
          <button id="set-crashes">Open crash reports</button>
        </div>
        <p class="page-sub" id="set-logpath"></p>
        <div class="field"><label>Optional Python path</label><input id="set-py" placeholder="python.exe" /></div>
      </section>
    </div>
  `;
}

function tunerCardHtml(id: string, title: string, downspiral = false, iptv = false): string {
  return `
    <section class="tile">
      <label class="check"><input type="checkbox" id="${id}-on" /> ${title}</label>
      <div class="field"><label>Friendly name</label><input id="${id}-name" /></div>
      <div class="field"><label>Port</label><input id="${id}-port" type="number" /></div>
      <div class="field"><label>Tuner count</label><input id="${id}-count" type="number" /></div>
      <label class="check"><input type="checkbox" id="${id}-lan" /> Allow LAN</label>
      ${downspiral ? `<label class="check"><input type="checkbox" id="${id}-down" /> Downspiral — one playlist + guide per group (switch lists without changing Jellyfin profiles)</label>` : ""}
      ${iptv ? `
        <label class="check"><input type="checkbox" id="${id}-remux" /> Remux IPTV playlist through Studio (MPEG-TS)</label>
        <div class="field"><label>Tuner EPG for IPTV players</label>
          <select id="set-epgsrc">
            <option value="0">Local Studio guide (/guide.xml)</option>
            <option value="1">my.epg.monster curated feed</option>
          </select></div>
        <p class="page-sub" id="set-epghint"></p>` : ""}
      <p class="page-sub" id="${id}-urls" style="user-select:text"></p>
    </section>
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
  const tunerHelp = (p: TunerProfile, st: AppSettings) => {
    const root = `http://${p.AllowLan && (p.BindAddress === "0.0.0.0" || p.BindAddress === "*" || p.BindAddress === "+") ? "127.0.0.1" : p.BindAddress || "127.0.0.1"}:${p.Port}`;
    const epg = st.TunerUseMemberEpg && (st.MemberFeedUrlGz || st.MemberFeedUrl)
      ? (st.MemberFeedUrlGz || st.MemberFeedUrl)
      : `${root}/guide.xml`;
    const lines = [`Device ID: ${p.DeviceId} (stable)`, `Tuner: ${root}`, `EPG: ${epg}`];
    if (st.HostLogosOnTuner || st.UseLocalLogos) lines.push(`Logos: ${root}/logos/{tvg-id}.png`);
    if (p.Kind === "Jellyfin" || p.Kind === "Iptv") {
      lines.push(`Playlist: ${root}/playlist.m3u8`);
      lines.push(`M3U: ${root}/tuner.m3u`);
    }
    if (p.Kind === "Jellyfin" && p.DownspiralEnabled) {
      lines.push(`Downspiral index: ${root}/downspiral/index.json`);
      lines.push(`Per-group: ${root}/downspiral/{group}.m3u8 + .xml`);
    }
    if (!p.Enabled) lines.push("Disabled — turn on to start this tuner.");
    return lines.join("\n");
  };

  const paintTunerUrls = () => {
    if (!s) return;
    $("plex-urls").textContent = tunerHelp(s.PlexTuner, s);
    $("jelly-urls").textContent = tunerHelp(s.JellyfinTuner, s);
    $("emby-urls").textContent = tunerHelp(s.EmbyTuner, s);
    $("iptv-urls").textContent = tunerHelp(s.IptvTuner, s);
  };

  const loadTuner = (id: string, p: TunerProfile) => {
    setChk(`${id}-on`, p.Enabled);
    setVal(`${id}-name`, p.FriendlyName);
    setVal(`${id}-port`, String(p.Port));
    setVal(`${id}-count`, String(p.TunerCount));
    setChk(`${id}-lan`, p.AllowLan);
  };
  const readTuner = (id: string, existing: TunerProfile, kind: string): TunerProfile => {
    const enabled = chk(`${id}-on`);
    return {
      ...existing,
      Kind: kind,
      Enabled: enabled,
      Running: enabled ? existing.Running : false,
      FriendlyName: val(`${id}-name`).trim() || existing.FriendlyName,
      Port: parseInt(val(`${id}-port`), 10) || existing.Port,
      TunerCount: parseInt(val(`${id}-count`), 10) || existing.TunerCount,
      AllowLan: chk(`${id}-lan`),
    };
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
    const jelly = readTuner("jelly", s.JellyfinTuner, "Jellyfin");
    jelly.DownspiralEnabled = chk("jelly-down");
    const iptv = readTuner("iptv", s.IptvTuner, "Iptv");
    iptv.RemuxEnabled = chk("iptv-remux");
    const key = val("set-key").trim() || s.MemberAccessKey;
    let useMember = (page.querySelector("#set-epgsrc") as HTMLSelectElement).value === "1";
    if (useMember && !s.MemberFeedUrl && !s.MemberFeedUrlGz && !key) useMember = false;
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
      PlexTuner: readTuner("plex", s.PlexTuner, "Plex"),
      JellyfinTuner: jelly,
      EmbyTuner: readTuner("emby", s.EmbyTuner, "Emby"),
      IptvTuner: iptv,
      MemberEmail: val("set-email").trim(),
      MemberAccessKey: key,
      MemberApiBase: val("set-api").trim() || "https://epg.monster",
      TunerUseMemberEpg: useMember,
      DiscoveryEnabled: chk("set-disco"),
      WeeklyAuditJson: JSON.stringify(week),
      WeeklyAuditAutoRun: chk("set-weekauto"),
      BlackDetectEnabled: chk("set-black"),
      LogoSaveDirectory: val("set-logodir").trim() || folders.logoDir,
      HostLogosOnTuner: chk("set-hostlogos") || chk("set-locallogos"),
      UseLocalLogos: chk("set-locallogos"),
      RemuxEngine: (page.querySelector("#set-reng") as HTMLSelectElement).value,
      RemuxProfile: (page.querySelector("#set-rprof") as HTMLSelectElement).value,
      RemuxBufferKb: parseInt(val("set-rbuf"), 10) || 4096,
    };
  };

  const fill = (st: AppSettings) => {
    (page.querySelector("#set-player") as HTMLSelectElement).value = String(st.DefaultPlayer ?? 0);
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
    loadTuner("plex", st.PlexTuner);
    loadTuner("jelly", st.JellyfinTuner);
    loadTuner("emby", st.EmbyTuner);
    loadTuner("iptv", st.IptvTuner);
    setChk("jelly-down", !!st.JellyfinTuner.DownspiralEnabled);
    setChk("iptv-remux", st.IptvTuner.RemuxEnabled !== false);
    setChk("set-disco", st.DiscoveryEnabled !== false);
    (page.querySelector("#set-reng") as HTMLSelectElement).value = st.RemuxEngine === "vlc" ? "vlc" : "ffmpeg";
    (page.querySelector("#set-rprof") as HTMLSelectElement).value =
      st.RemuxProfile === "copy_aac" ? "copy_aac" : "mpeg2_ac3";
    setVal("set-rbuf", String(st.RemuxBufferKb || 4096));
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
    setVal("set-logodir", st.LogoSaveDirectory || folders.logoDir);
    setChk("set-hostlogos", !!st.HostLogosOnTuner);
    setChk("set-locallogos", !!st.UseLocalLogos);
    const hasFeed = !!(st.MemberFeedUrlGz || st.MemberFeedUrl);
    (page.querySelector("#set-epgsrc") as HTMLSelectElement).value = st.TunerUseMemberEpg && hasFeed ? "1" : "0";
    $("set-epghint").textContent = hasFeed
      ? "Curated feed: " + (st.MemberFeedUrlGz || st.MemberFeedUrl)
      : "Upload channels.json first to use the my.epg.monster feed as tuner EPG.";
    setVal("set-py", st.PythonPath ?? "");
    paintTunerUrls();
  };

  page.querySelector("#save-settings")!.addEventListener("click", async () => {
    try {
      const next = collect();
      if (next.TunerUseMemberEpg && !next.MemberFeedUrl && !next.MemberFeedUrlGz) {
        next.TunerUseMemberEpg = false;
        (page.querySelector("#set-epgsrc") as HTMLSelectElement).value = "0";
        $("set-epghint").textContent = "Upload channels.json first to use the my.epg.monster feed as tuner EPG.";
      }
      await invoke("save_settings", { settings: next });
      s = next;
      paintTunerUrls();
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
    const p = await invoke<{ mpv: string; vlc: string; ffmpeg: string; ffprobe: string }>("detect_tool_paths");
    setVal("set-mpv", p.mpv);
    setVal("set-vlc", p.vlc);
    setVal("set-ffmpeg", p.ffmpeg);
    setVal("set-ffprobe", p.ffprobe);
    $("set-status").textContent = "Detected bundled / common install paths.";
  });

  page.querySelector("#set-test")!.addEventListener("click", async () => {
    const key = val("set-key").trim() || s?.MemberAccessKey || "";
    $("set-member-status").textContent = "Testing…";
    const ping = await invoke<Ping>("members_ping", { apiBase: val("set-api"), accessKey: key });
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
    folders = await invoke("settings_folders");
    $("set-logpath").textContent = `Log: ${folders.currentLog}`;
    s = await invoke<AppSettings>("load_settings");
    fill(s);
    await paintSlates();
  } catch (e) {
    toast(String(e));
  }
}
