import type { ComponentFn } from "@json-render/react";

import type { AtlasCatalog } from "../../catalog.ts";
import { PlainSection } from "../../plain-section.tsx";
import {
  BASELINE,
  Drawing,
  Lines,
  MARGIN,
  TextAlternative,
  WIDTH,
  charsIn,
  linesHeight,
  round,
  wrap,
} from "../drawing.tsx";

/** The numbered marker of each step. */
const RADIUS = 12;
/** Up to this many steps sit side by side; more run down the page. */
const ACROSS = 4;
/** Between a step's label and its note, and between steps down the page. */
const NOTE_GAP = 4;
const STEP_GAP = 20;

interface Step {
  label: string;
  note: string;
}

interface PlacedStep extends Step {
  marker: { x: number; y: number };
  text: { x: number; y: number };
  labelLines: string[];
  noteLines: string[];
}

function textOf(step: Step, chars: number) {
  const labelLines = wrap(step.label, chars, 3);
  const noteLines = wrap(step.note, chars, 8);
  const height =
    linesHeight(labelLines.length) + NOTE_GAP + linesHeight(noteLines.length);
  return { labelLines, noteLines, height };
}

/** Few steps: side by side under a line through their markers. */
function across(steps: Step[]) {
  const column = (WIDTH - 2 * MARGIN) / steps.length;
  const chars = charsIn(column - 16);
  const markerY = MARGIN + RADIUS;
  const textTop = MARGIN + 2 * RADIUS + 12;
  let bottom = textTop;
  const placed = steps.map((step, index): PlacedStep => {
    const x = round(MARGIN + index * column);
    const text = textOf(step, chars);
    bottom = Math.max(bottom, textTop + text.height);
    return {
      ...step,
      ...text,
      marker: { x: x + RADIUS, y: markerY },
      text: { x, y: textTop },
    };
  });
  return { placed, height: bottom + MARGIN };
}

/** Many steps: one under the other beside a line through their markers. */
function down(steps: Step[]) {
  const textX = MARGIN + 2 * RADIUS + 16;
  const chars = charsIn(WIDTH - MARGIN - textX);
  let top = MARGIN;
  const placed = steps.map((step): PlacedStep => {
    const text = textOf(step, chars);
    const result = {
      ...step,
      ...text,
      marker: { x: MARGIN + RADIUS, y: top + RADIUS },
      text: { x: textX, y: top + 3 },
    };
    top += Math.max(2 * RADIUS, text.height + 3) + STEP_GAP;
    return result;
  });
  return { placed, height: top - STEP_GAP + MARGIN };
}

/**
 * Ordered steps shown all at once: numbered markers joined by one line,
 * each step's label and note beside its marker. Nothing steps or moves.
 */
export const StepThrough: ComponentFn<AtlasCatalog, "step-through"> = ({
  props,
}) => {
  const { placed, height } =
    props.steps.length <= ACROSS ? across(props.steps) : down(props.steps);
  const first = placed[0].marker;
  const last = placed[placed.length - 1].marker;

  return (
    <PlainSection heading={props.caption}>
      <Drawing height={height}>
        <path
          d={`M${first.x} ${first.y}L${last.x} ${last.y}`}
          className="fill-none stroke-current opacity-60"
        />
        {placed.map((step, index) => {
          const labelY = step.text.y + BASELINE;
          const noteY =
            step.text.y +
            linesHeight(step.labelLines.length) +
            NOTE_GAP +
            BASELINE;
          return (
            <g key={index}>
              <title>{`Step ${index + 1}`}</title>
              <circle
                cx={step.marker.x}
                cy={step.marker.y}
                r={RADIUS}
                className="fill-canvas stroke-current"
              />
              <Lines
                x={step.marker.x}
                y={step.marker.y + 4}
                lines={[String(index + 1)]}
                anchor="middle"
                className="fill-body"
              />
              <Lines
                x={step.text.x}
                y={labelY}
                lines={step.labelLines}
                className="fill-body uppercase"
              />
              <Lines
                x={step.text.x}
                y={noteY}
                lines={step.noteLines}
                className="fill-muted-foreground"
              />
            </g>
          );
        })}
      </Drawing>
      <TextAlternative>
        <ol aria-label="Steps">
          {props.steps.map((step, index) => (
            <li key={index}>
              {step.label}: {step.note}
            </li>
          ))}
        </ol>
      </TextAlternative>
    </PlainSection>
  );
};
