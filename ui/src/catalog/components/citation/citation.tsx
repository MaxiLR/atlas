import type { ComponentFn } from "@json-render/react";
import {
  createContext,
  useContext,
  useId,
  useMemo,
  type ReactNode,
} from "react";

import { failureInStore, useAtlasStore } from "@/app/store";
import { InlineCitation } from "@/components/ai-elements/inline-citation";
import { siteOf } from "@/components/ai-elements/sources";

import type { AtlasCatalog } from "../../catalog.ts";

/** What the citations of one text and its list of sources share. */
export interface CitationScopeValue {
  /** The id of a source's entry in the list. */
  entryId: (number: number) => string;
  /** Opens a source's address in the system browser. */
  open: (url: string) => void;
}

const ScopeContext = createContext<CitationScopeValue | null>(null);

/**
 * Wraps a text and its list of sources, a section or a side-chat reply: its
 * chips point at their entries in the list, and both open a source through
 * the core in the system browser.
 */
export function CitationScope({ children }: { children: ReactNode }) {
  const { send } = useAtlasStore();
  const prefix = useId();
  const scope = useMemo<CitationScopeValue>(
    () => ({
      entryId: (number) => `${prefix}source-${number}`,
      open: (url) => {
        send({ type: "OpenExternal", url }).catch(failureInStore);
      },
    }),
    [prefix, send],
  );
  return <ScopeContext value={scope}>{children}</ScopeContext>;
}

/** The scope around a list of sources, which it needs to open them. */
export function useCitationScope(): CitationScopeValue {
  const scope = useContext(ScopeContext);
  if (scope === null) throw new Error("Sources needs a CitationScope");
  return scope;
}

/**
 * The source's number as a chip in the sentence, which opens a card with
 * its title and site on hover or focus. Inside a `CitationScope` the chip
 * points at its entry in the list and opens the source in the system
 * browser. The title stays in the sentence's markup as a hidden `<cite>`,
 * so the text of a section names what it cites.
 */
export const Citation: ComponentFn<AtlasCatalog, "citation"> = ({ props }) => {
  const scope = useContext(ScopeContext);
  return (
    <>
      <InlineCitation
        number={props.number}
        url={props.url}
        title={props.title}
        site={siteOf(props.url)}
        describedBy={scope?.entryId(props.number)}
        onOpen={scope === null ? undefined : () => scope.open(props.url)}
      />
      <cite hidden>{props.title}</cite>
    </>
  );
};
