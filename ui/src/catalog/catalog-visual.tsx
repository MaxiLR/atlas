import { JSONUIProvider, Renderer } from "@json-render/react";
import { useEffect, useEffectEvent, useMemo } from "react";

import { registry } from "./registry.ts";
import { checkVisualSpec } from "./visual-spec.ts";

/**
 * Renders a catalog visual through json-render with the catalog's registry
 * only. A spec that is not a catalog visual renders nothing, and `onInvalid`
 * receives the reasons.
 */
export function CatalogVisual({
  spec,
  onInvalid,
}: {
  spec: unknown;
  onInvalid?: (reasons: string[]) => void;
}) {
  const check = useMemo(() => checkVisualSpec(spec), [spec]);
  const report = useEffectEvent((reasons: string[]) => onInvalid?.(reasons));
  useEffect(() => {
    if (!check.ok) report(check.reasons);
  }, [check]);

  if (!check.ok) return null;
  return (
    <JSONUIProvider registry={registry}>
      <Renderer spec={check.spec} registry={registry} />
    </JSONUIProvider>
  );
}
