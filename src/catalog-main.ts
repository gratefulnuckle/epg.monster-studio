import "./styles.css";
import { installCrashHooks } from "./crash";
import { mountCatalogWindow } from "./catalog-window";

installCrashHooks();

const app = document.querySelector<HTMLDivElement>("#app");
if (!app) throw new Error("#app missing");
await mountCatalogWindow(app);
