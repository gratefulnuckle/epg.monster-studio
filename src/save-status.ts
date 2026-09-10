/** App-wide save LED + undo stack. Newest undo is at the front. */
export const MAX_UNDO = 50;
export const SAVED_LED_MS = 5000;

export type UndoRecord = {
  label: string;
  undo: () => Promise<void>;
};

export type SaveNotice = {
  undoCount: number;
  showSaved: boolean;
  showUndo: boolean;
};

type Listener = (n: SaveNotice) => void;

let stack: UndoRecord[] = [];
let undoRevealed = false;
let savedUntil = 0;
let ledTimer = 0;
const listeners = new Set<Listener>();

export function resetSaveStatusForTests(): void {
  if (ledTimer) clearTimeout(ledTimer);
  stack = [];
  undoRevealed = false;
  savedUntil = 0;
  ledTimer = 0;
  listeners.clear();
}

function notice(showSaved: boolean): SaveNotice {
  return {
    undoCount: stack.length,
    showSaved,
    showUndo: undoRevealed && stack.length > 0,
  };
}

function emit(showSaved: boolean): void {
  const n = notice(showSaved);
  for (const fn of listeners) fn(n);
}

export function subscribeSaveStatus(fn: Listener): () => void {
  listeners.add(fn);
  fn(notice(Date.now() < savedUntil));
  return () => {
    listeners.delete(fn);
  };
}

/** Call only after the backend confirms the write. */
export function notifyPhysicalSave(undo?: UndoRecord, now = Date.now()): SaveNotice {
  if (undo) {
    stack.unshift(undo);
    if (stack.length > MAX_UNDO) stack.length = MAX_UNDO;
  }
  savedUntil = now + SAVED_LED_MS;
  if (ledTimer) clearTimeout(ledTimer);
  const t = setTimeout(() => {
    ledTimer = 0;
    expireSavedLed();
  }, SAVED_LED_MS);
  if (typeof t === "object" && t && "unref" in t) {
    (t as { unref: () => void }).unref();
  }
  ledTimer = t as unknown as number;
  const n = notice(true);
  emit(true);
  return n;
}

/** When the 5s saved LED ends, reveal Undo (if the stack has anything). */
export function expireSavedLed(now = Date.now()): SaveNotice {
  if (savedUntil === 0 || now < savedUntil) return notice(now < savedUntil);
  savedUntil = 0;
  if (stack.length > 0) undoRevealed = true;
  const n = notice(false);
  emit(false);
  return n;
}

export function peekUndo(): UndoRecord | null {
  return stack[0] ?? null;
}

export async function runUndo(): Promise<boolean> {
  const top = stack.shift();
  if (!top) {
    emit(Date.now() < savedUntil);
    return false;
  }
  await top.undo();
  emit(Date.now() < savedUntil);
  return true;
}
