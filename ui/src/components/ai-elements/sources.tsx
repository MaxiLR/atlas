// AI Elements: Sources (https://ui.shadcn.com/docs/ai-elements/radix/sources).
// The sources a section or a reply used, numbered as their citations are.

export interface Source {
  url: string;
  title?: string;
  /** Who published it; the link's host name when unknown. */
  site?: string;
}

export function Sources({ sources }: { sources: Source[] }) {
  return (
    <ol className="grid gap-2 text-13 text-muted-foreground">
      {sources.map((source, index) => {
        const site = source.site ?? new URL(source.url).hostname;
        return (
          <li
            key={source.url}
            className="grid grid-cols-[1.5rem_minmax(0,1fr)] tabular-nums"
          >
            <span aria-hidden="true">{index + 1}</span>
            <span>
              <a
                href={source.url}
                target="_blank"
                rel="noreferrer"
                className="rounded-sm text-body underline decoration-edge-strong underline-offset-3 transition-colors outline-none hover:text-heading hover:decoration-current focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus focus-visible:outline-solid"
              >
                {source.title ?? site}
                <span className="sr-only"> (opens in a new tab)</span>
              </a>
              {source.title !== undefined && <>, {site}</>}
            </span>
          </li>
        );
      })}
    </ol>
  );
}
