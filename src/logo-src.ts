import { invoke } from "@tauri-apps/api/core";

const cache = new Map<string, Promise<string | null>>();
const MAX_IN_FLIGHT = 4;
let inFlight = 0;
const waiters: Array<() => void> = [];

function decodeLogoUrl(url: string): string {
  return url
    .trim()
    .replace(/&amp;/gi, "&")
    .replace(/&lt;/gi, "<")
    .replace(/&gt;/gi, ">")
    .replace(/&quot;/gi, '"')
    .replace(/&#39;/g, "'");
}

function acquire(): Promise<void> {
  if (inFlight < MAX_IN_FLIGHT) {
    inFlight += 1;
    return Promise.resolve();
  }
  return new Promise((resolve) => {
    waiters.push(() => {
      inFlight += 1;
      resolve();
    });
  });
}

function release(): void {
  inFlight = Math.max(0, inFlight - 1);
  const next = waiters.shift();
  if (next) next();
}

/** Same bytes as the logo probe (VLC UA GET). Avoids WebView2 Origin/hotlink blocks. */
export function playerLogoSrc(url: string): Promise<string | null> {
  const u = decodeLogoUrl(url);
  if (!u) return Promise.resolve(null);
  let p = cache.get(u);
  if (!p) {
    p = (async () => {
      await acquire();
      try {
        return await invoke<string>("logo_preview_data", { url: u });
      } catch {
        return null;
      } finally {
        release();
      }
    })();
    cache.set(u, p);
    void p.then((src) => {
      if (!src) cache.delete(u);
    });
  }
  return p;
}

export function bindPlayerLogo(img: HTMLImageElement, url: string, onFail: () => void): void {
  const raw = decodeLogoUrl(url);
  if (!raw) {
    onFail();
    return;
  }
  const use = (src: string, allowFallback: boolean) => {
    if (!img.isConnected) return;
    img.referrerPolicy = "no-referrer";
    img.onerror = () => {
      if (allowFallback && src !== raw) {
        use(raw, false);
        return;
      }
      onFail();
    };
    img.src = src;
  };
  void playerLogoSrc(raw).then((src) => {
    if (!img.isConnected) return;
    if (src) use(src, true);
    else use(raw, false);
  });
}
