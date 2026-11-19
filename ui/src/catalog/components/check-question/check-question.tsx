import type { ComponentFn } from "@json-render/react";

import type { AtlasCatalog } from "../../catalog.ts";
import { PlainSection } from "../../plain-section.tsx";

/** The stem and the options; the key and the explanation stay untold. */
export const CheckQuestion: ComponentFn<AtlasCatalog, "check-question"> = ({
  props,
}) => (
  <PlainSection heading={props.stem}>
    <ol>
      {props.options.map((option, index) => (
        <li key={index}>{option.text}</li>
      ))}
    </ol>
  </PlainSection>
);
