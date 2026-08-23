export type ColResizeOpts = {
  grid: HTMLElement;
  handle: HTMLElement;
  cssVar: string;
  storageKey: string;
  min: number;
  max: number;
  measure: (clientX: number, grid: DOMRect) => number;
};

export function applySavedColWidth(
  grid: HTMLElement,
  cssVar: string,
  storageKey: string,
  min: number,
  max: number,
): void {
  const saved = Number(window.localStorage.getItem(storageKey) || "0");
  if (saved >= min && saved <= max) {
    grid.style.setProperty(cssVar, `${Math.round(saved)}px`);
  }
}

export function bindThreeColSplit(
  grid: HTMLElement,
  handleGroups: HTMLElement,
  handleChans: HTMLElement,
  storagePrefix: string,
): void {
  const groupsW = () => {
    const n = parseInt(getComputedStyle(grid).getPropertyValue("--split-groups-w").trim(), 10);
    return Number.isFinite(n) && n > 0 ? n : 220;
  };
  bindColResize({
    grid,
    handle: handleGroups,
    cssVar: "--split-groups-w",
    storageKey: `${storagePrefix}-groups-w`,
    min: 140,
    max: 420,
    measure: (x, rect) => x - rect.left,
  });
  bindColResize({
    grid,
    handle: handleChans,
    cssVar: "--split-chans-w",
    storageKey: `${storagePrefix}-chans-w`,
    min: 180,
    max: 560,
    measure: (x, rect) => x - rect.left - groupsW() - 6,
  });
}

export function bindColResize(opts: ColResizeOpts): void {
  const { grid, handle, cssVar, storageKey, min, max, measure } = opts;
  applySavedColWidth(grid, cssVar, storageKey, min, max);
  handle.addEventListener("pointerdown", (ev) => {
    ev.preventDefault();
    handle.classList.add("dragging");
    handle.setPointerCapture(ev.pointerId);
    const onMove = (e: PointerEvent) => {
      const rect = grid.getBoundingClientRect();
      const w = Math.min(max, Math.max(min, measure(e.clientX, rect)));
      grid.style.setProperty(cssVar, `${Math.round(w)}px`);
    };
    const onUp = () => {
      handle.classList.remove("dragging");
      handle.releasePointerCapture(ev.pointerId);
      handle.removeEventListener("pointermove", onMove);
      handle.removeEventListener("pointerup", onUp);
      const raw = getComputedStyle(grid).getPropertyValue(cssVar).trim();
      const n = parseInt(raw, 10);
      if (n >= min) window.localStorage.setItem(storageKey, String(n));
    };
    handle.addEventListener("pointermove", onMove);
    handle.addEventListener("pointerup", onUp);
  });
}
