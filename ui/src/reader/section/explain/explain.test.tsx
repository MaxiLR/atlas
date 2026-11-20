import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import type { ExplainStyle } from "@/core/generated/ExplainStyle";

import { Explain } from "./explain";

const names = ["Simpler", "More technical", "More examples"];

describe("Explain this section differently", () => {
  it("passes the chosen style on", async () => {
    const chosen: ExplainStyle[] = [];
    render(
      <Explain
        onExplain={(style) => {
          chosen.push(style);
          return Promise.resolve();
        }}
      />,
    );
    const user = userEvent.setup();

    for (const name of names) {
      await user.click(screen.getByRole("button", { name }));
    }

    expect(chosen).toEqual(["Simpler", "MoreTechnical", "MoreExamples"]);
  });

  it("makes every action unavailable once one is chosen, until the core refuses it", async () => {
    const { promise, reject } = Promise.withResolvers<void>();
    const chosen: ExplainStyle[] = [];
    render(
      <Explain
        onExplain={(style) => {
          chosen.push(style);
          return promise;
        }}
      />,
    );
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: "More examples" }));
    for (const name of names) {
      expect(screen.getByRole("button", { name })).toBeDisabled();
    }
    await user.click(screen.getByRole("button", { name: "Simpler" }));
    expect(chosen).toEqual(["MoreExamples"]);

    reject(new Error("offline"));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Simpler" })).toBeEnabled(),
    );
  });
});
