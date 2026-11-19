import type { ComponentFn } from "@json-render/react";
import { useId } from "react";

import { cn } from "@/lib/utils";

import type { AtlasCatalog } from "../../catalog.ts";
import { useCaptionInFrame } from "../../frame-context.ts";

/** The label column's width, as in the draft's Table 1. */
const LABEL_WIDTH = 112;
/**
 * The narrowest a compared column gets. Past the reading column's width the
 * table keeps it and scrolls sideways inside the frame instead of squeezing.
 */
const COLUMN_MIN_WIDTH = 128;

/** Every cell: a hairline above and to the left, text that wraps. */
const cell =
  "border-t border-l border-edge px-3.5 py-2.5 text-left align-top font-normal wrap-break-word";

/**
 * A table on the figure face: a header row of the things compared, a label
 * column of the aspects, hairline rules between cells and no filled tiles.
 * Inside a frame the table is the content; on its own its caption heads it.
 */
export const Comparison: ComponentFn<AtlasCatalog, "comparison"> = ({
  props,
}) => {
  const headingId = useId();
  const framed = useCaptionInFrame();

  const table = (
    <div
      data-slot="comparison"
      tabIndex={0}
      className="max-w-full overflow-x-auto outline-none focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid"
    >
      <table
        aria-label={framed ? props.caption : undefined}
        aria-labelledby={framed ? undefined : headingId}
        className="w-full table-fixed border-collapse text-13 text-body"
        style={{
          minWidth: LABEL_WIDTH + props.columns.length * COLUMN_MIN_WIDTH,
        }}
      >
        <colgroup>
          <col style={{ width: LABEL_WIDTH }} />
          {props.columns.map((_, index) => (
            <col key={index} />
          ))}
        </colgroup>
        <thead>
          <tr>
            <td
              aria-hidden="true"
              className={cn(cell, "border-t-0 border-l-0")}
            />
            {props.columns.map((column, index) => (
              <th
                key={index}
                scope="col"
                className={cn(cell, "border-t-0 font-medium text-heading")}
              >
                {column}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {props.rows.map((row, index) => (
            <tr key={index} className="break-inside-avoid">
              <th
                scope="row"
                className={cn(cell, "border-l-0 text-muted-foreground")}
              >
                {row.label}
              </th>
              {row.cells.map((value, cellIndex) => (
                <td key={cellIndex} className={cell}>
                  {value}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );

  if (framed) return table;
  return (
    <section aria-labelledby={headingId}>
      <h3 id={headingId} className="mb-2 text-15 font-medium text-heading">
        {props.caption}
      </h3>
      {table}
    </section>
  );
};
