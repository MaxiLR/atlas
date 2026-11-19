import { defineRegistry } from "@json-render/react";

import { catalog } from "./catalog.ts";
import { Callout } from "./components/callout/callout.tsx";
import { CheckQuestion } from "./components/check-question/check-question.tsx";
import { Citation } from "./components/citation/citation.tsx";
import { Comparison } from "./components/comparison/comparison.tsx";
import { ConceptMap } from "./components/concept-map/concept-map.tsx";
import { Figure } from "./components/figure/figure.tsx";
import { LayeredFigure } from "./components/layered-figure/layered-figure.tsx";
import { MermaidDiagram } from "./components/mermaid-diagram/mermaid-diagram.tsx";
import { StepThrough } from "./components/step-through/step-through.tsx";
import { Timeline } from "./components/timeline/timeline.tsx";

/** Every catalog entry's component, and nothing else. */
export const { registry } = defineRegistry(catalog, {
  components: {
    "concept-map": ConceptMap,
    "layered-figure": LayeredFigure,
    timeline: Timeline,
    "step-through": StepThrough,
    comparison: Comparison,
    callout: Callout,
    "mermaid-diagram": MermaidDiagram,
    figure: Figure,
    "check-question": CheckQuestion,
    citation: Citation,
  },
});
