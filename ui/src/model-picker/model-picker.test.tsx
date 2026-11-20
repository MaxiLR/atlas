import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, describe, expect, it } from "vitest";

import { AtlasStoreProvider } from "@/app/store";
import type { CoreClient } from "@/core/client";
import { CoreProvider } from "@/core/context";
import { createFakeCore, createManualClock } from "@/core/fake";
import type { CoreError } from "@/core/generated/CoreError";
import type { CoreRequest } from "@/core/generated/CoreRequest";
import type { Model } from "@/core/generated/Model";
import { drained, startedAt } from "@/test/fake-core";
import { applyStyles, clearStyles, endAnimation } from "@/test/styles";

import { ModelPicker } from "./model-picker";

beforeAll(() => {
  // Radix measures floating content; jsdom has no layout to observe.
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
});

afterEach(clearStyles);

const offline: CoreError = { type: "Offline" };

/**
 * Pickers over one store on the fake core, whose plan lists `models` with the
 * first in use. Every request goes to the fake core, except that
 * `ListModels` waits until the test answers it: `list()` lets the core answer
 * the oldest one waiting, `fail(error)` refuses it. `show(count)` renders
 * that many pickers over the same store; `requests` records every request
 * the pickers sent.
 */
function renderPicker(models?: Model[]) {
  const core = createFakeCore({
    clock: createManualClock(startedAt),
    pace: () => Promise.resolve(),
  });
  if (models !== undefined) {
    core.world.models = { models, current: models[0]?.slug ?? null };
  }
  const requests: CoreRequest[] = [];
  const waiting: PromiseWithResolvers<void>[] = [];
  const client: CoreClient = {
    call: ((request: CoreRequest) => {
      requests.push(request);
      if (request.type !== "ListModels") return core.call(request);
      const held = Promise.withResolvers<void>();
      waiting.push(held);
      return held.promise.then(() => core.call(request));
    }) as CoreClient["call"],
    subscribe: (listener) => core.subscribe(listener),
  };
  const tree = (count: number) => (
    <CoreProvider client={client}>
      <AtlasStoreProvider>
        {Array.from({ length: count }, (_, index) => (
          <ModelPicker key={index} />
        ))}
      </AtlasStoreProvider>
    </CoreProvider>
  );
  const { rerender } = render(tree(1));
  const answer = async (settle: (held: PromiseWithResolvers<void>) => void) => {
    await waitFor(() => expect(waiting.length).toBeGreaterThan(0));
    await act(async () => {
      settle(waiting.shift()!);
      await drained();
    });
  };
  return {
    core,
    requests,
    user: userEvent.setup(),
    show: (count: number) => rerender(tree(count)),
    list: () => answer((held) => held.resolve()),
    fail: (error: CoreError) => answer((held) => held.reject(error)),
  };
}

/** The static model label shown without a menu, by its whole text. */
function label(text: string): HTMLElement {
  return screen.getByText(
    (_, element) =>
      element !== null &&
      element.textContent === text &&
      [...element.children].every((child) => child.textContent !== text),
  );
}

