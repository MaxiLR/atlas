import { useId } from "react";

import { Button } from "@/components/ui/button";
import type { ExplainStyle } from "@/core/generated/ExplainStyle";

const styles: [ExplainStyle, string][] = [
  ["Simpler", "Simpler"],
  ["MoreTechnical", "More technical"],
  ["MoreExamples", "More examples"],
];

/** Explain this section differently: three styles, each a new version. */
export function Explain({
  onExplain,
}: {
  onExplain: (style: ExplainStyle) => void;
}) {
  const labelId = useId();
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
        <Button key={style} onClick={() => onExplain(style)}>
          {label}
        </Button>
      ))}
    </div>
  );
}
