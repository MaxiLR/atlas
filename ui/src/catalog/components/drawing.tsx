/**
 * What the drawn catalog visuals share: the drawing's width, the metrics of
 * its labels, line wrapping, and the SVG element itself.
 *
 * A drawing is laid out from its props alone, never measured in the page,
 * so the same spec always draws the same figure. Every label is 12 px IBM
 * Plex Mono tracked 0.04 em, the figure frame's label type, whose fixed
 * advance makes a line's width known before it is drawn.
 */
import type { ReactNode } from "react";

/** The drawing's coordinate width: the reading column's. */
export const WIDTH = 640;

/** Space kept clear inside the drawing's edges. */
export const MARGIN = 16;

/** Distance between the baselines of wrapped lines. */
export const LINE = 16;

/** Plex Mono's 600-unit advance at 12 px, plus 0.04 em tracking. */
const ADVANCE = 12 * 0.6 + 12 * 0.04;

/** From a line box's top to its baseline, with room for the ascenders. */
export const BASELINE = 13;

/** A line's descenders below its baseline. */
export const DESCENT = 4;

/** The height of `count` lines set from a box's top. */
export function linesHeight(count: number): number {
  return BASELINE + (count - 1) * LINE + DESCENT;
}

/** The drawn width of a line of label type. */
export function textWidth(line: string): number {
  return Math.ceil([...line].length * ADVANCE);
}

/** The most characters of label type that fit in `width`. */
export function charsIn(width: number): number {
  return Math.max(1, Math.floor(width / ADVANCE));
}

/** The widest of the lines. */
export function linesWidth(lines: string[]): number {
  return Math.max(0, ...lines.map(textWidth));
}

/**
 * `text` broken at spaces into lines of at most `chars` characters, a word
 * longer than a line split across lines. Past `maxLines` the last line ends
 * in an ellipsis; the caller keeps the full text available.
 */
export function wrap(text: string, chars: number, maxLines = Infinity) {
  const lines: string[] = [];
  let line = "";
  for (const word of text.trim().split(/\s+/)) {
    let rest = [...word];
    if (line && [...line].length + 1 + rest.length <= chars) {
      line += ` ${word}`;
      continue;
    }
    if (line) lines.push(line);
    while (rest.length > chars) {
      lines.push(rest.slice(0, chars).join(""));
      rest = rest.slice(chars);
    }
    line = rest.join("");
  }
  if (line || lines.length === 0) lines.push(line);
  if (lines.length <= maxLines) return lines;
  const kept = lines.slice(0, maxLines);
  const last = [...kept[maxLines - 1]].slice(0, chars - 1).join("");
  kept[maxLines - 1] = `${last.trimEnd()}…`;
  return kept;
}

/** A coordinate as written in the drawing: two decimals at most. */
export function round(value: number): number {
  return Math.round(value * 100) / 100;
}

/**
 * Lines of label type, each its own `tspan`, the first baseline at `y`.
 * `className` sets their colour and case.
 */
export function Lines({
  x,
  y,
  lines,
  anchor = "start",
  className,
}: {
  x: number;
  y: number;
  lines: string[];
  anchor?: "start" | "middle";
  className: string;
}) {
  return (
    <text textAnchor={anchor} className={className}>
      {lines.map((line, index) => (
        <tspan
          key={index}
          x={round(x)}
          y={round(y + index * LINE)}
          className={className}
        >
          {line}
        </tspan>
      ))}
    </text>
  );
}

/**
 * The drawing: thin strokes in the text colour and label type, scaled to
 * the width it is given. It is hidden from assistive technology; each
 * visual gives its content as text beside it.
 */
export function Drawing({
  height,
  children,
}: {
  height: number;
  children: ReactNode;
}) {
  return (
    <svg
      viewBox={`0 0 ${WIDTH} ${round(height)}`}
      aria-hidden="true"
      focusable="false"
      className="block h-auto w-full font-mono text-12 tracking-[0.04em] text-body"
    >
      {children}
    </svg>
  );
}

/** Lines of text read by assistive technology only. */
export function TextAlternative({ children }: { children: ReactNode }) {
  return <div className="sr-only">{children}</div>;
}
