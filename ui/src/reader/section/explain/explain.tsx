import { useId, useState } from "react";

import { Button } from "@/components/ui/button";
import type { ExplainStyle } from "@/core/generated/ExplainStyle";

const styles: [ExplainStyle, string][] = [
  ["Simpler", "Simpler"],
  ["MoreTechnical", "More technical"],
  ["MoreExamples", "More examples"],
];

/**
 * Explain this section differently: three styles, each a new version. Once
 * one is chosen, none can be until the request it starts settles.
 */
export function Explain({
  onExplain,
}: {
  onExplain: (style: ExplainStyle) => Promise<unknown>;
}) {
  const labelId = useId();
  const [pending, setPending] = useState(false);
  const settle = () => setPending(false);
  const explain = (style: ExplainStyle) => {
    setPending(true);
    onExplain(style).then(settle, settle);
  };
  return (
    <div
      role="group"
      aria-labelledby={labelId}
      className="mt-11 flex flex-wrap items-center gap-2 border-t border-edge pt-5 text-13 text-muted-foreground"
    >
      <span id={labelId} className="mr-1.5">
        Explain this section differently
      </span>
      {styles.map(([style, label]) => (
        <Button key={style} disabled={pending} onClick={() => explain(style)}>
          {label}
        </Button>
      ))}
    </div>
  );
}
