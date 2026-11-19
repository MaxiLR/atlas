import type { ComponentFn } from "@json-render/react";

import type { AtlasCatalog } from "../../catalog.ts";
import { PlainSection } from "../../plain-section.tsx";

export const LayeredFigure: ComponentFn<AtlasCatalog, "layered-figure"> = ({
  props,
}) => (
  <PlainSection heading={props.caption}>
    <ol>
      {props.layers.map((layer, index) => (
        <li key={index}>
          {layer.label}: {layer.note}
        </li>
      ))}
    </ol>
  </PlainSection>
);
