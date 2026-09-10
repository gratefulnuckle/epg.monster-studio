import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { colWidthFromMeasure, groupsTrackWidth, savedColWidth } from "./split.ts";

describe("colWidthFromMeasure", () => {
  it("allows 0 so a pane can be dragged closed", () => {
    assert.equal(colWidthFromMeasure(0), 0);
  });

  it("does not cap at the old 140–560 band — any positive width is kept", () => {
    assert.equal(colWidthFromMeasure(12.4), 12);
    assert.equal(colWidthFromMeasure(420), 420);
    assert.equal(colWidthFromMeasure(560), 560);
    assert.equal(colWidthFromMeasure(900), 900);
    assert.equal(colWidthFromMeasure(2400), 2400);
  });

  it("does not grow a negative drag into a minimum width", () => {
    assert.equal(colWidthFromMeasure(-40), 0);
  });
});

describe("savedColWidth", () => {
  it("restores a saved width of 0 and widths past the old max", () => {
    assert.equal(savedColWidth("0"), 0);
    assert.equal(savedColWidth("12.9"), 13);
    assert.equal(savedColWidth("1800"), 1800);
  });

  it("ignores missing or invalid saved values instead of substituting a min", () => {
    assert.equal(savedColWidth(null), null);
    assert.equal(savedColWidth(""), null);
    assert.equal(savedColWidth("nope"), null);
    assert.equal(savedColWidth("-8"), null);
  });
});

describe("groupsTrackWidth", () => {
  it("uses a groups width of 0 when measuring the channels column", () => {
    assert.equal(groupsTrackWidth("0px"), 0);
    assert.equal(groupsTrackWidth("  0 "), 0);
  });

  it("falls back only when the CSS var is missing, not when it is 0", () => {
    assert.equal(groupsTrackWidth(""), 220);
    assert.equal(groupsTrackWidth("180px"), 180);
  });
});
