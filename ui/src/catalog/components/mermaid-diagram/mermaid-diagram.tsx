import type { ComponentFn } from "@json-render/react";

import type { AtlasCatalog } from "../../catalog.ts";
import { PlainSection } from "../../plain-section.tsx";

/** The diagram's source as text; nothing is run. */
export const MermaidDiagram: ComponentFn<AtlasCatalog, "mermaid-diagram"> = ({
  props,
}) => (
  <PlainSection heading={props.caption}>
    <pre>
      <code>{props.source}</code>
    </pre>
  </PlainSection>
);
