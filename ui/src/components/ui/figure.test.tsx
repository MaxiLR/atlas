import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import {
  applyStyles,
  clearStyles,
  literalColorsIn,
  paintOf,
  schemes,
  tokenOf,
} from "@/test/styles";

import { Figure } from "./figure";

afterEach(clearStyles);

const caption =
  "An oceanic plate subducting beneath a continental plate. Not to scale.";

function renderFigure() {
  render(
    <Figure label="Fig. 1" caption={caption}>
      <svg role="img" aria-label="Cross-section of a subduction zone">
        <path d="M0 96H300" stroke="currentColor" />
      </svg>
    </Figure>,
  );
  return screen.getByRole("figure");
}

describe.each(schemes)("in the %s theme", (scheme) => {
  it("is a figure named by its number and caption, with its drawing inside", async () => {
    const figure = renderFigure();
    await applyStyles(scheme);

    expect(figure).toHaveAccessibleName(`Fig. 1 ${caption}`);
    expect(
      screen.getByRole("img", { name: "Cross-section of a subduction zone" }),
    ).toBeVisible();
  });

  it("is a panel: 12 px corners, a hairline edge, the panel face and no shadow", async () => {
    const figure = renderFigure();
    await applyStyles(scheme);

    const style = getComputedStyle(figure);
    expect(style.borderTopLeftRadius).toBe("12px");
    expect(style.borderTopWidth).toBe("1px");
    expect(tokenOf(paintOf(figure).border ?? "", figure)).toBe("edge");
    expect(tokenOf(paintOf(figure).background, figure)).toBe("panel");
    expect(["none", ""]).toContain(
      style.boxShadow.replaceAll(/rgba\(0, 0, 0, 0\) 0px 0px(, )?/g, ""),
    );
    expect(literalColorsIn(figure)).toEqual([]);
  });

  it("has a caption bar under a hairline with the number in uppercase mono", async () => {
    const figure = renderFigure();
    await applyStyles(scheme);

    const bar = figure.querySelector("figcaption")!;
    expect(getComputedStyle(bar).borderTopWidth).toBe("1px");
    expect(tokenOf(paintOf(bar).border ?? "", bar)).toBe("edge");
    expect(tokenOf(paintOf(bar).color, bar)).toBe("muted");

    const number = screen.getByText("Fig. 1");
    const style = getComputedStyle(number);
    expect(style.fontFamily).toMatch(/^"?IBM Plex Mono/);
    expect(style.fontSize).toBe("12px");
    expect(style.textTransform).toBe("uppercase");
    expect(bar).toHaveTextContent(caption);
  });
});
