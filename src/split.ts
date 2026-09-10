export type ColResizeOpts = {
  grid: HTMLElement;
  handle: HTMLElement;
  cssVar: string;
  storageKey: string;
  measure: (clientX: number, grid: DOMRect) => number;
};

export function colWidthFromMeasure(measured: number): number {
  if (!Number.isFinite(measured)) return 0;
  return Math.max(0, Math.round(measured));
}

export function savedColWidth(raw: string | null): number | null {
  if (raw == null || raw.trim() === "") return null;
  const n = Number(raw);
  if (Number.isFinite(n) && n >= 0) return Math.round(n);
  return null;
}

export function groupsTrackWidth(cssValue: string, fallback = 220): number {
  const n = parseInt(cssValue.trim(), 10);
  return Number.isFinite(n) && n >= 0 ? n : fallback;
}

export function applySavedColWidth(grid: HTMLElement, cssVar: string, storageKey: string): void {
  const saved = savedColWidth(window.localStorage.getItem(storageKey));
  if (saved !== null) grid.style.setProperty(cssVar, `${saved}px`);
}

export function bindThreeColSplit(
  grid: HTMLElement,
  handleGroups: HTMLElement,
  handleChans: HTMLElement,
  storagePrefix: string,
): void {
  const groupsW = () =>
    groupsTrackWidth(getComputedStyle(grid).getPropertyValue("--split-groups-w"));
  bindColResize({
    grid,
    handle: handleGroups,
    cssVar: "--split-groups-w",
    storageKey: `${storagePrefix}-groups-w`,
    measure: (x, rect) => x - rect.left,
  });
  bindColResize({
    grid,
    handle: handleChans,
    cssVar: "--split-chans-w",
    storageKey: `${storagePrefix}-chans-w`,
    measure: (x, rect) => x - rect.left - groupsW() - 6,
  });
}

export function bindColResize(opts: ColResizeOpts): void {
  const { grid, handle, cssVar, storageKey, measure } = opts;
  applySavedColWidth(grid, cssVar, storageKey);
  handle.addEventListener("pointerdown", (ev) => {
    ev.preventDefault();
    handle.classList.add("dragging");
    handle.setPointerCapture(ev.pointerId);
    const onMove = (e: PointerEvent) => {
      const rect = grid.getBoundingClientRect();
      const w = colWidthFromMeasure(measure(e.clientX, rect));
      grid.style.setProperty(cssVar, `${w}px`);
    };
    const onUp = () => {
      handle.classList.remove("dragging");
      handle.releasePointerCapture(ev.pointerId);
      handle.removeEventListener("pointermove", onMove);
      handle.removeEventListener("pointerup", onUp);
      const raw = getComputedStyle(grid).getPropertyValue(cssVar).trim();
      const n = parseInt(raw, 10);
      if (Number.isFinite(n) && n >= 0) window.localStorage.setItem(storageKey, String(n));
    };
    handle.addEventListener("pointermove", onMove);
    handle.addEventListener("pointerup", onUp);
  });
}
