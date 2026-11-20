import { useId, type ReactNode } from "react";

import { useCaptionInFrame } from "./frame-context.ts";

/**
 * A visual in its plainest accessible form: its content as text under a
 * heading. The registry's first components all use it. Inside a figure
 * frame, whose caption names the visual, the content stands alone.
 */
export function PlainSection({
  heading,
  children,
}: {
  heading: string;
  children?: ReactNode;
}) {
  const id = useId();
  if (useCaptionInFrame()) return children;
  return (
    <section aria-labelledby={id}>
      <h3 id={id}>{heading}</h3>
      {children}
    </section>
  );
}
