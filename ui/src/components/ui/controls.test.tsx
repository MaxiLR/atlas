import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MoreHorizontalIcon } from "lucide-react";
import { cloneElement, type ReactElement } from "react";
import { afterEach, describe, expect, it } from "vitest";

import {
  applyStyles,
  clearStyles,
  forced,
  literalColors,
  literalColorsIn,
  paintOf,
  sameColor,
  schemes,
  token,
  tokenOf,
} from "@/test/styles";

import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "./breadcrumb";
import { Button } from "./button";
import { Input } from "./input";
import { Kbd } from "./kbd";
import { Progress } from "./progress";
import { Separator } from "./separator";
import { Skeleton } from "./skeleton";
import { Spinner } from "./spinner";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./tabs";
import { Textarea } from "./textarea";

afterEach(clearStyles);

/** A paint and the opacity it is shown at, to compare two states. */
function lookOf(element: Element) {
  return { ...paintOf(element), opacity: getComputedStyle(element).opacity };
}

function expectAccentFocus(element: Element) {
  const style = getComputedStyle(element);
  expect(style.outlineStyle).toBe("solid");
  expect(style.outlineWidth).toBe("2px");
  expect(sameColor(style.outlineColor, token(element, "focus"))).toBe(true);
}

type ButtonCase = {
  variant: string;
  name: string;
  ui: ReactElement<Record<string, unknown>>;
  toggles: boolean;
};

const buttons: ButtonCase[] = [
  {
    variant: "default",
    name: "Glossary",
    ui: <Button>Glossary</Button>,
    toggles: true,
  },
  {
    variant: "primary",
    name: "Next",
    ui: <Button variant="primary">Next</Button>,
    toggles: false,
  },
  {
    variant: "ghost",
    name: "Library",
    ui: <Button variant="ghost">Library</Button>,
    toggles: true,
  },
  {
    variant: "icon",
    name: "Course menu",
    ui: (
      <Button variant="ghost" size="icon" aria-label="Course menu">
        <MoreHorizontalIcon />
      </Button>
    ),
    toggles: true,
  },
];

