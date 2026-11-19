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
  wrap,
} from "../drawing.tsx";

/**
 * Each layer is a slab seen from above and in front: a top face `DEPTH`
 * deep, set back by `SKEW`, over a front edge `THICKNESS` high.
 */
const SLAB = 156;
const SKEW = 40;
const DEPTH = 24;
const THICKNESS = 10;
/** Between one slab's text and the next slab. */
const STACK_GAP = 16;
/** Between a layer's label and its note. */
const NOTE_GAP = 4;
/** Labels and notes, right of the stack, each reached by a leader. */
const TEXT_X = MARGIN + SLAB + SKEW + 28;
const CHARS = charsIn(WIDTH - MARGIN - TEXT_X);

/** A closed outline through the points. */
function face(points: [number, number][]): string {
  return `${points.map(([x, y], index) => `${index === 0 ? "M" : "L"}${x} ${y}`).join("")}Z`;
}

/** Each layer's slab top and text baselines, top to bottom. */
function stack(layers: { label: string; note: string }[]) {
  let top = MARGIN;
  const placed = layers.map((layer) => {
    const labelLines = wrap(layer.label, CHARS, 3);
    const noteLines = wrap(layer.note, CHARS, 8);
    const baseline = top + DEPTH / 2 + 4;
    const noteBaseline = baseline + linesHeight(labelLines.length) + NOTE_GAP;
    const textBottom = noteBaseline - BASELINE + linesHeight(noteLines.length);
    const slab = {
      ...layer,
      top,
      baseline,
      noteBaseline,
      labelLines,
      noteLines,
    };
    top = Math.max(top + DEPTH + THICKNESS, textBottom) + STACK_GAP;
    return slab;
  });
  return { layers: placed, height: top - STACK_GAP + MARGIN };
}

/**
 * Ordered layers as a stack of slabs, the first on top, drawn apart so each
 * shows; a leader runs from each slab to its label in uppercase mono and its
 * note.
 */
export const LayeredFigure: ComponentFn<AtlasCatalog, "layered-figure"> = ({
  props,
}) => {
  const { layers, height } = stack(props.layers);

  const left = MARGIN;
  const front = MARGIN + SLAB;
  const back = MARGIN + SLAB + SKEW;

  return (
    <PlainSection heading={props.caption}>
      <Drawing height={height}>
        {layers.slice(1).map((below, index) => {
          const above = layers[index];
          return (
            <path
              key={index}
              d={`M${left} ${above.top + DEPTH + THICKNESS}L${left} ${below.top + DEPTH}M${back} ${above.top + THICKNESS}L${back} ${below.top}`}
              strokeDasharray="2 4"
              className="fill-none stroke-current opacity-50"
            />
          );
        })}
        {layers.map((layer, index) => {
          const y = layer.top;
          return (
            <g key={index}>
              <title>{layer.label}</title>
              <path
                d={face([
                  [left + SKEW, y],
                  [back, y],
                  [front, y + DEPTH],
                  [left, y + DEPTH],
                ])}
                className="fill-raised stroke-current"
              />
              <path
                d={face([
                  [left, y + DEPTH],
                  [front, y + DEPTH],
                  [front, y + DEPTH + THICKNESS],
                  [left, y + DEPTH + THICKNESS],
                ])}
                className="fill-canvas stroke-current"
              />
              <path
                d={face([
                  [front, y + DEPTH],
                  [back, y],
                  [back, y + THICKNESS],
                  [front, y + DEPTH + THICKNESS],
                ])}
                className="fill-canvas stroke-current"
              />
              <path
                d={`M${front + SKEW / 2} ${y + DEPTH / 2}L${TEXT_X - 8} ${y + DEPTH / 2}`}
                className="fill-none stroke-current opacity-60"
              />
              <Lines
                x={TEXT_X}
                y={layer.baseline}
                lines={layer.labelLines}
                className="fill-body uppercase"
              />
              <Lines
                x={TEXT_X}
                y={layer.noteBaseline}
                lines={layer.noteLines}
                className="fill-muted-foreground"
              />
            </g>
          );
        })}
      </Drawing>
      <TextAlternative>
        <ol aria-label="Layers, top first">
          {props.layers.map((layer, index) => (
            <li key={index}>
              {layer.label}: {layer.note}
            </li>
          ))}
        </ol>
      </TextAlternative>
    </PlainSection>
  );
};
