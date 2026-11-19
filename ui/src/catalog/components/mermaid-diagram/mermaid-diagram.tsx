import type { ComponentFn } from "@json-render/react";
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";

import { Skeleton } from "@/components/ui/skeleton";

import type { AtlasCatalog } from "../../catalog.ts";
import { useCaptionInFrame } from "../../frame-context.ts";
import { PlainSection } from "../../plain-section.tsx";
import { paletteAt, renderDiagram, type Diagram } from "./render.ts";

/** Calls `onChange` whenever a theme scope on the page changes theme. */
function subscribeToTheme(onChange: () => void) {
  const observer = new MutationObserver(onChange);
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme"],
    subtree: true,
  });
  return () => observer.disconnect();
}

const pageTheme = () => document.documentElement.dataset.theme ?? "";

type Drawn =
  | { state: "drawing" }
  | { state: "drawn"; diagram: Diagram }
  | { state: "failed" };

/**
 * The diagram drawn by Mermaid in the theme where it sits, drawn again when
 * the theme changes; the previous drawing stays until the new one is ready.
 */
function Drawing({ source, caption }: { source: string; caption: string }) {
  const theme = useSyncExternalStore(subscribeToTheme, pageTheme);
  const host = useRef<HTMLDivElement>(null);
  const [drawn, setDrawn] = useState<Drawn>({ state: "drawing" });

  useEffect(() => {
    const element = host.current;
    if (element === null) return;
    let current = true;
    const dark =
      element.closest<HTMLElement>("[data-theme]")?.dataset.theme === "dark";
    renderDiagram(source, caption, paletteAt(element), dark).then((diagram) => {
      if (current) {
        setDrawn(diagram ? { state: "drawn", diagram } : { state: "failed" });
      }
    });
    return () => {
      current = false;
    };
  }, [source, caption, theme]);

  useLayoutEffect(() => {
    const element = host.current;
    if (element === null || drawn.state !== "drawn") return;
    element.replaceChildren(drawn.diagram.svg);
    return () => element.replaceChildren();
  }, [drawn]);

  return (
    <div className="p-4">
      {drawn.state === "drawing" && (
        <>
          <Skeleton className="h-40 w-full rounded-lg" />
          <p className="sr-only">Drawing the diagram…</p>
        </>
      )}
      {drawn.state === "failed" && (
        <p className="py-1 text-13 text-muted-foreground">
          This diagram could not be drawn; its description is below.
        </p>
      )}
      {drawn.state === "drawn" && <style>{drawn.diagram.css}</style>}
      <div
        ref={host}
        className="[&_text]:uppercase [&>svg]:mx-auto [&>svg]:block [&>svg]:h-auto [&>svg]:w-full"
      />
    </div>
  );
}

/**
 * A Mermaid diagram the model wrote, drawn at Mermaid's strict level with
 * no links, handlers or HTML. Inside the figure frame it is drawn; on its
 * own it shows its caption as its heading over its source as text.
 */
export const MermaidDiagram: ComponentFn<AtlasCatalog, "mermaid-diagram"> = ({
  props,
}) => {
  const framed = useCaptionInFrame();
  if (framed) return <Drawing source={props.source} caption={props.caption} />;
  return (
    <PlainSection heading={props.caption}>
      <pre>
        <code>{props.source}</code>
      </pre>
    </PlainSection>
  );
};