describe("ModelPicker", () => {
  it("shows a neutral placeholder while the list loads, then the model in use", async () => {
    const picker = renderPicker();

    expect(picker.requests).toContainEqual({ type: "ListModels" });
    const placeholder = screen.getByRole("button", { name: "Model" });
    expect(placeholder).toBeDisabled();
    expect(placeholder).toHaveAttribute("aria-busy", "true");
    expect(screen.queryByText(/GPT/)).toBeNull();

    await picker.list();

    const control = screen.getByRole("button", { name: "Model: GPT-5.5" });
    expect(control).toBeEnabled();
    expect(control).not.toHaveAttribute("aria-busy", "true");
    expect(control).toHaveTextContent("GPT-5.5");
  });

  it("shows the last known model while the list loads again", async () => {
    const picker = renderPicker();
    await picker.list();
    picker.show(0);
    picker.requests.length = 0;

    picker.show(1);

    expect(picker.requests).toEqual([{ type: "ListModels" }]);
    const control = screen.getByRole("button", { name: "Model: GPT-5.5" });
    expect(control).toHaveAttribute("aria-busy", "true");

    await picker.list();
    expect(control).not.toHaveAttribute("aria-busy", "true");
  });

  it("lists several models in the reported order with the one in use marked", async () => {
    const picker = renderPicker();
    await picker.list();

    const control = screen.getByRole("button", { name: "Model: GPT-5.5" });
    expect(control).toHaveAttribute("aria-haspopup", "menu");
    await picker.user.click(control);

    const menu = await screen.findByRole("menu");
    const items = within(menu).getAllByRole("menuitemradio");
    expect(items.map((item) => item.textContent)).toEqual([
      "GPT-5.5",
      "GPT-5.4",
      "GPT-5.4 mini",
    ]);
    expect(items.map((item) => item.getAttribute("aria-checked"))).toEqual([
      "true",
      "false",
      "false",
    ]);
  });

  it("follows the core's order rather than sorting the models", async () => {
    const picker = renderPicker([
      { slug: "zeta", displayName: "Zeta" },
      { slug: "alpha", displayName: "Alpha" },
      { slug: "mu", displayName: "Mu" },
    ]);
    await picker.list();

    await picker.user.click(
      screen.getByRole("button", { name: "Model: Zeta" }),
    );
    const items = within(await screen.findByRole("menu")).getAllByRole(
      "menuitemradio",
    );
    expect(items.map((item) => item.textContent)).toEqual([
      "Zeta",
      "Alpha",
      "Mu",
    ]);
  });

  it("choosing another model selects it and the control names it", async () => {
    const picker = renderPicker();
    await picker.list();

    const control = screen.getByRole("button", { name: "Model: GPT-5.5" });
    await picker.user.click(control);
    const menu = await screen.findByRole("menu");
    await picker.user.click(
      within(menu).getByRole("menuitemradio", { name: "GPT-5.4 mini" }),
    );
    endAnimation(menu);

    expect(picker.requests).toContainEqual({
      type: "SelectModel",
      slug: "gpt-5.4-mini",
    });
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Model: GPT-5.4 mini" }),
      ).toBeInTheDocument(),
    );
    expect(picker.core.world.models.current).toBe("gpt-5.4-mini");

    await picker.user.click(
      screen.getByRole("button", { name: "Model: GPT-5.4 mini" }),
    );
    expect(
      within(await screen.findByRole("menu")).getByRole("menuitemradio", {
        name: "GPT-5.4 mini",
      }),
    ).toHaveAttribute("aria-checked", "true");
  });

  it("with one model shows it without a menu", async () => {
    const picker = renderPicker([{ slug: "gpt-5", displayName: "GPT-5" }]);
    await picker.list();

    expect(screen.queryByRole("button")).toBeNull();
    expect(label("Model: GPT-5")).toBeVisible();
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("when the list fails before any is known, says so with a neutral name", async () => {
    const picker = renderPicker();
    await picker.fail(offline);

    const control = screen.getByRole("button", { name: "Model" });
    expect(control).toBeEnabled();
    expect(control).toHaveAccessibleDescription(
      "The model list could not be loaded.",
    );
  });

  it("when the list fails, shows the model in use, says so and can try again", async () => {
    const picker = renderPicker();
    await picker.list();
    picker.show(0);
    picker.show(1);
    await picker.fail(offline);

    const control = screen.getByRole("button", { name: "Model: GPT-5.5" });
    expect(control).toHaveAccessibleDescription(
      "The model list could not be loaded.",
    );

    await picker.user.click(control);
    const menu = await screen.findByRole("menu");
    expect(menu).toHaveTextContent("The model list could not be loaded.");
    expect(within(menu).queryAllByRole("menuitemradio")).toEqual([]);
    picker.requests.length = 0;
    await picker.user.click(
      within(menu).getByRole("menuitem", { name: "Try again" }),
    );
    endAnimation(menu);
    expect(picker.requests).toEqual([{ type: "ListModels" }]);
    await picker.list();

    const listed = screen.getByRole("button", { name: "Model: GPT-5.5" });
    expect(listed).not.toHaveAccessibleDescription();
    await picker.user.click(listed);
    expect(
      within(await screen.findByRole("menu")).getAllByRole("menuitemradio"),
    ).toHaveLength(3);
  });

  it("names a long model in full and truncates what it shows", async () => {
    const long =
      "GPT-5.5 Thinking with extended context for long research sessions";
    const picker = renderPicker([
      { slug: "long", displayName: long },
      { slug: "gpt-5.4", displayName: "GPT-5.4" },
    ]);
    await picker.list();
    await applyStyles("dark");

    const control = screen.getByRole("button", { name: `Model: ${long}` });
    expect(control).toHaveAttribute("title", long);
    expect(getComputedStyle(control).maxWidth).not.toBe("none");
    const name = within(control).getByText(long);
    expect(getComputedStyle(name).overflow).toBe("hidden");
    expect(getComputedStyle(name).textOverflow).toBe("ellipsis");
    expect(getComputedStyle(name).whiteSpace).toBe("nowrap");

    await picker.user.click(control);
    const menu = await screen.findByRole("menu");
    await applyStyles("dark");
    expect(getComputedStyle(menu).maxWidth).not.toBe("none");
    const item = within(menu).getByRole("menuitemradio", { name: long });
    const itemName = within(item).getByText(long);
    expect(getComputedStyle(itemName).textOverflow).toBe("ellipsis");
  });

  describe("keyboard", () => {
    it("opens, moves, chooses and returns focus to the control", async () => {
      const picker = renderPicker();
      await picker.list();

      const control = screen.getByRole("button", { name: "Model: GPT-5.5" });
      control.focus();
      await picker.user.keyboard("{Enter}");
      const menu = await screen.findByRole("menu");
      const current = within(menu).getByRole("menuitemradio", {
        name: "GPT-5.5",
      });
      await waitFor(() => expect(current).toHaveFocus());

      await picker.user.keyboard("{ArrowDown}");
      expect(
        within(menu).getByRole("menuitemradio", { name: "GPT-5.4" }),
      ).toHaveFocus();
      await picker.user.keyboard("{Enter}");
      endAnimation(menu);

      expect(picker.requests).toContainEqual({
        type: "SelectModel",
        slug: "gpt-5.4",
      });
      const updated = await screen.findByRole("button", {
        name: "Model: GPT-5.4",
      });
      await waitFor(() => expect(updated).toHaveFocus());
    });

    it("Escape closes the menu and returns focus to the control", async () => {
      const picker = renderPicker();
      await picker.list();

      const control = screen.getByRole("button", { name: "Model: GPT-5.5" });
      control.focus();
      await picker.user.keyboard("{ArrowDown}");
      const menu = await screen.findByRole("menu");
      await picker.user.keyboard("{Escape}");
      endAnimation(menu);

      await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
      expect(control).toHaveFocus();
      expect(picker.requests).not.toContainEqual(
        expect.objectContaining({ type: "SelectModel" }),
      );
    });
  });
});
