import type { ComponentFn } from "@json-render/react";

import type { AtlasCatalog } from "../../catalog.ts";
import { PlainSection } from "../../plain-section.tsx";

export const Comparison: ComponentFn<AtlasCatalog, "comparison"> = ({
  props,
}) => (
  <PlainSection heading={props.caption}>
    <table>
      <thead>
        <tr>
          <td />
          {props.columns.map((column, index) => (
            <th key={index} scope="col">
              {column}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {props.rows.map((row, index) => (
          <tr key={index}>
            <th scope="row">{row.label}</th>
            {row.cells.map((cell, cellIndex) => (
              <td key={cellIndex}>{cell}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  </PlainSection>
);
