import type { ComponentFn } from "@json-render/react";

import type { AtlasCatalog } from "../../catalog.ts";
import { PlainSection } from "../../plain-section.tsx";

/** The caption only: the SVG is never inserted in this form. */
export const Figure: ComponentFn<AtlasCatalog, "figure"> = ({ props }) => (
  <PlainSection heading={props.caption} />
);
