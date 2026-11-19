import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";

import {
  applyStyles,
  clearStyles,
  forced,
  literalColorsIn,
  paintOf,
  sameColor,
  schemes,
  token,
  tokenOf,
} from "@/test/styles";

import {
  Questionnaire,
  QuestionnaireChoice,
  QuestionnaireChoices,
  QuestionnaireItem,
  QuestionnaireTitle,
} from "./questionnaire";

afterEach(clearStyles);

const question =
  "An oceanic plate and a continental plate are moving toward each other. What happens where they meet?";
const options = [
  "The oceanic plate bends and sinks beneath the continental plate.",
  "The continental plate sinks beneath the oceanic plate.",
  "Both plates crumple upward into a mountain range.",
  "The two plates lock together and stop moving.",
];

function renderQuestion({ disabled = false } = {}) {
  render(
    <Questionnaire shortcuts="numbers">
      <QuestionnaireItem name="placement">
        <QuestionnaireTitle>{question}</QuestionnaireTitle>
        <QuestionnaireChoices>
          {options.map((option, index) => (
            <QuestionnaireChoice
              key={option}
              value={`option-${index}`}
              disabled={disabled && index === 3}
            >
              {option}
            </QuestionnaireChoice>
          ))}
        </QuestionnaireChoices>
      </QuestionnaireItem>
    </Questionnaire>,
  );
}

const row = (name: string) =>
  screen.getByRole("radio", { name }).closest("label") as HTMLElement;

const checked = () =>
  screen
    .getAllByRole("radio")
    .filter((radio) => (radio as HTMLInputElement).checked)
    .map((radio) => radio.closest("label")?.textContent);

describe.each(schemes)("in the %s theme", (scheme) => {
  it("an option row group moves with arrow keys and exposes one selected option", async () => {
    const user = userEvent.setup();
    renderQuestion();
    await applyStyles(scheme);

    const group = screen.getByRole("group", { name: question });
    expect(within(group).getAllByRole("radio")).toHaveLength(4);
    expect(checked()).toEqual([]);

    await user.click(screen.getByRole("radio", { name: options[0] }));
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("radio", { name: options[1] })).toHaveFocus();
    expect(checked()).toEqual([expect.stringContaining(options[1])]);

    await user.keyboard("{ArrowDown}{ArrowDown}{ArrowUp}");
    expect(screen.getByRole("radio", { name: options[2] })).toHaveFocus();
    expect(checked()).toEqual([expect.stringContaining(options[2])]);
  });

  it("option rows have 8 to 10 px corners and a hairline edge; the selected one takes the accent edge and wash", async () => {
    const user = userEvent.setup();
    renderQuestion();
    await applyStyles(scheme);

    for (const option of options) {
      const radius = parseFloat(
        getComputedStyle(row(option)).borderTopLeftRadius,
      );
      expect(radius).toBeGreaterThanOrEqual(8);
      expect(radius).toBeLessThanOrEqual(10);
      expect(tokenOf(paintOf(row(option)).border ?? "", row(option))).toBe(
        "edge",
      );
      expect(tokenOf(paintOf(row(option)).background, row(option))).toBe(
        "transparent",
      );
    }

    await user.click(screen.getByRole("radio", { name: options[1] }));
    const selected = row(options[1]);
    expect(tokenOf(paintOf(selected).border ?? "", selected)).toBe("accent");
    expect(tokenOf(paintOf(selected).background, selected)).toBe("accent-wash");
    expect(sameColor(paintOf(selected).color, token(selected, "heading"))).toBe(
      true,
    );
    for (const option of [options[0], options[2], options[3]])
      expect(tokenOf(paintOf(row(option)).border ?? "", row(option))).toBe(
        "edge",
      );
    expect(literalColorsIn(screen.getByRole("group"))).toEqual([]);
  });

  it("option rows change fill on hover and show keyboard focus in the accent", async () => {
    const user = userEvent.setup();
    renderQuestion();
    await applyStyles(scheme);

    const first = row(options[0]);
    const rest = paintOf(first);
    first.classList.add(forced.hover);
    expect(paintOf(first).background).not.toEqual(rest.background);
    first.classList.remove(forced.hover);

    await user.tab();
    expect(screen.getByRole("radio", { name: options[0] })).toHaveFocus();
    const style = getComputedStyle(first);
    expect(style.outlineStyle).toBe("solid");
    expect(sameColor(style.outlineColor, token(first, "focus"))).toBe(true);
  });

  it("each option shows its key, and the key chooses it", async () => {
    const user = userEvent.setup();
    renderQuestion();
    await applyStyles(scheme);

    options.forEach((option, index) => {
      const key = within(row(option)).getByText(String(index + 1));
      expect(tokenOf(paintOf(key).color, key)).toBe("muted");
      expect(tokenOf(paintOf(key).border ?? "", key)).toBe("edge-strong");
    });

    screen.getByRole("group", { name: question }).focus();
    await user.keyboard("3");
    expect(checked()).toEqual([expect.stringContaining(options[2])]);
  });

  it("a disabled option is exposed as disabled and looks it", async () => {
    renderQuestion({ disabled: true });
    await applyStyles(scheme);

    expect(screen.getByRole("radio", { name: options[3] })).toBeDisabled();
    const look = (option: string) => ({
      ...paintOf(row(option)),
      opacity: getComputedStyle(row(option)).opacity,
    });
    expect(look(options[3])).not.toEqual(look(options[0]));
  });
});
