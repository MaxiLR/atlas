import type { ComponentFn } from "@json-render/react";

import type { AtlasCatalog } from "../../catalog.ts";
import { PlainSection } from "../../plain-section.tsx";

export const Callout: ComponentFn<AtlasCatalog, "callout"> = ({ props }) => (
  <PlainSection heading={props.title}>
    <p>{props.body}</p>
  </PlainSection>
);
