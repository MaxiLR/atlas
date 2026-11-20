// AI Elements: Sources (https://ui.shadcn.com/docs/ai-elements/radix/sources).
// The sources a section or a reply used, numbered as their citations are.
import type { ComponentProps } from "react";

export interface Source {
  url: string;
  title?: string;
  /** Who published it; the link's host name when unknown. */
  site?: string;
  /** Its number, which its citations show; its place in the list when unknown. */
  number?: number;
  /** The entry's id, so a citation can point at it. */
  id?: string;
}

/** Who published a page, as its address names it. */
export function siteOf(url: string): string {
  return new URL(url).hostname.replace(/^www\./, "");
}

/**
 * Activating a source asks `onOpen` to open it in the system browser; the
 * window itself never follows the link.
 */
export function Sources({
  sources,
  onOpen,
  ...props
}: Omit<ComponentProps<"ol">, "children"> & {
  sources: Source[];
  onOpen?: (url: string) => void;
}) {
  return (
    <ol className="grid gap-2 text-13 text-muted-foreground" {...props}>
      {sources.map((source, index) => {
        const site = source.site ?? siteOf(source.url);
        const number = source.number ?? index + 1;
        return (
          <li
            key={number}
            id={source.id}
            className="grid grid-cols-[1.5rem_minmax(0,1fr)] tabular-nums"
          >
            <span aria-hidden="true">{number}</span>
            <span>
              <a
                href={source.url}
                onClick={(event) => {
                  event.preventDefault();
                  onOpen?.(source.url);
                }}
                onAuxClick={(event) => event.preventDefault()}
                className="rounded-sm text-body underline decoration-edge-strong underline-offset-3 transition-colors outline-none hover:text-heading hover:decoration-current focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid"
              >
                {source.title ?? site}
                <span className="sr-only"> (opens in your browser)</span>
              </a>
              {source.title !== undefined && <>, {site}</>}
            </span>
          </li>
        );
      })}
    </ol>
  );
}
