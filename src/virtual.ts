export type VirtualList<T> = {
  setItems: (items: T[]) => void;
  destroy: () => void;
};

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
  let onScroll: (() => void) | null = null;

  const paint = () => {
    const headerH = opts.header?.offsetHeight ?? 0;
    inner.style.height = `${items.length * rowH}px`;
    const view = opts.scroller.clientHeight;
    const scroll = Math.max(0, opts.scroller.scrollTop - headerH);
    const start = Math.max(0, Math.floor(scroll / rowH) - 10);
    const end = Math.min(items.length, Math.ceil((scroll + view) / rowH) + 10);
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

  onScroll = paint;
  opts.scroller.addEventListener("scroll", onScroll, { passive: true });

  return {
    setItems(next) {
      items = next;
      paint();
    },
    destroy() {
      if (onScroll) opts.scroller.removeEventListener("scroll", onScroll);
      inner.remove();
    },
  };
}