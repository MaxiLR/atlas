import type { ComponentFn } from "@json-render/react";
import { useId, useLayoutEffect, useMemo, useRef } from "react";

import type { AtlasCatalog } from "../../catalog.ts";
import { useCaptionInFrame } from "../../frame-context.ts";
import { PlainSection } from "../../plain-section.tsx";
import { inertSvg } from "./inert-svg.ts";

/**
 * The figure's drawing, inline so `currentColor` and the theme's variables
 * apply: thin strokes in the text colour unless the figure paints them,
 * faces from the surface tokens, labels in the uppercase mono label style,
 * scaled to the column.
 */
function Drawing({ svg, caption }: { svg: string; caption: string }) {
  const prefix = useId().replace(/[^\w-]/g, "");
  const drawing = useMemo(
    () => inertSvg(svg, caption, `figure-${prefix}`),
    [svg, caption, prefix],
  );
  const host = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const element = host.current;
    if (element === null || drawing === null) return;
    element.replaceChildren(drawing);
    return () => element.replaceChildren();
  }, [drawing]);

  if (drawing === null) {
    return (
      <p className="px-4 py-5 text-13 text-muted-foreground">
        This figure could not be drawn; its description is below.
      </p>
    );
  }
  return (
    <div
      ref={host}
      className="[&_.face-a]:fill-raised [&_.face-b]:fill-canvas [&_.face-c]:fill-panel [&_text]:uppercase [&>svg]:block [&>svg]:h-auto [&>svg]:w-full [&>svg:not([fill])]:fill-none [&>svg:not([stroke])]:stroke-current"
    />
  );
}

/**
 * A figure the model drew, as the core's sanitiser passed it. Inside the
 * figure frame it is drawn; on its own it is named by its caption only, and
 * the SVG is never inserted.
 */
export const Figure: ComponentFn<AtlasCatalog, "figure"> = ({ props }) => {
  const framed = useCaptionInFrame();
  if (!framed) return <PlainSection heading={props.caption} />;
  return <Drawing svg={props.svg} caption={props.caption} />;
};
