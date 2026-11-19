import type { ComponentFn } from "@json-render/react";
import { useId } from "react";

import type { AtlasCatalog } from "../../catalog.ts";

/**
 * One point set apart from the prose: a hairline box, never framed, with its
 * title at 510 in the heading colour over the body text. No icon and no
 * filled tile.
 */
export const Callout: ComponentFn<AtlasCatalog, "callout"> = ({ props }) => {
  const titleId = useId();
  return (
    <div
      data-slot="callout"
      role="note"
      aria-labelledby={titleId}
      className="rounded-xl border border-edge px-4 py-3.5 text-15 leading-[1.6] tracking-[-0.011em] wrap-break-word shadow-none break-inside-avoid"
    >
      <h3 id={titleId} className="font-medium text-heading">
        {props.title}
      </h3>
      <p className="mt-1 text-body">{props.body}</p>
    </div>
  );
};
