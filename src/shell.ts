import { invoke } from "@tauri-apps/api/core";

export type NavId =
  | "audit"
  | "editor"
  | "epg"
  | "logoaudit"
  | "autoaudit"
  | "output"
  | "tuner"
  | "settings";

const NAV: { id: NavId; label: string }[] = [
  { id: "audit", label: "Add Sources" },
  { id: "editor", label: "Playlist Editor" },
  { id: "epg", label: "EPG Audit" },
  { id: "logoaudit", label: "Logo Audit" },
  { id: "autoaudit", label: "Stream Audit" },
  { id: "output", label: "Managed Output" },
  { id: "tuner", label: "TV Tuner" },
];

const SEARCH_PAGES: NavId[] = ["audit", "editor", "output"];

export function mountShell(root: HTMLElement): void {
  root.innerHTML = `
    <div class="shell">
      <header class="titlebar" data-tauri-drag-region>
        <span class="brand">epg.monster studio</span>
        <input class="search" id="search" placeholder="Search name, group, tvg-id, URL…" />
      </header>
      <div class="workspace">
        <aside class="nav">
          <button class="nav-logo" id="about" title="About epg.monster studio">
            <img src="/logo.png" alt="epg.monster studio" />
          </button>
          <div class="nav-items" id="nav-items"></div>
          <div class="nav-footer">
            <button class="nav-item" data-nav="settings">Settings</button>
          </div>
        </aside>
        <main class="page" id="page"></main>
      </div>
      <div class="toast" id="toast"></div>
      <div class="dialog-backdrop" id="about-dlg">
        <div class="dialog">
          <h2>About epg.monster studio</h2>
          <p>epg.monster studio</p>
          <p>GNU General Public License v3.0</p>
          <p><a href="https://www.gnu.org/licenses/gpl-3.0.html">https://www.gnu.org/licenses/gpl-3.0.html</a></p>
          <div class="dialog-actions">
            <button id="about-close">Close</button>
          </div>
        </div>
      </div>
    </div>
  `;

  const items = root.querySelector("#nav-items")!;
  for (const item of NAV) {
    const b = document.createElement("button");
    b.className = "nav-item";
    b.dataset.nav = item.id;
    b.textContent = item.label;
    items.appendChild(b);
  }

  const page = root.querySelector<HTMLElement>("#page")!;
  const search = root.querySelector<HTMLInputElement>("#search")!;
  const toast = root.querySelector<HTMLElement>("#toast")!;
  const aboutDlg = root.querySelector<HTMLElement>("#about-dlg")!;

  const showToast = (text: string) => {
    toast.textContent = text;
    toast.classList.add("open");
    window.setTimeout(() => toast.classList.remove("open"), 3000);
  };

  const render = (id: NavId) => {
    root.querySelectorAll(".nav-item").forEach((el) => {
      el.classList.toggle("active", (el as HTMLElement).dataset.nav === id);
    });
    search.classList.toggle("hidden", !SEARCH_PAGES.includes(id));
    page.innerHTML = pageHtml(id);
  };

  root.querySelectorAll<HTMLButtonElement>("[data-nav]").forEach((btn) => {
    btn.addEventListener("click", () => render(btn.dataset.nav as NavId));
  });

  root.querySelector("#about")!.addEventListener("click", () => aboutDlg.classList.add("open"));
  root.querySelector("#about-close")!.addEventListener("click", () => aboutDlg.classList.remove("open"));

  root.addEventListener("click", async (ev) => {
    const t = ev.target as HTMLElement;
    if (t.id === "add-source") {
      try {
        const added = await invoke<boolean>("pick_source_file");
        showToast(added ? "Source loaded." : "No file selected.");
      } catch (e) {
        showToast(String(e));
      }
    }
    if (t.id === "save-settings") {
      showToast("Settings saved.");
    }
    if (t.id === "detect-tools") {
      try {
        const n = await invoke<number>("detect_bundled_tools");
        showToast(`Detected ${n} bundled tool path(s).`);
      } catch (e) {
        showToast(String(e));
      }
    }
  });

  render("audit");
}

