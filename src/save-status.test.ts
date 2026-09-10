import assert from "node:assert/strict";
import { describe, it, beforeEach } from "node:test";
import {
  MAX_UNDO,
  SAVED_LED_MS,
  expireSavedLed,
  notifyPhysicalSave,
  peekUndo,
  resetSaveStatusForTests,
  runUndo,
} from "./save-status.ts";

describe("notifyPhysicalSave", () => {
  beforeEach(() => resetSaveStatusForTests());

  it("shows saved immediately and keeps undo hidden until the LED expires", () => {
    const n = notifyPhysicalSave({ label: "a", undo: async () => {} }, 1_000);
    assert.equal(n.showSaved, true);
    assert.equal(n.showUndo, false);
    assert.equal(n.undoCount, 1);
  });

  it("reveals undo after the 5s saved LED ends", () => {
    notifyPhysicalSave({ label: "a", undo: async () => {} }, 1_000);
    const n = expireSavedLed(1_000 + SAVED_LED_MS);
    assert.equal(n.showSaved, false);
    assert.equal(n.showUndo, true);
  });

  it("puts later saves at the front of the queue, capped at MAX_UNDO", () => {
    for (let i = 0; i < MAX_UNDO + 5; i++) {
      notifyPhysicalSave({ label: String(i), undo: async () => {} }, i);
    }
    assert.equal(peekUndo()?.label, String(MAX_UNDO + 4));
    expireSavedLed(MAX_UNDO + 5 + SAVED_LED_MS);
    const n = notifyPhysicalSave({ label: "newest", undo: async () => {} }, 9_000);
    assert.equal(n.undoCount, MAX_UNDO);
    assert.equal(n.showSaved, true);
    assert.equal(n.showUndo, true);
    assert.equal(peekUndo()?.label, "newest");
  });

  it("runUndo restores the newest entry first", async () => {
    const order: string[] = [];
    notifyPhysicalSave({ label: "old", undo: async () => { order.push("old"); } }, 1);
    notifyPhysicalSave({ label: "new", undo: async () => { order.push("new"); } }, 2);
    assert.equal(await runUndo(), true);
    assert.deepEqual(order, ["new"]);
    assert.equal(peekUndo()?.label, "old");
  });
});
