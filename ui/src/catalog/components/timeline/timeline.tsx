import type { ComponentFn } from "@json-render/react";

import type { AtlasCatalog } from "../../catalog.ts";
import { PlainSection } from "../../plain-section.tsx";

export const Timeline: ComponentFn<AtlasCatalog, "timeline"> = ({ props }) => (
  <PlainSection heading={props.caption}>
    <ol>
      {props.events.map((event, index) => (
        <li key={index}>
          {event.date}: {event.label}
          {event.note !== undefined && <>. {event.note}</>}
        </li>
      ))}
    </ol>
  </PlainSection>
);
