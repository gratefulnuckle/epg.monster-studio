import "./styles.css";
import { installCrashHooks } from "./crash";
import { mountSourceSearchWindow } from "./search-window";

installCrashHooks();

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("#app missing");
await mountSourceSearchWindow(app);
