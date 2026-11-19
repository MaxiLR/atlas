import type { ComponentFn } from "@json-render/react";

import type { AtlasCatalog } from "../../catalog.ts";
import { PlainSection } from "../../plain-section.tsx";

export const StepThrough: ComponentFn<AtlasCatalog, "step-through"> = ({
  props,
}) => (
  <PlainSection heading={props.caption}>
    <ol>
      {props.steps.map((step, index) => (
        <li key={index}>
          {step.label}: {step.note}
        </li>
      ))}
    </ol>
  </PlainSection>
);
