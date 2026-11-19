# ADR-0003: Generated visuals come from a catalog, with json-render

- Status: Accepted
- Date: 2026-11-18

## Context

Lessons need varied, good-looking visuals that still look like one app. Letting
the model write arbitrary interface code gives variety without coherence.
Catalog-based generative-interface libraries solve this: we register components
with typed props, the library turns the catalog into prompt instructions, the
model emits a spec, and a streaming renderer mounts only our components.

Three are widely used: [json-render](https://github.com/vercel-labs/json-render)
(Vercel Labs, JSON specs, 36 shadcn/ui components),
[OpenUI](https://github.com/thesysdev/openui) (a compact line-based language,
built-in charts) and [A2UI](https://github.com/google/A2UI) (a protocol for
remote agents across platforms).

## Decision

Atlas uses json-render with its own catalog of learning components (concept
maps, layered figures, timelines, step-throughs, comparisons, questions, a
Mermaid diagram component), built on shadcn/ui and restyled toward Linear.

The registry that maps catalog names to React components is ours, so any
component can be restyled or replaced without forking json-render.

The catalog has one free-form entry, the figure: a static SVG the model writes
using only Atlas's colour and type tokens. It is sanitised and carries no
scripts. The model never writes HTML or JavaScript.

## Tradeoff

A JSON spec is something the Rust core can validate, repair and store with
serde; OpenUI's language needs its TypeScript runtime to parse. Against that,
json-render costs about twice the output tokens per visual, and OpenUI's own
benchmark reports lower first-pass validity for it (82.8 % against 96.9 %;
vendor-run, not independently checked).

Figures are static: there are no generated interactive simulations.

## To verify

json-render's mode that mixes prose with interface specs fits long-form
sections.
