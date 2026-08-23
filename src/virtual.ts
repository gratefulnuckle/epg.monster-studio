export type VirtualList<T> = {
  setItems: (items: T[]) => void;
  destroy: () => void;
};

const OVERSCAN = 8;
/** Safety cap if the scroller still has no bounded height. */
const MAX_ROWS = 48;

/** Visible index window. `view <= 0` paints a small stub, never the whole list. */
export function virtWindow(
  view: number,
  scroll: number,
  rowH: number,
  n: number,
): [number, number] {
  if (n <= 0 || rowH <= 0) return [0, 0];
  if (view <= 0) return [0, Math.min(n, OVERSCAN * 2)];
  const start = Math.max(0, Math.floor(Math.max(0, scroll) / rowH) - OVERSCAN);
  let end = Math.min(n, Math.ceil((Math.max(0, scroll) + view) / rowH) + OVERSCAN);
  if (end - start > MAX_ROWS) end = start + MAX_ROWS;
  return [start, end];
}

export function bindVirtualList<T>(opts: {
  scroller: HTMLElement;
  rowHeight: number;
  header?: HTMLElement | null;
  renderRow: (item: T, index: number) => HTMLElement;
}): VirtualList<T> {
  const rowH = opts.rowHeight;
  const inner = document.createElement("div");
  inner.className = "virt-inner";
  inner.style.position = "relative";
  inner.style.width = "100%";
  if (opts.header) {
    opts.scroller.appendChild(opts.header);
  }
  opts.scroller.appendChild(inner);

  let items: T[] = [];
  let lastStart = -1;
  let lastEnd = -1;
  let raf = 0;
  let alive = true;

  const paint = (force = false) => {
    if (!alive) return;
    const headerH = opts.header?.offsetHeight ?? 0;
    inner.style.height = `${items.length * rowH}px`;
    const view = opts.scroller.clientHeight;
    const scroll = Math.max(0, opts.scroller.scrollTop - headerH);
    const [start, end] = virtWindow(view, scroll, rowH, items.length);
    if (!force && start === lastStart && end === lastEnd) return;
    lastStart = start;
    lastEnd = end;
    inner.replaceChildren();
    for (let i = start; i < end; i++) {
      const el = opts.renderRow(items[i], i);
      el.style.position = "absolute";
      el.style.top = `${i * rowH}px`;
      el.style.left = "0";
      el.style.right = "0";
      el.style.height = `${rowH}px`;
      el.style.boxSizing = "border-box";
      inner.appendChild(el);
    }
  };

  const schedule = () => {
    if (raf || !alive) return;
    raf = requestAnimationFrame(() => {
      raf = 0;
      paint(false);
    });
  };

  opts.scroller.addEventListener("scroll", schedule, { passive: true });
  const ro =
    typeof ResizeObserver !== "undefined"
      ? new ResizeObserver(() => {
          lastStart = -1;
          schedule();
        })
      : null;
  ro?.observe(opts.scroller);

  return {
    setItems(next) {
      items = next;
      lastStart = -1;
      paint(true);
    },
    destroy() {
      alive = false;
      if (raf) cancelAnimationFrame(raf);
      raf = 0;
      opts.scroller.removeEventListener("scroll", schedule);
      ro?.disconnect();
      inner.remove();
    },
  };
}
