import { describe, expect, it } from "vitest";

import { citationsOf } from "./world";

describe("the fake core's citations", () => {
  it("count Unicode characters, as the core does, not UTF-16 units", () => {
    const citations = citationsOf(
      {
        type: "Prose",
        chunks: ["🌋 Mount St. Helens erupted in 1980, killing 57 people."],
        cites: [{ phrase: "killing 57 people", source: 1 }],
      },
      3,
    );

    // 🌋 is one Unicode character and two UTF-16 units.
    expect(citations).toEqual([{ block: 3, start: 36, end: 53, source: 1 }]);
  });
});
