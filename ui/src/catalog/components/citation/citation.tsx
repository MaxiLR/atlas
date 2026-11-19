import type { ComponentFn } from "@json-render/react";
import type { ReactNode } from "react";

import type { AtlasCatalog } from "../../catalog.ts";

/** The source's number and title, inline: it sits inside a sentence. */
export const Citation: ComponentFn<AtlasCatalog, "citation"> = ({ props }) => (
  <span>
    [{props.number}] <cite>{props.title}</cite>
  </span>
);

export function CitationScope({ children }: { children: ReactNode }) {
  return children;
}