function pageHtml(id: NavId): string {
  switch (id) {
    case "audit":
      return `
        <h1 class="page-title">Add Sources</h1>
        <p class="page-sub">Load file or URL playlists (custom headers). Groups + channels, play (mpv/VLC), copy URL/tvg-id.</p>
        <div class="empty">
          <div class="glyph">☰</div>
          <p>Add source…</p>
          <button class="accent" id="add-source">Add source…</button>
        </div>`;
    case "editor":
      return `
        <h1 class="page-title">Playlist Editor</h1>
        <p class="page-sub">Curated channels: name, group, logo, tvg-id, tvg-shift. Visible stream + hidden backups.</p>`;
    case "epg":
      return `
        <h1 class="page-title">EPG Audit</h1>
        <p class="page-sub">Fetch epg.monster XMLTV only. Catalog is the &lt;channel&gt; tvg-ids in that file. Exact + fuzzy match.</p>`;
    case "logoaudit":
      return `
        <h1 class="page-title">Logo Audit</h1>
        <p class="page-sub">Find missing, invalid, or broken logos. Save Logos downloads a local PNG pack.</p>`;
    case "autoaudit":
      return `
        <h1 class="page-title">Stream Audit</h1>
        <p class="page-sub">Serial stream probes (ffmpeg + ffprobe). Live streams that only show a known “channel is offline” card fail as an offline slate.
        The full result list is kept if you leave this page. Pause / resume survives a crash via auditprocess.db.</p>
        <div>
          <button class="accent">Start (all variants)</button>
          <button>Visible only</button>
          <button>Audit specific channels…</button>
          <button>Run today's groups</button>
          <button disabled>Pause</button>
          <button disabled>Resume</button>
          <button disabled>Cancel</button>
          <button>Undo last swap</button>
          <label class="check"><input type="checkbox" checked /> Auto-swap on fail</label>
          <button># Results</button>
        </div>`;
    case "output":
      return `
        <h1 class="page-title">Managed Output</h1>
        <p class="page-sub">Overview of visible vs hidden streams. Export. Undo last swap. Tuner lineup.</p>`;
    case "tuner":
      return `
        <h1 class="page-title">TV Tuner</h1>
        <p class="page-sub">Start and stop the local tuner hosts. Enable a card in Settings, then Start here. Ports are 8080 Plex, 8081 Jellyfin, 8082 Emby, 8083 IPTV. Channel numbers stay in Managed Output → Tuner lineup.</p>
        <div>
          <button class="accent">Start all enabled</button>
          <button>Stop all</button>
          <button>Log</button>
          <button>Graphs</button>
          <button>Self-test</button>
        </div>
        <p class="page-sub">Enable a tuner in Settings (Plex, Jellyfin, Emby, or IPTV), Save, then press Start on that card.</p>`;
    case "settings":
      return settingsHtml();
  }
}

