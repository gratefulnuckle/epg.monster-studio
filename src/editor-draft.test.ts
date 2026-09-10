import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { draftToManaged, takeEditorDraft } from "./editor-draft.ts";

function memoryStorage(initial: Record<string, string> = {}) {
  const data = { ...initial };
  return {
    getItem(key: string) {
      return Object.prototype.hasOwnProperty.call(data, key) ? data[key] : null;
    },
    setItem(key: string, value: string) {
      data[key] = value;
    },
    removeItem(key: string) {
      delete data[key];
    },
  };
}

describe("takeEditorDraft", () => {
  it("returns the source channel once, then nothing — so a second editor visit can still pick it up", () => {
    const storage = memoryStorage({
      "studio-editor-draft": JSON.stringify({
        id: "src-1",
        name: "CNN",
        groupTitle: "News",
        tvgId: "CNN.us",
        tvgLogo: "http://logo/cnn.png",
        url: "http://stream/cnn",
      }),
    });
    const first = takeEditorDraft(storage);
    assert.deepEqual(first, {
      name: "CNN",
      groupTitle: "News",
      tvgId: "CNN.us",
      tvgLogo: "http://logo/cnn.png",
      url: "http://stream/cnn",
    });
    assert.equal(takeEditorDraft(storage), null);
  });

  it("returns null for missing or invalid JSON", () => {
    assert.equal(takeEditorDraft(memoryStorage()), null);
    assert.equal(takeEditorDraft(memoryStorage({ "studio-editor-draft": "{" })), null);
  });
});

describe("draftToManaged", () => {
  it("loads source details onto a new Unassigned channel for the edit form", () => {
    const ch = draftToManaged(
      {
        name: "CNN",
        groupTitle: "News",
        tvgId: "CNN.us",
        tvgLogo: "http://logo/cnn.png",
        url: "http://stream/cnn",
      },
      "new-id",
    );
    assert.equal(ch.id, "new-id");
    assert.equal(ch.name, "CNN");
    assert.equal(ch.groupTitle, "Unassigned");
    assert.equal(ch.tvgId, "CNN.us");
    assert.equal(ch.tvgLogo, "http://logo/cnn.png");
    assert.equal(ch.variants[0]?.url, "http://stream/cnn");
    assert.equal(ch.variants[0]?.visibility, "visible");
  });
});
