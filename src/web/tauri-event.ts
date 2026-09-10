type Unlisten = () => void;

export async function listen<T>(event: string, handler: (ev: { payload: T }) => void): Promise<Unlisten> {
  const es = new EventSource("/api/events");
  const onMsg = (ev: MessageEvent) => {
    try {
      const j = JSON.parse(String(ev.data)) as { event?: string; payload?: T };
      if (j.event === event) handler({ payload: j.payload as T });
    } catch {
      /* ignore */
    }
  };
  es.addEventListener("message", onMsg);
  return () => {
    es.removeEventListener("message", onMsg);
    es.close();
  };
}

export async function emit(_event: string, _payload?: unknown): Promise<void> {
  /* catalog picks stay in-page for the web host */
}
