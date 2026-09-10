import path from "node:path";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;
const web = process.env.STUDIO_WEB === "1" || process.env.STUDIO_WEB === "true";

function patchRelative(api: { relative: (from: string, to: string) => string }) {
  const orig = api.relative.bind(api);
  api.relative = (from: string, to: string) => {
    const norm = (p: string) => String(p ?? "").replace(/\\/g, "/");
    let a = norm(from);
    let b = norm(to);
    const da = a.match(/^([a-zA-Z]:)/);
    const db = b.match(/^([a-zA-Z]:)/);
    if (da && db && da[1].toLowerCase() === db[1].toLowerCase()) {
      a = a.slice(2);
      b = b.slice(2);
    }
    let rel = orig(a, b).replace(/\\/g, "/");
    if (!rel || path.win32.isAbsolute(rel) || /^[a-zA-Z]:/.test(rel) || rel.startsWith("/")) {
      const base = b.split("/").pop();
      return base && base.length ? base : "index.html";
    }
    return rel;
  };
}

patchRelative(path);
patchRelative(path.win32);
patchRelative(path.posix);

export default defineConfig({
  clearScreen: false,
  define: {
    __STUDIO_WEB__: web,
  },
  resolve: {
    alias: {
      "@tauri-apps/api/core": path.resolve("src/bridge/core.ts"),
      "@tauri-apps/api/event": path.resolve("src/bridge/event.ts"),
      "@studio-native/core": path.resolve("node_modules/@tauri-apps/api/core.js"),
      "@studio-native/event": path.resolve("node_modules/@tauri-apps/api/event.js"),
      ...(web
        ? {
            "@tauri-apps/api/window": path.resolve("src/web/tauri-window.ts"),
            "@tauri-apps/plugin-opener": path.resolve("src/web/tauri-opener.ts"),
          }
        : {}),
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    // WebView2 on Windows resolves localhost to 127.0.0.1; Vite 7 with
    // host:false often binds [::1] only, so the studio window never loads.
    host: web ? "0.0.0.0" : host || "127.0.0.1",
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**", "**/tools/**", "**/data/**"] },
    proxy: web
      ? {
          "/api": { target: "http://127.0.0.1:9477", changeOrigin: true },
        }
      : undefined,
  },
  build: {
    rollupOptions: {
      input: {
        main: "index.html",
        catalog: "catalog.html",
        search: "search.html",
      },
    },
  },
});
