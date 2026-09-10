/** Vite-bundled in-browser player (mpegts.js + hls.js). The server never spawns mpv/VLC. */

type Engine = { destroy: () => void };

let engine: Engine | null = null;

export function playInBrowser(url: string, sourceId?: string): void {
  const src = url.trim();
  if (!src) throw new Error("URL is empty.");
  closeWebPlayer();
  const proxied = streamProxyUrl(src, sourceId);
  const back = document.createElement("div");
  back.className = "web-player-backdrop open";
  back.id = "web-player-backdrop";
  back.innerHTML = `
    <div class="web-player" role="dialog" aria-label="Play">
      <div class="web-player-bar">
        <span class="web-player-title">Play</span>
        <a class="web-player-open" href="${escapeAttr(src)}" target="_blank" rel="noopener">Open URL</a>
        <button type="button" id="web-player-close" title="Close" aria-label="Close">&#xE711;</button>
      </div>
      <video id="web-player-video" controls autoplay playsinline></video>
      <p class="page-sub" id="web-player-hint">Playing in this browser (mpegts.js / hls.js). The studio server does not launch mpv or VLC.</p>
    </div>
  `;
  document.body.appendChild(back);
  const video = back.querySelector<HTMLVideoElement>("#web-player-video")!;
  const hint = back.querySelector<HTMLElement>("#web-player-hint")!;
  const fail = (why: string) => {
    hint.textContent = why;
  };
  video.addEventListener("error", () =>
    fail("The browser player hit an error. Try Open URL, or Connect a desktop for mpv/VLC."),
  );
  void attachEngine(video, src, proxied, fail);
  back.querySelector("#web-player-close")?.addEventListener("click", closeWebPlayer);
  back.addEventListener("click", (ev) => {
    if (ev.target === back) closeWebPlayer();
  });
  const onKey = (ev: KeyboardEvent) => {
    if (ev.key === "Escape") closeWebPlayer();
  };
  window.addEventListener("keydown", onKey);
  (back as HTMLElement & { _onKey?: (e: KeyboardEvent) => void })._onKey = onKey;
}

export function closeWebPlayer(): void {
  engine?.destroy();
  engine = null;
  const back = document.getElementById("web-player-backdrop") as
    | (HTMLElement & { _onKey?: (e: KeyboardEvent) => void })
    | null;
  if (!back) return;
  const video = back.querySelector("video");
  if (video) {
    video.pause();
    video.removeAttribute("src");
    video.load();
  }
  if (back._onKey) window.removeEventListener("keydown", back._onKey);
  back.remove();
}

function streamProxyUrl(url: string, sourceId?: string): string {
  const u = new URL("/api/stream", window.location.origin);
  u.searchParams.set("url", url);
  if (sourceId) u.searchParams.set("sourceId", sourceId);
  return u.toString();
}

function isHls(url: string): boolean {
  return /\.m3u8(\?|#|$)/i.test(url) || /[?&](type=)?m3u8\b/i.test(url);
}

function isFileVideo(url: string): boolean {
  return /\.(mp4|webm|ogg|ogv|mov)(\?|#|$)/i.test(url);
}

async function attachEngine(
  video: HTMLVideoElement,
  raw: string,
  proxied: string,
  fail: (why: string) => void,
): Promise<void> {
  if (isHls(raw)) {
    const ok = await attachHls(video, raw, proxied, fail);
    if (ok) return;
  }
  if (!isFileVideo(raw)) {
    const ok = await attachMpegTs(video, proxied, fail);
    if (ok) return;
  }
  video.src = raw;
  void video.play().catch(() => fail("Native playback failed. Open URL, or Connect a desktop."));
}

async function attachHls(
  video: HTMLVideoElement,
  raw: string,
  proxied: string,
  fail: (why: string) => void,
): Promise<boolean> {
  if (video.canPlayType("application/vnd.apple.mpegurl")) {
    video.src = raw;
    void video.play().catch(() => fail("HLS native playback failed."));
    return true;
  }
  const { default: Hls } = await import("hls.js");
  if (!Hls.isSupported()) return false;
  const hls = new Hls({
    enableWorker: true,
    xhrSetup: (xhr) => {
      xhr.withCredentials = true;
    },
  });
  hls.on(Hls.Events.ERROR, (_e, data) => {
    if (data.fatal) fail("HLS could not play this stream.");
  });
  hls.loadSource(proxied);
  hls.attachMedia(video);
  engine = { destroy: () => hls.destroy() };
  void video.play().catch(() => undefined);
  return true;
}

async function attachMpegTs(
  video: HTMLVideoElement,
  proxied: string,
  fail: (why: string) => void,
): Promise<boolean> {
  const mpegts = (await import("mpegts.js")).default;
  if (!mpegts.isSupported()) return false;
  const player = mpegts.createPlayer(
    {
      type: "mpegts",
      isLive: true,
      url: proxied,
      cors: true,
      withCredentials: true,
    },
    {
      enableWorker: true,
      enableStashBuffer: true,
      liveBufferLatencyChasing: true,
      isLive: true,
    },
  );
  player.on("error", (_t: string, detail: string) => {
    fail(`MPEG-TS player error (${detail}). Open URL, or Connect a desktop.`);
  });
  player.attachMediaElement(video);
  player.load();
  void player.play();
  engine = {
    destroy: () => {
      try {
        player.pause();
        player.unload();
        player.detachMediaElement();
        player.destroy();
      } catch {
        /* already torn down */
      }
    },
  };
  return true;
}

function escapeAttr(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
}
