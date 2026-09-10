import { invoke } from "@tauri-apps/api/core";
import { applyPlayerEngineValue, loadStudioCaps, playerEngineOptionsHtml } from "./capabilities";
import {
  connectRemote,
  disconnectRemote,
  isRemoteConnected,
  isWebHost,
  loadClient,
} from "./bridge/client";
import { invokeLocal } from "./bridge/core";
import { notifyPhysicalSave } from "./save-status";

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

type ApiKeyInfo = {
  id: string;
  name: string;
  prefix: string;
  created: number;
  lastUsed?: number | null;
};

const DAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"] as const;
const SAVE_MS = 500;

export function settingsHtml(): string {
  const web = isWebHost();
  const remote = isRemoteConnected();
  const machineLabel = web ? "This host" : "This computer";
  const showAccount = web || remote;
  const showConnect = !web;
  return `
    <div class="settings-shell">
      <h1 class="page-title">Settings</h1>
      <p class="page-sub" id="set-sub">${web ? "This studio host." : remote ? "This computer, talking to a remote studio." : "This computer’s studio."}</p>
      <div class="settings-tabs" id="set-tabs">
        <button type="button" class="tab active" data-set-tab="machine">${machineLabel}</button>
        <button type="button" class="tab" data-set-tab="studio">Studio</button>
        ${showAccount ? `<button type="button" class="tab" data-set-tab="account">Account</button>` : ""}
        <button type="button" class="tab" data-set-tab="advanced">Advanced</button>
      </div>
      <p class="page-sub" id="set-status"></p>
      <div class="settings-body">
        <div class="settings-pane" data-set-pane="machine">
          ${
            showConnect
              ? `
          <section class="tile">
            <h2>Remote studio</h2>
            <p class="hint">The remote host is the live studio. This computer keeps player paths. Generate a key on the server (Settings → Account, or <code>studio.ps1 --makekey</code>).</p>
            <div class="field"><label>Server URL</label><input id="set-remote-url" placeholder="http://192.168.1.10:1420" /></div>
            <div class="field"><label>API key</label><input id="set-remote-key" type="password" placeholder="epgs_…" autocomplete="off" /></div>
            <div class="settings-actions">
              <button type="button" class="accent" id="set-remote-go">${remote ? "Reconnect" : "Connect"}</button>
              ${remote ? `<button type="button" id="set-remote-off">Disconnect</button>` : ""}
            </div>
            <p class="page-sub" id="set-remote-status"></p>
          </section>`
              : ""
          }
          <section class="tile">
            <h2>${web ? "Play" : "Players"}</h2>
            <p class="hint">${
              web
                ? "Play opens in this browser. The server does not spawn mpv or VLC."
                : remote
                  ? "Play on this computer uses mpv or VLC. Stream audit tools live on the host (Studio tab)."
                  : "Play uses mpv or VLC from the paths below."
            }</p>
            ${
              web
                ? ""
                : `
            <div class="field"><label>Default player</label>
              <select id="set-player">${playerEngineOptionsHtml()}</select></div>
            <div class="field"><label id="lbl-mpv">mpv path</label><input id="set-mpv" /></div>
            <div class="field"><label id="lbl-vlc">vlc path</label><input id="set-vlc" /></div>`
            }
            ${
              remote
                ? ""
                : `
            <div class="field"><label id="lbl-ffmpeg">ffmpeg path</label><input id="set-ffmpeg" /></div>
            <div class="field"><label id="lbl-ffprobe">ffprobe path</label><input id="set-ffprobe" /></div>`
            }
            <div class="settings-actions">
              <button type="button" id="detect-tools">Detect bundled tools</button>
            </div>
          </section>
          ${
            web
              ? ""
              : `
          <section class="tile">
            <h2>App</h2>
            <label class="check"><input type="checkbox" id="set-updates" /> Check for app updates on splash</label>
          </section>`
          }
        </div>
        <div class="settings-pane" hidden data-set-pane="studio">
          <section class="tile">
            <h2>Guide</h2>
            <p class="hint">XMLTV catalog. Built from tvg-ids in this file. One URL per line.</p>
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
            <p class="page-sub" id="set-api-warn" hidden>This host is not epg.monster / my.epg.monster / localhost. Saving or testing will send your access key there.</p>
            <div class="settings-actions">
              <button type="button" id="set-test">Test key</button>
              <button type="button" class="accent" id="set-upload">Upload channels.json</button>
            </div>
            <p class="page-sub" id="set-member-status"></p>
            <p class="page-sub" id="set-member-feed" style="user-select:text"></p>
            <p class="page-sub" id="set-member-pub"></p>
          </section>
          <section class="tile">
            <h2>Stream Audit</h2>
            <p class="hint">ffmpeg / ffprobe used for probes and the HDHomeRun remux${remote ? " on the studio host" : ""}.</p>
            ${
              remote
                ? `
            <div class="field"><label id="lbl-ffmpeg">ffmpeg path (host)</label><input id="set-ffmpeg" /></div>
            <div class="field"><label id="lbl-ffprobe">ffprobe path (host)</label><input id="set-ffprobe" /></div>
            <div class="settings-actions"><button type="button" id="detect-host-tools">Detect host tools</button></div>`
                : ""
            }
            <div class="field"><label>Delay between probes (ms)</label><input id="set-delay" type="number" min="0" max="120000" /></div>
            <div class="field"><label>Probe timeout (ms)</label><input id="set-timeout" type="number" min="1000" max="120000" /></div>
            <label class="check"><input type="checkbox" id="set-autoswap" /> Auto-swap visible stream to working backup on fail</label>
            <label class="check"><input type="checkbox" id="set-pauseplay" /> Pause auto-audit while a player is active</label>
          </section>
          <section class="tile">
            <h2>Remux</h2>
            <p class="hint">Spawn ffmpeg or VLC, buffer MPEG-TS, then serve Plex. MPEG2+AC3 is the Plex-safe default.</p>
            <div class="field"><label>Engine</label>
              <select id="set-reng"><option value="ffmpeg">ffmpeg</option><option value="vlc">VLC</option></select></div>
            <div class="field"><label>ffmpeg profile</label>
              <select id="set-rprof">
                <option value="mpeg2_ac3">Plex MPEG2 + AC3 (recommended)</option>
                <option value="copy_aac">Threadfin copy (H264 + AAC stereo)</option>
              </select></div>
            <div class="field"><label>Buffer before send (KB)</label><input id="set-rbuf" type="number" min="512" max="16384" /></div>
          </section>
          <section class="tile">
            <h2>Logos</h2>
            <p class="hint">Logo Audit → Save Logos writes here. Existing files are skipped unless the download is a different size.</p>
            <div class="field"><label>Logo save directory</label><input id="set-logodir" placeholder="{app}/data/logo" /></div>
            <label class="check"><input type="checkbox" id="set-hostlogos" /> Host the logos folder on the tuner</label>
            <label class="check"><input type="checkbox" id="set-locallogos" /> Use local logos in tuner playlists and EPG</label>
            <label class="check"><input type="checkbox" id="set-cachelogos" /> Cache a local PNG copy when saving logos</label>
          </section>
          <section class="tile">
            <h2>Weekly Stream Audit</h2>
            <p class="hint">Group names, comma-separated. Stream Audit → Run today’s groups. Skip groups with no match.</p>
            <div class="week-grid">
              <div class="field"><label>Monday</label><input id="set-mon" /></div>
              <div class="field"><label>Tuesday</label><input id="set-tue" /></div>
              <div class="field"><label>Wednesday</label><input id="set-wed" /></div>
              <div class="field"><label>Thursday</label><input id="set-thu" /></div>
              <div class="field"><label>Friday</label><input id="set-fri" /></div>
              <div class="field"><label>Saturday</label><input id="set-sat" /></div>
              <div class="field"><label>Sunday</label><input id="set-sun" /></div>
            </div>
            <label class="check"><input type="checkbox" id="set-weekauto" /> Remind me when today’s groups have not run (does not start a probe)</label>
            <label class="check"><input type="checkbox" id="set-black" /> Fail fully black screens (ffmpeg blackdetect)</label>
          </section>
          <section class="tile">
            <h2>Screen matches</h2>
            <p class="hint">After a stream decodes, hash one frame against these stills (offline / slate cards).</p>
            <div id="set-slates" class="editor-list" style="max-height:140px"></div>
            <div class="settings-actions">
              <button type="button" id="set-slate-add">Add screen…</button>
              <button type="button" id="set-slate-del">Remove selected</button>
              <button type="button" id="set-slate-open">Open folder</button>
            </div>
            <p class="page-sub" id="set-slate-status"></p>
          </section>
        </div>
        <div class="settings-pane" hidden data-set-pane="account">
          ${
            web
              ? `
          <section class="tile">
            <h2>Password</h2>
            <p class="hint">Changes the admin password for this web host.</p>
            <div class="field"><label>Current password</label><input id="set-pw-cur" type="password" autocomplete="current-password" /></div>
            <div class="field"><label>New password</label><input id="set-pw-next" type="password" autocomplete="new-password" /></div>
            <div class="field"><label>Confirm new password</label><input id="set-pw-next2" type="password" autocomplete="new-password" /></div>
            <div class="settings-actions">
              <button type="button" class="accent" id="set-pw-save">Change password</button>
              <button type="button" id="set-logout">Sign out</button>
            </div>
            <p class="page-sub" id="set-pw-status"></p>
          </section>`
              : ""
          }
          <section class="tile">
            <h2>Desktop API keys</h2>
            <p class="hint">A desktop client pastes a key into Settings → This computer and hits Connect. The secret is shown once.</p>
            <div class="field"><label>Key name</label><input id="set-key-name" placeholder="Living room PC" /></div>
            <div class="settings-actions">
              <button type="button" class="accent" id="set-key-make">Generate API key</button>
            </div>
            <div id="set-key-list" class="key-list"></div>
            <p class="page-sub" id="set-key-status"></p>
          </section>
        </div>
        <div class="settings-pane" hidden data-set-pane="advanced">
          <section class="tile">
            <h2>Diagnostics</h2>
            <p class="hint">Daily logs and crash reports. A crash opens a report on the next launch.</p>
            <div class="settings-actions">
              <button type="button" id="set-logs">Open logs folder</button>
              <button type="button" id="set-crashes">Open crash reports</button>
            </div>
            <p class="page-sub" id="set-logpath"></p>
            ${web ? `<label class="check"><input type="checkbox" id="set-updates" /> Check for app updates on splash</label>` : ""}
            <div class="field"><label>Optional Python path</label><input id="set-py" placeholder="python3" /></div>
          </section>
        </div>
      </div>
    </div>
    <div class="dialog-backdrop" id="set-api-dlg">
      <div class="dialog">
        <h2>Unofficial members API host</h2>
        <p class="page-sub" id="set-api-dlg-msg"></p>
        <div class="dialog-actions">
          <button type="button" id="set-api-no">Cancel</button>
          <button type="button" class="accent" id="set-api-yes">Save anyway</button>
        </div>
      </div>
    </div>
    <div class="dialog-backdrop" id="set-key-dlg">
      <div class="dialog">
        <h2>API key created</h2>
        <p class="page-sub">Copy this now. Studio will not show it again.</p>
        <input id="set-key-once" readonly style="user-select:text" />
        <div class="dialog-actions">
          <button type="button" id="set-key-copy">Copy</button>
          <button type="button" class="accent" id="set-key-dlg-ok">Done</button>
        </div>
      </div>
    </div>
  `;
}