function settingsHtml(): string {
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
    <div class="settings-grid">
      <section class="tile">
        <h2>Players</h2>
        <p class="hint">External player used from Playlist Editor and Stream.</p>
        <div class="field"><label>Default player</label>
          <select><option>mpv</option><option>VLC</option></select></div>
        <div class="field"><label>mpv.exe path</label><input /></div>
        <div class="field"><label>vlc.exe path</label><input /></div>
      </section>
      <section class="tile">
        <h2>Stream Audit</h2>
        <p class="hint">ffmpeg / ffprobe used for probes and the HDHomeRun remux.</p>
        <div class="field"><label>ffmpeg.exe path</label><input /></div>
        <div class="field"><label>ffprobe.exe path</label><input /></div>
        <div class="field"><label>Delay between probes (ms)</label><input type="number" value="6000" /></div>
        <div class="field"><label>Probe timeout (ms)</label><input type="number" value="15000" /></div>
        <label class="check"><input type="checkbox" checked /> Auto-swap visible stream to working backup on fail</label>
        <label class="check"><input type="checkbox" checked /> Pause auto-audit while external player is active</label>
      </section>
      <section class="tile">
        <h2>Guide</h2>
        <p class="hint">XMLTV catalog. Built from tvg-ids in this file.</p>
        <div class="field"><label>Default User-Agent for URL sources</label><input value="epg.monster-studio/v1.0-beta" /></div>
        <div class="field"><label>XMLTV guide URL (epg.monster)</label>
          <textarea>https://epg.monster/epg.xml</textarea></div>
      </section>
      <section class="tile">
        <h2>my.epg.monster</h2>
        <p class="hint">Access key from Keys. Upload sends curated tvg-ids only — never stream URLs.</p>
        <div class="field"><label>Email</label><input placeholder="you@example.com" /></div>
        <div class="field"><label>Access key (epgm_…)</label><input type="password" /></div>
        <div class="field"><label>API base</label><input placeholder="https://epg.monster" /></div>
        <div><button>Test key</button> <button class="accent">Upload channels.json</button></div>
      </section>
      <section class="tile" style="grid-column:1/-1">
        <h2>TV Tuner</h2>
        <p class="hint">IPTV is on for new installs. Plex, Jellyfin, and Emby stay off until you enable them. Ports 8080–8083. Start/stop is on the TV Tuner panel.</p>
        <div class="settings-grid">
          ${tunerCard("Plex", 8080, false)}
          ${tunerCard("Jellyfin", 8081, false, true)}
          ${tunerCard("Emby", 8082, false)}
          ${tunerCard("IPTV (TiviMate / Smarters)", 8083, true, false, true)}
        </div>
        <label class="check"><input type="checkbox" checked /> Advertise tuners on the network (HDHomeRun UDP 65001 + SSDP). Turn on Allow LAN if Plex is another PC.</label>
      </section>
      <section class="tile">
        <h2>Remux</h2>
        <p class="hint">Spawn ffmpeg or VLC, buffer MPEG-TS, then serve Plex. MPEG2+AC3 is the Plex-safe default. VLC is always copy-to-TS.</p>
        <div class="field"><label>Engine</label><select><option>ffmpeg</option><option>VLC</option></select></div>
        <div class="field"><label>ffmpeg profile</label>
          <select><option>Plex MPEG2 + AC3 (recommended)</option><option>Threadfin copy (H264 + AAC stereo)</option></select></div>
        <div class="field"><label>Buffer before send (KB)</label><input type="number" value="4096" /></div>
      </section>
      <section class="tile">
        <h2>Logos</h2>
        <p class="hint">Local PNG pack and optional hosting on the tuner.</p>
        <div class="field"><label>Logo save directory</label><input placeholder="%LocalAppData%\\epg.monster-studio\\logo" /></div>
        <label class="check"><input type="checkbox" /> Host the logos folder on the tuner</label>
        <label class="check"><input type="checkbox" /> Use local logos in tuner playlists and EPG</label>
      </section>
      <section class="tile">
        <h2>Weekly Stream Audit</h2>
        <p class="hint">Group names, comma-separated. Stream Audit → Run today's groups. Skip groups with no match.</p>
        <div class="settings-grid">
          <div class="field"><label>Monday</label><input /></div>
          <div class="field"><label>Tuesday</label><input /></div>
          <div class="field"><label>Wednesday</label><input /></div>
          <div class="field"><label>Thursday</label><input /></div>
          <div class="field"><label>Friday</label><input /></div>
          <div class="field"><label>Saturday</label><input /></div>
          <div class="field" style="grid-column:1/-1"><label>Sunday</label><input /></div>
        </div>
        <label class="check"><input type="checkbox" /> Remind me when today's groups have not run (does not start a probe)</label>
        <label class="check"><input type="checkbox" /> Fail fully black screens (ffmpeg blackdetect)</label>
      </section>
      <section class="tile">
        <h2>Screen matches</h2>
        <p class="hint">After a stream decodes, hash one frame against these stills (offline / slate cards).</p>
        <div><button>Add screen…</button> <button>Remove selected</button> <button>Open folder</button></div>
        <h2 style="margin-top:16px">Diagnostics</h2>
        <p class="hint">Daily logs and crash reports live under local app data. A crash opens a report on the next launch.</p>
        <div><button>Open logs folder</button> <button>Open crash reports</button></div>
        <div class="field"><label>Optional Python path</label><input placeholder="python.exe" /></div>
      </section>
    </div>
  `;
}

function tunerCard(title: string, port: number, enabled: boolean, downspiral = false, iptv = false): string {
  return `
    <section class="tile">
      <label class="check"><input type="checkbox" ${enabled ? "checked" : ""} /> ${title}</label>
      <div class="field"><label>Friendly name</label><input /></div>
      <div class="field"><label>Port</label><input type="number" value="${port}" /></div>
      <div class="field"><label>Tuner count</label><input type="number" value="2" /></div>
      <label class="check"><input type="checkbox" /> Allow LAN</label>
      ${downspiral ? `<label class="check"><input type="checkbox" /> Downspiral — one playlist + guide per group (switch lists without changing Jellyfin profiles)</label>` : ""}
      ${iptv ? `
        <label class="check"><input type="checkbox" checked /> Remux IPTV playlist through Studio (MPEG-TS)</label>
        <div class="field"><label>Tuner EPG for IPTV players</label>
          <select><option>Local Studio guide (/guide.xml)</option><option>my.epg.monster curated feed</option></select></div>` : ""}
    </section>
  `;
}