describe.each(schemes)("in the %s theme", (scheme) => {
  describe("a button", () => {
    it.each(buttons)(
      "($variant) is a 28 px control with 6 px corners painted with tokens",
      async ({ ui, name, variant }) => {
        render(ui);
        await applyStyles(scheme);

        const button = screen.getByRole("button", { name });
        const style = getComputedStyle(button);
        expect(style.height).toBe("28px");
        if (variant === "icon") expect(style.width).toBe("28px");
        expect(style.borderTopLeftRadius).toBe("6px");
        expect(literalColors(button)).toEqual([]);
      },
    );

    it.each(buttons)(
      "($variant) changes fill or brightness on hover and scales to 0.97 on press",
      async ({ ui, name }) => {
        render(ui);
        await applyStyles(scheme);
        const button = screen.getByRole("button", { name });
        const rest = lookOf(button);

        button.classList.add(forced.hover);
        expect(lookOf(button)).not.toEqual(rest);
        expect(literalColors(button)).toEqual([]);
        button.classList.remove(forced.hover);

        button.classList.add(forced.active);
        expect(getComputedStyle(button).scale).toBe("0.97");
      },
    );

    it.each(buttons)(
      "($variant) shows keyboard focus in the accent",
      async ({ ui, name }) => {
        const user = userEvent.setup();
        render(ui);
        await applyStyles(scheme);

        await user.tab();
        const button = screen.getByRole("button", { name });
        expect(button).toHaveFocus();
        expectAccentFocus(button);
      },
    );

    it.each(buttons)(
      "($variant) disabled is exposed as disabled, looks it and ignores hover",
      async ({ ui, name }) => {
        render(
          <>
            {ui}
            {cloneElement(ui, { disabled: true })}
          </>,
        );
        await applyStyles(scheme);

        const [enabled, disabled] = screen.getAllByRole("button", { name });
        expect(enabled).toBeEnabled();
        expect(disabled).toBeDisabled();
        const look = lookOf(disabled);
        expect(look).not.toEqual(lookOf(enabled));
        expect(literalColors(disabled)).toEqual([]);

        disabled.classList.add(forced.hover);
        expect(lookOf(disabled)).toEqual(look);
      },
    );

    it.each(buttons.filter(({ toggles }) => toggles))(
      "($variant) selected is exposed as pressed and filled with tokens",
      async ({ ui, name }) => {
        render(
          <>
            {cloneElement(ui, { "aria-pressed": false })}
            {cloneElement(ui, { "aria-pressed": true })}
          </>,
        );
        await applyStyles(scheme);

        const rest = screen.getByRole("button", { name, pressed: false });
        const selected = screen.getByRole("button", { name, pressed: true });
        expect(lookOf(selected)).not.toEqual(lookOf(rest));
        expect(
          sameColor(paintOf(selected).color, token(selected, "heading")),
        ).toBe(true);
        expect(literalColors(selected)).toEqual([]);
      },
    );

    it("(primary) takes the one primary fill", async () => {
      render(<Button variant="primary">Check answer</Button>);
      await applyStyles(scheme);

      const button = screen.getByRole("button", { name: "Check answer" });
      const paint = paintOf(button);
      expect(sameColor(paint.background, token(button, "primary-fill"))).toBe(
        true,
      );
      expect(sameColor(paint.color, token(button, "primary-text"))).toBe(true);
    });

    it("(default) sits on a wash with a hairline edge; ghost has no fill", async () => {
      render(
        <>
          <Button>Go deeper</Button>
          <Button variant="ghost">All side chats</Button>
        </>,
      );
      await applyStyles(scheme);

      const filled = screen.getByRole("button", { name: "Go deeper" });
      expect(tokenOf(paintOf(filled).background, filled)).toBe("wash");
      expect(tokenOf(paintOf(filled).border ?? "", filled)).toBe("edge");
      const ghost = screen.getByRole("button", { name: "All side chats" });
      expect(tokenOf(paintOf(ghost).background, ghost)).toBe("transparent");
    });

    it("(primary) of a form is 32 px high with the same corners", async () => {
      render(
        <Button variant="primary" size="lg" type="submit">
          Continue
        </Button>,
      );
      await applyStyles(scheme);

      const style = getComputedStyle(
        screen.getByRole("button", { name: "Continue" }),
      );
      expect(style.height).toBe("32px");
      expect(style.borderTopLeftRadius).toBe("6px");
    });
  });

  describe.each([
    ["an input", <Input aria-label="Topic" placeholder="A topic" />],
    ["a text area", <Textarea aria-label="Question" placeholder="Ask" />],
  ])("%s", (_, ui) => {
    it("has 8 to 10 px corners and a hairline edge, painted with tokens", async () => {
      render(ui);
      await applyStyles(scheme);

      const field = screen.getByRole("textbox");
      const radius = parseFloat(getComputedStyle(field).borderTopLeftRadius);
      expect(radius).toBeGreaterThanOrEqual(8);
      expect(radius).toBeLessThanOrEqual(10);
      expect(["edge", "edge-strong"]).toContain(
        tokenOf(paintOf(field).border ?? "", field),
      );
      expect(literalColors(field)).toEqual([]);
    });

    it("takes the accent edge while focused", async () => {
      const user = userEvent.setup();
      render(ui);
      await applyStyles(scheme);

      await user.tab();
      const field = screen.getByRole("textbox");
      expect(field).toHaveFocus();
      expect(
        sameColor(paintOf(field).border ?? "", token(field, "focus")),
      ).toBe(true);
    });

    it("disabled is exposed as disabled and looks it", async () => {
      render(
        <>
          {ui}
          {cloneElement(ui as ReactElement<{ disabled: boolean }>, {
            disabled: true,
          })}
        </>,
      );
      await applyStyles(scheme);

      const [enabled, disabled] = screen.getAllByRole("textbox");
      expect(disabled).toBeDisabled();
      expect(lookOf(disabled)).not.toEqual(lookOf(enabled));
      expect(literalColors(disabled)).toEqual([]);
    });
  });

  it("a key is drawn as a small hairline key in muted text", async () => {
    render(
      <p>
        Choose with <Kbd>1</Kbd>
      </p>,
    );
    await applyStyles(scheme);

    const key = screen.getByText("1");
    expect(key.tagName).toBe("KBD");
    expect(tokenOf(paintOf(key).color, key)).toBe("muted");
    expect(tokenOf(paintOf(key).border ?? "", key)).toBe("edge-strong");
    expect(parseFloat(getComputedStyle(key).borderTopLeftRadius)).toBeLessThan(
      6,
    );
  });

  describe("tabs", () => {
    function renderTabs() {
      render(
        <Tabs defaultValue="chats">
          <TabsList aria-label="Side panel">
            <TabsTrigger value="chats">Side chats</TabsTrigger>
            <TabsTrigger value="glossary">Glossary</TabsTrigger>
            <TabsTrigger value="sources" disabled>
              Sources
            </TabsTrigger>
          </TabsList>
          <TabsContent value="chats">Three side chats</TabsContent>
          <TabsContent value="glossary">Six terms</TabsContent>
        </Tabs>,
      );
    }

    it("expose the list, the selected tab and its panel", async () => {
      renderTabs();
      await applyStyles(scheme);

      expect(screen.getByRole("tablist", { name: "Side panel" })).toBeVisible();
      expect(screen.getByRole("tab", { selected: true })).toHaveAccessibleName(
        "Side chats",
      );
      expect(screen.getByRole("tabpanel")).toHaveTextContent(
        "Three side chats",
      );
      expect(screen.getByRole("tab", { name: "Sources" })).toBeDisabled();
    });

    it("are 28 px controls; the selected one is filled and in the heading colour", async () => {
      renderTabs();
      await applyStyles(scheme);

      const selected = screen.getByRole("tab", { name: "Side chats" });
      const other = screen.getByRole("tab", { name: "Glossary" });
      const disabled = screen.getByRole("tab", { name: "Sources" });
      expect(getComputedStyle(selected).height).toBe("28px");
      expect(getComputedStyle(selected).borderTopLeftRadius).toBe("6px");
      expect(lookOf(selected)).not.toEqual(lookOf(other));
      expect(
        sameColor(paintOf(selected).color, token(selected, "heading")),
      ).toBe(true);
      expect(lookOf(disabled)).not.toEqual(lookOf(other));
      for (const tab of [selected, other, disabled])
        expect(literalColors(tab)).toEqual([]);

      const rest = lookOf(other);
      other.classList.add(forced.hover);
      expect(lookOf(other)).not.toEqual(rest);
    });

    it("move by arrow keys and show focus in the accent", async () => {
      const user = userEvent.setup();
      renderTabs();
      await applyStyles(scheme);

      await user.tab();
      expect(screen.getByRole("tab", { name: "Side chats" })).toHaveFocus();
      expectAccentFocus(screen.getByRole("tab", { name: "Side chats" }));
      await user.keyboard("{ArrowRight}");
      expect(screen.getByRole("tab", { selected: true })).toHaveAccessibleName(
        "Glossary",
      );
      expect(screen.getByRole("tabpanel")).toHaveTextContent("Six terms");
    });
  });

  it("a breadcrumb names its trail, marks the current page and links in tokens", async () => {
    const user = userEvent.setup();
    render(
      <Breadcrumb>
        <BreadcrumbList>
          <BreadcrumbItem>
            <BreadcrumbLink href="#plates">
              Plates and their boundaries
            </BreadcrumbLink>
          </BreadcrumbItem>
          <BreadcrumbSeparator />
          <BreadcrumbItem>
            <BreadcrumbPage>Convergent boundaries</BreadcrumbPage>
          </BreadcrumbItem>
        </BreadcrumbList>
      </Breadcrumb>,
    );
    await applyStyles(scheme);

    const trail = screen.getByRole("navigation", { name: "Breadcrumb" });
    const link = screen.getByRole("link", {
      name: "Plates and their boundaries",
    });
    expect(screen.getByText("Convergent boundaries")).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(literalColorsIn(trail)).toEqual([]);
    expect(tokenOf(paintOf(link).color, link)).toBe("muted");

    const rest = lookOf(link);
    link.classList.add(forced.hover);
    expect(lookOf(link)).not.toEqual(rest);
    link.classList.remove(forced.hover);

    await user.tab();
    expect(link).toHaveFocus();
    expectAccentFocus(link);
  });

  it("a progress bar exposes its value on a hairline track", async () => {
    render(<Progress aria-label="5-hour window" value={38} />);
    await applyStyles(scheme);

    const bar = screen.getByRole("progressbar", { name: "5-hour window" });
    expect(bar).toHaveAttribute("aria-valuenow", "38");
    expect(tokenOf(paintOf(bar).background, bar)).toBe("edge-strong");
    const fill = bar.firstElementChild!;
    expect(tokenOf(paintOf(fill).background, fill)).toBe("body");
    expect(literalColorsIn(bar)).toEqual([]);
  });

  it("a separator is a hairline", async () => {
    render(<Separator decorative={false} />);
    await applyStyles(scheme);

    const rule = screen.getByRole("separator");
    expect(tokenOf(paintOf(rule).background, rule)).toBe("edge");
    expect(getComputedStyle(rule).height).toBe("1px");
  });

  it("a skeleton is a quiet wash", async () => {
    render(<Skeleton data-testid="placeholder" className="h-4 w-24" />);
    await applyStyles(scheme);

    const placeholder = screen.getByTestId("placeholder");
    expect(tokenOf(paintOf(placeholder).background, placeholder)).toBe("wash");
    expect(placeholder).toHaveAttribute("aria-hidden", "true");
  });

  it("a spinner announces loading in muted ink", async () => {
    render(<Spinner />);
    await applyStyles(scheme);

    const spinner = screen.getByRole("status", { name: "Loading" });
    expect(tokenOf(paintOf(spinner).color, spinner)).toBe("muted");
  });
});
