import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SearchIcon } from "lucide-react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";

import {
  applyStyles,
  clearStyles,
  endAnimation,
  forced,
  literalColorsIn,
  paintOf,
  sameColor,
  schemes,
  token,
  tokenOf,
} from "@/test/styles";

import { Chat } from "./chat";
import { InlineCitation } from "./inline-citation";
import { AgentMessage, PersonMessage } from "./message";
import { ModelSelector } from "./model-selector";
import { PromptInput } from "./prompt-input";
import { Reasoning } from "./reasoning";
import { Sources } from "./sources";
import { Suggestions } from "./suggestion";
import { Task } from "./task";
import { Tool } from "./tool";

beforeAll(() => {
  // Radix measures floating content; jsdom has no layout to observe.
  globalThis.ResizeObserver ??= class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
});

afterEach(clearStyles);

describe.each(schemes)("in the %s theme", (scheme) => {
  it("Message: the candidate's message is a filled bubble, the reply is Typeset prose", async () => {
    render(
      <>
        <PersonMessage>
          Is slab pull the main thing that moves plates?
        </PersonMessage>
        <AgentMessage>
          <p>
            For plates with a subducting edge, slab pull is the larger force.
          </p>
        </AgentMessage>
      </>,
    );
    await applyStyles(scheme);

    const question = screen.getByText(/Is slab pull the main thing/);
    expect(tokenOf(paintOf(question).background, question)).toBe("wash-strong");
    expect(sameColor(paintOf(question).color, token(question, "heading"))).toBe(
      true,
    );
    expect(getComputedStyle(question).borderTopLeftRadius).toBe("10px");

    const reply = screen.getByText(/slab pull is the larger force/);
    expect(reply.closest(".typeset")).not.toBeNull();
    expect(sameColor(paintOf(reply).color, token(reply, "body"))).toBe(true);
    expect(literalColorsIn(document.body)).toEqual([]);
  });

  it("Message: the candidate's bubble is filled with the strong wash, compared by value, and the reply is Typeset prose", async () => {
    render(
      <>
        <PersonMessage>
          Is slab pull the main thing that moves plates?
        </PersonMessage>
        <AgentMessage>
          <p>
            For plates with a subducting edge, slab pull is the larger force.
          </p>
        </AgentMessage>
      </>,
    );
    await applyStyles(scheme);

    const question = screen.getByText(/Is slab pull the main thing/);
    expect(
      sameColor(paintOf(question).background, token(question, "wash-strong")),
    ).toBe(true);
    expect(sameColor(paintOf(question).color, token(question, "heading"))).toBe(
      true,
    );
    expect(getComputedStyle(question).borderTopLeftRadius).toBe("10px");

    const reply = screen.getByText(/slab pull is the larger force/);
    expect(reply.closest(".typeset")).not.toBeNull();
    expect(sameColor(paintOf(reply).color, token(reply, "body"))).toBe(true);
    expect(literalColorsIn(document.body)).toEqual([]);
  });

  it("Message Scroller: the thread shows its turns in order with tokens only", async () => {
    render(
      <div style={{ height: 300 }}>
        <Chat
          turns={[
            {
              id: "1",
              fromPerson: true,
              content: <PersonMessage>Why?</PersonMessage>,
            },
            {
              id: "2",
              fromPerson: false,
              content: (
                <AgentMessage>
                  <p>Because the slab is dense.</p>
                </AgentMessage>
              ),
            },
          ]}
        />
      </div>,
    );
    await applyStyles(scheme);

    const why = screen.getByText("Why?");
    const because = screen.getByText("Because the slab is dense.");
    expect(
      why.compareDocumentPosition(because) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(literalColorsIn(document.body)).toEqual([]);
  });

  it("Prompt Input: a labelled field with 10 px corners that takes the accent while focused, and a primary Send", async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();
    render(<PromptInput label="Ask a follow-up" onSend={onSend} />);
    await applyStyles(scheme);

    const field = screen.getByRole("textbox", { name: "Ask a follow-up" });
    const frame = field.closest("[data-slot=input-group]") as HTMLElement;
    expect(getComputedStyle(frame).borderTopLeftRadius).toBe("10px");
    expect(tokenOf(paintOf(frame).border ?? "", frame)).toBe("edge-strong");

    const send = screen.getByRole("button", { name: "Send" });
    expect(send).toBeDisabled();
    await user.click(field);
    expect(sameColor(paintOf(frame).border ?? "", token(frame, "focus"))).toBe(
      true,
    );
    await user.type(field, "What is ridge push?");
    expect(send).toBeEnabled();
    expect(
      sameColor(paintOf(send).background, token(send, "primary-fill")),
    ).toBe(true);
    expect(getComputedStyle(send).height).toBe("28px");
    expect(literalColorsIn(frame)).toEqual([]);

    await user.click(send);
    expect(onSend).toHaveBeenCalledWith("What is ridge push?");
  });

  it("Suggestion: follow-up prompts stack as wrapping buttons and send when chosen", async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();
    const prompts = [
      "What is ridge push, and how much does it add?",
      "How do we know which plates move fastest?",
    ];
    render(<Suggestions suggestions={prompts} onSend={onSend} />);
    await applyStyles(scheme);

    for (const prompt of prompts) {
      const button = screen.getByRole("button", { name: prompt });
      expect(getComputedStyle(button).whiteSpace).toBe("normal");
      expect(getComputedStyle(button).textAlign).toBe("left");
      expect(tokenOf(paintOf(button).background, button)).toBe("wash");
    }
    const list = screen.getByRole("button", {
      name: prompts[0],
    }).parentElement!;
    expect(getComputedStyle(list).display).toBe("flex");
    expect(getComputedStyle(list).flexDirection).toBe("column");
    expect(literalColorsIn(list)).toEqual([]);

    await user.click(screen.getByRole("button", { name: prompts[1] }));
    expect(onSend).toHaveBeenCalledWith(prompts[1]);
  });

  it("Task: the steps the agent takes, under a disclosure in muted ink", async () => {
    const user = userEvent.setup();
    render(
      <Task
        title="Writing Convergent boundaries"
        steps={[
          { id: "1", icon: <SearchIcon />, title: "Searched the web" },
          { id: "2", icon: <SearchIcon />, title: "Drafted the section" },
        ]}
      />,
    );
    await applyStyles(scheme);

    const toggle = screen.getByRole("button", {
      name: /Writing Convergent boundaries/,
    });
    expect(toggle).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Drafted the section")).toBeVisible();
    expect(tokenOf(paintOf(toggle).color, toggle)).toBe("muted");
    const rest = paintOf(toggle);
    toggle.classList.add(forced.hover);
    expect(paintOf(toggle)).not.toEqual(rest);
    toggle.classList.remove(forced.hover);
    expect(literalColorsIn(document.body)).toEqual([]);

    await user.click(toggle);
    expect(toggle).toHaveAttribute("aria-expanded", "false");
  });

  it("Reasoning: the thinking opens from a disclosure", async () => {
    const user = userEvent.setup();
    render(<Reasoning text="Slab pull dominates where…" seconds={4} />);
    await applyStyles(scheme);

    const toggle = screen.getByRole("button", { name: /Thought/ });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await user.click(toggle);
    expect(screen.getByText("Slab pull dominates where…")).toBeVisible();
    await applyStyles(scheme);
    expect(literalColorsIn(document.body)).toEqual([]);
  });

  it("Tool: a muted row naming what the agent did and its result", async () => {
    render(
      <Tool
        icon={<SearchIcon />}
        title="Searched the web"
        description="2 sources"
      />,
    );
    await applyStyles(scheme);

    const title = screen.getByText("Searched the web");
    expect(tokenOf(paintOf(title).color, title)).toBe("muted");
    const detail = screen.getByText("2 sources");
    expect(tokenOf(paintOf(detail).color, detail)).toBe("muted");
    expect(literalColorsIn(document.body)).toEqual([]);
  });

  it("Inline Citation: a numbered hairline chip that opens its source on focus", async () => {
    const user = userEvent.setup();
    render(
      <p>
        About as fast as fingernails grow.
        <InlineCitation
          number={1}
          url="https://www.usgs.gov/plates"
          title="Understanding plate motions"
          site="U.S. Geological Survey"
        />
      </p>,
    );
    await applyStyles(scheme);

    const chip = screen.getByRole("button", {
      name: "Source 1: Understanding plate motions",
    });
    expect(chip).toHaveTextContent("1");
    expect(tokenOf(paintOf(chip).color, chip)).toBe("muted");
    expect(tokenOf(paintOf(chip).border ?? "", chip)).toBe("edge-strong");
    expect(parseFloat(getComputedStyle(chip).borderTopLeftRadius)).toBeLessThan(
      6,
    );

    await user.tab();
    expect(chip).toHaveFocus();
    const card = await screen.findByText("U.S. Geological Survey");
    await applyStyles(scheme);
    expect(card.closest("[data-surface]")).toHaveAttribute(
      "data-surface",
      "raised",
    );
    expect(screen.getByText("Understanding plate motions")).toBeVisible();
    expect(literalColorsIn(document.body)).toEqual([]);
  });

  it("Sources: a numbered list of linked sources with their sites", async () => {
    render(
      <Sources
        sources={[
          {
            url: "https://www.usgs.gov/plates",
            title: "Understanding plate motions",
            site: "U.S. Geological Survey",
          },
          {
            url: "https://example.org/subduction",
            title: "Subduction zones",
            site: "Reviews of Geophysics",
          },
        ]}
      />,
    );
    await applyStyles(scheme);

    const list = screen.getByRole("list");
    const items = within(list).getAllByRole("listitem");
    expect(items.map((item) => item.textContent)).toEqual([
      expect.stringContaining("Understanding plate motions"),
      expect.stringContaining("Subduction zones"),
    ]);
    expect(items[0]).toHaveTextContent("U.S. Geological Survey");
    const link = within(items[0]).getByRole("link", {
      name: /Understanding plate motions/,
    });
    expect(link).toHaveAttribute("href", "https://www.usgs.gov/plates");
    expect(sameColor(paintOf(link).color, token(link, "body"))).toBe(true);
    expect(literalColorsIn(list)).toEqual([]);
  });

  it("Model Selector: a ghost control named for the model whose menu marks the one in use", async () => {
    const user = userEvent.setup();
    const onValueChange = vi.fn();
    render(
      <ModelSelector
        models={[
          { slug: "gpt-5", displayName: "GPT-5" },
          { slug: "gpt-5-mini", displayName: "GPT-5 mini" },
        ]}
        value="gpt-5"
        onValueChange={onValueChange}
      />,
    );
    await applyStyles(scheme);

    const control = screen.getByRole("button", { name: "Model: GPT-5" });
    expect(tokenOf(paintOf(control).background, control)).toBe("transparent");
    expect(getComputedStyle(control).height).toBe("28px");

    control.focus();
    await user.keyboard("{Enter}");
    const menu = await screen.findByRole("menu");
    await applyStyles(scheme);
    expect(menu).toHaveAttribute("data-surface", "raised");
    expect(
      screen.getByRole("menuitemradio", { name: "GPT-5" }),
    ).toHaveAttribute("aria-checked", "true");
    expect(literalColorsIn(menu)).toEqual([]);

    await user.click(screen.getByRole("menuitemradio", { name: "GPT-5 mini" }));
    endAnimation(menu);
    expect(onValueChange).toHaveBeenCalledWith("gpt-5-mini");
    await waitFor(() => expect(control).toHaveFocus());
  });
});
