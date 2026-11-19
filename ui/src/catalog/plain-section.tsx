import { useId, type ReactNode } from "react";

/**
 * A visual in its plainest accessible form: its content as text under a
 * heading. The registry's first components all use it.
 */
export function PlainSection({
  heading,
  children,
}: {
  heading: string;
  children?: ReactNode;
}) {
  const id = useId();
  return (
    <section aria-labelledby={id}>
      <h3 id={id}>{heading}</h3>
      {children}
    </section>
  );
}
