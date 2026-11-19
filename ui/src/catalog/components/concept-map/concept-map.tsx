import type { ComponentFn } from "@json-render/react";
import { useId, useMemo } from "react";

import type { AtlasCatalog } from "../../catalog.ts";
import { PlainSection } from "../../plain-section.tsx";
import { Drawing, Lines, TextAlternative, round } from "../drawing.tsx";
import { layoutConceptMap } from "./layout.ts";

/**
 * Concepts as faces joined by labelled links, drawn top to bottom; a
 * directed link ends in an arrowhead at the node it points to.
 */
export const ConceptMap: ComponentFn<AtlasCatalog, "concept-map"> = ({
  props,
}) => {
  const { nodes, links } = props;
  const layout = useMemo(() => layoutConceptMap(nodes, links), [nodes, links]);
  const arrow = useId();
  const labels = new Map(
    nodes.toReversed().map((node) => [node.id, node.label]),
  );
  const sentence = (link: (typeof links)[number]) =>
    `${labels.get(link.from) ?? link.from} ${link.label} ${labels.get(link.to) ?? link.to}`;

  return (
    <PlainSection heading={props.caption}>
      <Drawing height={layout.height}>
        <defs>
          <marker
            id={arrow}
            viewBox="0 0 10 10"
            refX={10}
            refY={5}
            markerWidth={8}
            markerHeight={8}
            orient="auto"
          >
            <path
              d="M1 1L10 5L1 9"
              className="fill-none stroke-current"
              strokeWidth={1.25}
            />
          </marker>
        </defs>
        {layout.links.map((placed) => {
          const { box, lines, baseline } = placed.label;
          return (
            <g key={placed.index}>
              <title>{sentence(links[placed.index])}</title>
              <path
                d={placed.path}
                className="fill-none stroke-current opacity-70"
                markerEnd={placed.directed ? `url(#${arrow})` : undefined}
              />
              <rect
                x={box.x}
                y={box.y}
                width={box.width}
                height={box.height}
                rx={3}
                className="fill-panel"
              />
              <Lines
                x={round(box.x + box.width / 2)}
                y={baseline}
                lines={lines}
                anchor="middle"
                className="fill-muted-foreground"
              />
            </g>
          );
        })}
        {layout.nodes.map((node, index) => (
          <g key={index}>
            <title>{node.label}</title>
            <rect
              x={node.face.x}
              y={node.face.y}
              width={node.face.width}
              height={node.face.height}
              rx={6}
              className="fill-canvas stroke-current"
            />
            <Lines
              x={round(node.face.x + node.face.width / 2)}
              y={node.baseline}
              lines={node.lines}
              anchor="middle"
              className="fill-body uppercase"
            />
          </g>
        ))}
      </Drawing>
      <TextAlternative>
        <ul aria-label="Concepts">
          {nodes.map((node, index) => (
            <li key={index}>{node.label}</li>
          ))}
        </ul>
        <ul aria-label="Links">
          {links.map((link, index) => (
            <li key={index}>{sentence(link)}</li>
          ))}
        </ul>
      </TextAlternative>
    </PlainSection>
  );
};
