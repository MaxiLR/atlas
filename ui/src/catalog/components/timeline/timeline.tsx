import type { ComponentFn } from "@json-render/react";
import { useId } from "react";

import { cn } from "@/lib/utils";

import type { AtlasCatalog } from "../../catalog.ts";
import { useCaptionInFrame } from "../../frame-context.ts";

/**
 * Dated events down a drawn line: the date as an uppercase mono label beside
 * the line, a hollow node on it for each event, and the event's label and
 * note after the node. However many events there are, they stack downward.
 * Inside a frame the list is the content; on its own its caption heads it.
 */
export const Timeline: ComponentFn<AtlasCatalog, "timeline"> = ({ props }) => {
  const headingId = useId();
  const framed = useCaptionInFrame();
  const last = props.events.length - 1;

  const list = (
    <ol
      data-slot="timeline"
      aria-label={framed ? props.caption : undefined}
      aria-labelledby={framed ? undefined : headingId}
      className="px-4 py-5 wrap-break-word"
    >
      {props.events.map((event, index) => (
        <li
          key={index}
          className="grid grid-cols-[5.5rem_minmax(0,1fr)] gap-x-4 break-inside-avoid"
        >
          <p className="pt-1 text-right font-mono text-12 tracking-[0.04em] text-muted-foreground uppercase">
            {event.date}
          </p>
          <div className={cn("relative pl-6", index < last && "pb-5")}>
            {index > 0 && (
              <span
                aria-hidden="true"
                className="absolute top-0 left-1 h-[7px] w-px bg-faint"
              />
            )}
            <span
              aria-hidden="true"
              className="absolute top-[7px] left-0 size-[9px] rounded-full border border-muted-foreground"
            />
            {index < last && (
              <span
                aria-hidden="true"
                className="absolute top-4 bottom-0 left-1 w-px bg-faint"
              />
            )}
            <p className="text-15 text-heading">{event.label}</p>
            {event.note !== undefined && (
              <p className="mt-0.5 text-13 text-muted-foreground">
                {event.note}
              </p>
            )}
          </div>
        </li>
      ))}
    </ol>
  );

  if (framed) return list;
  return (
    <section aria-labelledby={headingId}>
      <h3 id={headingId} className="text-15 font-medium text-heading">
        {props.caption}
      </h3>
      {list}
    </section>
  );
};
