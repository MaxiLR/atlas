import type { ComponentFn } from "@json-render/react";

import type { AtlasCatalog } from "../../catalog.ts";
import { PlainSection } from "../../plain-section.tsx";

export const ConceptMap: ComponentFn<AtlasCatalog, "concept-map"> = ({
  props,
}) => {
  const labels = new Map(props.nodes.map((node) => [node.id, node.label]));
  return (
    <PlainSection heading={props.caption}>
      <ul aria-label="Concepts">
        {props.nodes.map((node, index) => (
          <li key={index}>{node.label}</li>
        ))}
      </ul>
      <ul aria-label="Links">
        {props.links.map((link, index) => (
          <li key={index}>
            {labels.get(link.from) ?? link.from} {link.label}{" "}
            {labels.get(link.to) ?? link.to}
          </li>
        ))}
      </ul>
    </PlainSection>
  );
};
