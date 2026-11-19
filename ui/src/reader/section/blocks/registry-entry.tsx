import type { Spec } from "@json-render/core";
import { JSONUIProvider, Renderer } from "@json-render/react";
import { useMemo } from "react";

import type { CatalogName } from "@/catalog/catalog";
import { registry } from "@/catalog/registry";

/**
 * One catalog entry rendered through the registry from props the core
 * already validated: a figure, a Mermaid diagram, a check question or a
 * citation that reached the interface as its own block or mark.
 */
export function RegistryEntry({
  type,
  props,
}: {
  type: CatalogName;
  props: Record<string, unknown>;
}) {
  const spec = useMemo<Spec>(
    () => ({
      root: "entry",
      elements: { entry: { type, props, children: [] } },
    }),
    [type, props],
  );
  return (
    <JSONUIProvider registry={registry}>
      <Renderer spec={spec} registry={registry} />
    </JSONUIProvider>
  );
}
