import { describe, expect, it } from "vitest";

import { skippedTail } from "./importSkips";

describe("skippedTail", () => {
  it("says nothing when no file was skipped", () => {
    expect(skippedTail([])).toBe("");
  });

  it("names the first skipped file and why, after the count", () => {
    expect(skippedTail(["a.mp3: not a file", "b.txt: \"txt\" is not a format rekordbox plays"]))
      .toBe("; 2 skipped (a.mp3: not a file)");
  });
});