export async function mountSettings(page: HTMLElement, toast: (s: string) => void): Promise<void> {
  let s: AppSettings | null = null;
  let localS: AppSettings | null = null;
  let folders = { logs: "", crashes: "", slates: "", currentLog: "", logoDir: "" };
  let selectedSlate = "";
  let saveTimer = 0;
  let ignore = true;
  const remote = isRemoteConnected();

  const officialMemberHost = (base: string) => {
    try {
      const raw = base.trim() || "https://epg.monster";
      const u = new URL(/^[a-z][a-z0-9+.-]*:/i.test(raw) ? raw : `https://${raw}`);
      const h = u.hostname.toLowerCase();
      return h === "epg.monster" || h === "my.epg.monster" || h === "localhost" || h === "127.0.0.1" || h === "::1";
    } catch {
      return false;
    }
  };
  const paintApiWarn = () => {
    const warn = page.querySelector<HTMLElement>("#set-api-warn");
    if (warn) warn.hidden = officialMemberHost(val("set-api"));
  };
  const $ = <T extends HTMLElement>(id: string) => page.querySelector<T>(`#${id}`)!;
  const has = (id: string) => !!page.querySelector(`#${id}`);
  const val = (id: string) => (page.querySelector(`#${id}`) as HTMLInputElement | null)?.value ?? "";
  const setVal = (id: string, v: string) => {
    const el = page.querySelector(`#${id}`) as HTMLInputElement | null;
    if (el) el.value = v;
  };
  const chk = (id: string) => !!(page.querySelector(`#${id}`) as HTMLInputElement | null)?.checked;
  const setChk = (id: string, v: boolean) => {
    const el = page.querySelector(`#${id}`) as HTMLInputElement | null;
    if (el) el.checked = v;
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
    notifyPhysicalSave();
    $("set-member-feed").textContent = feedLabel(s.MemberFeedUrl, s.MemberFeedUrlGz);
    $("set-member-pub").textContent = publishLabel(s);
  };

  const collectStudio = (base: AppSettings): AppSettings => {
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
    const key = val("set-key").trim() || base.MemberAccessKey;
    return {
      ...base,
      FfmpegPath: val("set-ffmpeg").trim(),
      FfprobePath: val("set-ffprobe").trim(),
      AuditDelayMs: parseInt(val("set-delay"), 10) || 6000,
      AuditTimeoutMs: parseInt(val("set-timeout"), 10) || 15000,
      AutoSwapOnAuditFail: chk("set-autoswap"),
      PauseAuditWhilePlaying: chk("set-pauseplay"),
      DefaultUserAgent: val("set-ua").trim() || base.DefaultUserAgent,
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
      LogoSaveDirectory: val("set-logodir").trim() || folders.logoDir,
      HostLogosOnTuner: chk("set-hostlogos") || chk("set-locallogos"),
      UseLocalLogos: chk("set-locallogos"),
      CacheLogos: chk("set-cachelogos"),
      RemuxEngine: (page.querySelector("#set-reng") as HTMLSelectElement).value,
      RemuxProfile: (page.querySelector("#set-rprof") as HTMLSelectElement).value,
      RemuxBufferKb: parseInt(val("set-rbuf"), 10) || 2048,
    };
  };

  const collectPlayers = (base: AppSettings): AppSettings => {
    const next = { ...base };
    if (has("set-player")) {
      next.DefaultPlayer = parseInt((page.querySelector("#set-player") as HTMLSelectElement).value, 10) || 0;
    }
    if (has("set-mpv")) next.MpvPath = val("set-mpv").trim();
    if (has("set-vlc")) next.VlcPath = val("set-vlc").trim();
    if (!remote) {
      next.FfmpegPath = val("set-ffmpeg").trim();
      next.FfprobePath = val("set-ffprobe").trim();
    }
    if (has("set-updates")) next.CheckForAppUpdates = chk("set-updates");
    return next;
  };

  const fillStudio = (st: AppSettings) => {
    if (!remote) {
      setVal("set-ffmpeg", st.FfmpegPath ?? "");
      setVal("set-ffprobe", st.FfprobePath ?? "");
    } else {
      setVal("set-ffmpeg", st.FfmpegPath ?? "");
      setVal("set-ffprobe", st.FfprobePath ?? "");
    }
    setVal("set-delay", String(st.AuditDelayMs ?? 6000));
    setVal("set-timeout", String(st.AuditTimeoutMs ?? 15000));
    setChk("set-autoswap", st.AutoSwapOnAuditFail !== false);
    setChk("set-pauseplay", st.PauseAuditWhilePlaying !== false);
    setVal("set-ua", st.DefaultUserAgent ?? "");
    const urls = st.EpgXmlUrls && st.EpgXmlUrls.length ? st.EpgXmlUrls : [st.EpgXmlUrl ?? ""];
    const xml = page.querySelector("#set-xml") as HTMLTextAreaElement | null;
    if (xml) xml.value = urls.join("\n");
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
    setVal("set-logodir", st.LogoSaveDirectory || folders.logoDir);
    setChk("set-hostlogos", !!st.HostLogosOnTuner);
    setChk("set-locallogos", !!st.UseLocalLogos);
    setChk("set-cachelogos", !!st.CacheLogos);
    setVal("set-py", st.PythonPath ?? "");
    paintApiWarn();
  };

  const fillPlayers = (st: AppSettings) => {
    applyPlayerEngineValue(page.querySelector("#set-player") as HTMLSelectElement, st.DefaultPlayer ?? 0);
    setVal("set-mpv", st.MpvPath ?? "");
    setVal("set-vlc", st.VlcPath ?? "");
    if (!remote) {
      setVal("set-ffmpeg", st.FfmpegPath ?? "");
      setVal("set-ffprobe", st.FfprobePath ?? "");
    }
    if (has("set-updates")) setChk("set-updates", !!st.CheckForAppUpdates);
  };

  const doSave = async () => {
    if (!s) return;
    try {
      if (remote) {
        const hostNext = collectStudio(s);
        await invoke("save_settings", { settings: hostNext });
        s = hostNext;
        const loc = localS ?? hostNext;
        const playerNext = collectPlayers(loc);
        await invokeLocal("save_settings", { settings: playerNext });
        localS = playerNext;
      } else {
        const next = collectPlayers(collectStudio(s));
        await invoke("save_settings", { settings: next });
        s = next;
        localS = next;
      }
      notifyPhysicalSave();
      await loadStudioCaps();
      ($("set-upload") as HTMLButtonElement).disabled = !s.MemberAccessKey;
      $("set-status").textContent = "";
    } catch (e) {
      toast(String(e));
    }
  };

  const scheduleSave = () => {
    if (ignore) return;
    window.clearTimeout(saveTimer);
    saveTimer = window.setTimeout(() => void doSave(), SAVE_MS);
  };

  const showTab = (id: string) => {
    page.querySelectorAll("[data-set-tab]").forEach((b) => {
      b.classList.toggle("active", (b as HTMLElement).dataset.setTab === id);
    });
    page.querySelectorAll("[data-set-pane]").forEach((p) => {
      (p as HTMLElement).hidden = (p as HTMLElement).dataset.setPane !== id;
    });
    try {
      sessionStorage.setItem("studio-settings-tab", id);
    } catch {
      /* ignore */
    }
  };
  page.querySelector("#set-tabs")?.addEventListener("click", (ev) => {
    const b = (ev.target as HTMLElement).closest<HTMLElement>("[data-set-tab]");
    if (b?.dataset.setTab) showTab(b.dataset.setTab);
  });

  const apiDlg = page.querySelector("#set-api-dlg")!;
  page.querySelector("#set-api")?.addEventListener("input", paintApiWarn);

  const pendingSave = () => {
    paintApiWarn();
    if (officialMemberHost(val("set-api"))) {
      scheduleSave();
      return;
    }
    const msg = page.querySelector("#set-api-dlg-msg");
    if (msg) {
      msg.textContent = `API base “${val("set-api").trim() || "(empty)"}” is not epg.monster, my.epg.monster, or localhost. Studio will send your access key to that host.`;
    }
    apiDlg.classList.add("open");
  };
  page.querySelector("#set-api-no")!.addEventListener("click", () => apiDlg.classList.remove("open"));
  page.querySelector("#set-api-yes")!.addEventListener("click", () => {
    apiDlg.classList.remove("open");
    scheduleSave();
  });

  page.querySelector("#detect-tools")?.addEventListener("click", async () => {
    try {
      const p = await invokeLocal<{ mpv: string; vlc: string; ffmpeg: string; ffprobe: string }>("detect_tool_paths");
      setVal("set-mpv", p.mpv);
      setVal("set-vlc", p.vlc);
      if (!remote) {
        setVal("set-ffmpeg", p.ffmpeg);
        setVal("set-ffprobe", p.ffprobe);
      }
      $("set-status").textContent = "Detected bundled / common install paths.";
      scheduleSave();
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#detect-host-tools")?.addEventListener("click", async () => {
    try {
      const p = await invoke<{ mpv: string; vlc: string; ffmpeg: string; ffprobe: string }>("detect_tool_paths");
      setVal("set-ffmpeg", p.ffmpeg);
      setVal("set-ffprobe", p.ffprobe);
      $("set-status").textContent = "Detected tools on the studio host.";
      scheduleSave();
    } catch (e) {
      toast(String(e));
    }
  });

  page.querySelector("#set-test")!.addEventListener("click", async () => {
    paintApiWarn();
    if (!officialMemberHost(val("set-api"))) {
      toast("API base is not an official epg.monster host. Testing will send the key there.");
    }
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
      await doSave();
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
  page.querySelector("#set-logs")!.addEventListener("click", () => void invokeLocal("open_folder", { path: folders.logs }));
  page.querySelector("#set-crashes")!.addEventListener("click", () => void invokeLocal("open_folder", { path: folders.crashes }));

  page.querySelector("#set-remote-go")?.addEventListener("click", async () => {
    const status = page.querySelector("#set-remote-status");
    if (status) status.textContent = "Connecting…";
    try {
      await connectRemote(val("set-remote-url"), val("set-remote-key"));
    } catch (e) {
      if (status) status.textContent = String(e instanceof Error ? e.message : e);
      toast(String(e));
    }
  });
  page.querySelector("#set-remote-off")?.addEventListener("click", () => disconnectRemote());

  page.querySelector("#set-pw-save")?.addEventListener("click", async () => {
    const status = page.querySelector("#set-pw-status");
    const next = val("set-pw-next");
    if (next !== val("set-pw-next2")) {
      if (status) status.textContent = "New passwords do not match.";
      return;
    }
    try {
      const r = await fetch("/api/password", {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ current: val("set-pw-cur"), next }),
      });
      const j = (await r.json()) as { error?: string };
      if (!r.ok) throw new Error(j.error || "Could not change password.");
      if (status) status.textContent = "Password updated.";
      toast("Password updated");
      setVal("set-pw-cur", "");
      setVal("set-pw-next", "");
      setVal("set-pw-next2", "");
    } catch (e) {
      if (status) status.textContent = String(e instanceof Error ? e.message : e);
      toast(String(e));
    }
  });
  page.querySelector("#set-logout")?.addEventListener("click", async () => {
    await fetch("/api/logout", { method: "POST", credentials: "same-origin" });
    sessionStorage.removeItem("studio-live");
    window.location.reload();
  });

  const paintKeys = async () => {
    const list = page.querySelector("#set-key-list");
    if (!list) return;
    let keys: ApiKeyInfo[] = [];
    try {
      keys = await invoke<ApiKeyInfo[]>("list_api_keys");
    } catch (e) {
      list.textContent = String(e);
      return;
    }
    list.innerHTML = "";
    if (keys.length === 0) {
      list.innerHTML = `<p class="page-sub">No desktop API keys yet.</p>`;
      return;
    }
    for (const k of keys) {
      const row = document.createElement("div");
      row.className = "key-row";
      const when = k.created ? new Date(k.created * 1000).toLocaleString() : "";
      row.innerHTML = `<span><strong>${k.name}</strong> · ${k.prefix}… · ${when}</span>`;
      const del = document.createElement("button");
      del.type = "button";
      del.textContent = "Revoke";
      del.addEventListener("click", async () => {
        await invoke("revoke_api_key", { id: k.id });
        toast("Revoked " + k.name);
        await paintKeys();
      });
      row.appendChild(del);
      list.appendChild(row);
    }
  };
  page.querySelector("#set-key-make")?.addEventListener("click", async () => {
    try {
      const created = await invoke<{ id: string; name: string; key: string; created: number }>("create_api_key", {
        name: val("set-key-name").trim() || "Desktop",
      });
      setVal("set-key-once", created.key);
      page.querySelector("#set-key-dlg")?.classList.add("open");
      await paintKeys();
    } catch (e) {
      toast(String(e));
    }
  });
  page.querySelector("#set-key-copy")?.addEventListener("click", async () => {
    const v = val("set-key-once");
    try {
      await navigator.clipboard.writeText(v);
      toast("Copied");
    } catch {
      (page.querySelector("#set-key-once") as HTMLInputElement | null)?.select();
    }
  });
  page.querySelector("#set-key-dlg-ok")?.addEventListener("click", () => {
    page.querySelector("#set-key-dlg")?.classList.remove("open");
    setVal("set-key-once", "");
  });

  page.querySelector(".settings-body")?.addEventListener("input", (ev) => {
    const t = ev.target as HTMLElement;
    if (t.closest("#set-remote-url, #set-remote-key, #set-pw-cur, #set-pw-next, #set-pw-next2, #set-key-name, #set-key-once")) {
      return;
    }
    if ((t as HTMLElement).id === "set-api") {
      paintApiWarn();
      return;
    }
    scheduleSave();
  });
  page.querySelector(".settings-body")?.addEventListener("change", (ev) => {
    const t = ev.target as HTMLElement;
    if (t.closest("#set-remote-url, #set-remote-key, #set-pw-cur, #set-pw-next, #set-pw-next2, #set-key-name")) return;
    if ((t as HTMLElement).id === "set-api") {
      pendingSave();
      return;
    }
    scheduleSave();
  });

  try {
    const host = await invokeLocal<{ os: string; exeSuffix: string }>("host_info");
    const win = host.os === "windows";
    const setLbl = (id: string, text: string) => {
      const el = page.querySelector(`#${id}`);
      if (el) el.textContent = text;
    };
    setLbl("lbl-mpv", win ? "mpv.exe path" : "mpv path");
    setLbl("lbl-vlc", win ? "vlc.exe path" : "vlc path");
    setLbl("lbl-ffmpeg", remote ? (win ? "ffmpeg.exe path (host)" : "ffmpeg path (host)") : win ? "ffmpeg.exe path" : "ffmpeg path");
    setLbl("lbl-ffprobe", remote ? (win ? "ffprobe.exe path (host)" : "ffprobe path (host)") : win ? "ffprobe.exe path" : "ffprobe path");
    const mpv = page.querySelector("#set-mpv") as HTMLInputElement | null;
    const vlc = page.querySelector("#set-vlc") as HTMLInputElement | null;
    if (mpv) mpv.placeholder = win ? "mpv.exe" : "mpv";
    if (vlc) vlc.placeholder = win ? "vlc.exe" : "vlc";
    const ff = page.querySelector("#set-ffmpeg") as HTMLInputElement | null;
    const fp = page.querySelector("#set-ffprobe") as HTMLInputElement | null;
    if (ff) ff.placeholder = win ? "ffmpeg.exe" : "ffmpeg";
    if (fp) fp.placeholder = win ? "ffprobe.exe" : "ffprobe";
    const py = page.querySelector("#set-py") as HTMLInputElement | null;
    if (py) py.placeholder = win ? "python.exe" : "python3";
  } catch {
    /* labels stay generic */
  }

  try {
    folders = await invoke("settings_folders");
    $("set-logpath").textContent = `Log: ${folders.currentLog || folders.logs}`;
    s = await invoke<AppSettings>("load_settings");
    localS = remote ? await invokeLocal<AppSettings>("load_settings") : s;
    fillStudio(s);
    fillPlayers(remote ? localS : s);
    if (showConnectFields()) {
      const c = loadClient();
      setVal("set-remote-url", c.url);
      setVal("set-remote-key", c.apiKey);
      const st = page.querySelector("#set-remote-status");
      if (st) st.textContent = remote ? `Connected to ${c.url}` : "";
    }
    await paintSlates();
    if (page.querySelector("#set-key-list")) await paintKeys();
    const savedTab = sessionStorage.getItem("studio-settings-tab");
    if (savedTab && page.querySelector(`[data-set-tab="${savedTab}"]`)) showTab(savedTab);
  } catch (e) {
    toast(String(e));
  }
  ignore = false;
}

function showConnectFields(): boolean {
  return !isWebHost();
}
