import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  keepGroup,
  nextIssueId,
  patchLogoAfterSet,
  type LogoIssueRow,
} from "./issue-nav.ts";

describe("nextIssueId", () => {
  it("lands on the following issue after the applied row disappears", () => {
    assert.equal(nextIssueId(["a", "b", "c"], "b"), "c");
  });

  it("lands on the new last issue when the last row was applied", () => {
    assert.equal(nextIssueId(["a", "b", "c"], "c"), "b");
  });

  it("returns null when the only remaining issue was applied", () => {
    assert.equal(nextIssueId(["a"], "a"), null);
  });
});

describe("keepGroup", () => {
  it("stays on the current group when it still has issues", () => {
    assert.equal(keepGroup(["News", "Sports"], "Sports"), "Sports");
  });

  it("does not jump to the first group until the current group is empty", () => {
    assert.equal(keepGroup(["News", "Sports"], "Movies"), "News");
  });
});

describe("patchLogoAfterSet", () => {
  const row = (over: Partial<LogoIssueRow> = {}): LogoIssueRow => ({
    managedChannelId: "a",
    channelName: "CNN",
    groupTitle: "News",
    tvgId: "CNN.us",
    currentLogo: null,
    issue: "missing",
    reason: "No logo URL.",
    ...over,
  });

  it("clears only the applied row and leaves other probe issues intact", () => {
    const rows = [
      row(),
      row({
        managedChannelId: "b",
        channelName: "BBC",
        issue: "broken",
        reason: "Players got HTTP 404.",
        currentLogo: "https://cdn/bbc.png",
      }),
    ];
    const next = rows.map((r) =>
      r.managedChannelId === "a" ? patchLogoAfterSet(r, "https://cdn/cnn.png") : r,
    );
    assert.equal(next[0].issue, "");
    assert.equal(next[0].currentLogo, "https://cdn/cnn.png");
    assert.equal(next[1].issue, "broken");
    assert.equal(next[1].reason, "Players got HTTP 404.");
  });

  it("marks a cleared logo as missing without touching other rows", () => {
    const rows = [
      row({
        currentLogo: "https://cdn/cnn.png",
        issue: "",
        reason: "",
      }),
      row({ managedChannelId: "b", issue: "player-reject", reason: "SVG" }),
    ];
    const next = rows.map((r) =>
      r.managedChannelId === "a" ? patchLogoAfterSet(r, null) : r,
    );
    assert.equal(next[0].issue, "missing");
    assert.equal(next[0].currentLogo, null);
    assert.equal(next[1].issue, "player-reject");
  });
});
