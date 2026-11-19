# Design reference

Atlas aims for a Linear-like look. The reference the engineer supplied is
`reference/linear.app-DESIGN.md`, Refero's description of style
`3affd07f-e21e-46a4-8aee-26accba689a6`. It is a third-party export and is kept
out of Git; each checkout needs its own copy.

It is input to the design step, not Atlas's own `DESIGN.md`. That file is
written from the built interface.

## What the reference covers

It describes linear.app's **marketing site**, dark theme only: landing, pricing,
company and campaign pages. It does not describe Linear's application: there is
no sidebar, list, input, menu, popover or dialog in it, no long-form reading
typography, and no light theme.

## What carries over to Atlas

- **Colour.** The neutral ramp: canvas `#08090a`, panel `#0f1011`, headings
  `#f7f8f8`, large copy `#d0d6e0`, prose `#8a8f98`, tertiary `#62666d`. Edges are
  translucent white at 8 % and 12 %, half a pixel to one pixel wide. Saturated
  colour is rare and local.
- **Type.** Inter Variable with `cv01` and `ss03`; headings at weight 510, never
  bold; prose at 15px/1.6 with -0.011em tracking; small text at 13px/1.5. A mono
  face (Berkeley Mono, else IBM Plex Mono) at 12px, uppercase, only for figure
  captions and technical markings.
- **Surfaces.** 12px corners on panels; one soft shadow
  (`rgba(0, 0, 0, 0.25) 0px 2px 32px`) under floating panels; separation by
  space and hairlines instead of filled tiles.
- **Figures.** The technical-drawing treatment (thin grey strokes, dark faces,
  mono captions) is the model for catalog visuals and generated figures.
- **Motion.** State changes by brightness and small fill changes; a press scales
  to 0.97.
- **Writing.** Direct, declarative, no exclamation marks, no hype words.

## What does not carry over

Display sizes of 48px to 72px, the product-stage opening, proof cards and their
acid-yellow and pastel fills, pricing and comparison tables, the footer, and the
page flows. Pill-shaped 44px actions are the marketing site's buttons; whether
Atlas's controls take that shape is a decision for the design step.

## Gaps the design step must fill

Application chrome (sidebar, index tree, toolbar, side panel), form controls,
menus and popovers, the reading column for lessons, and anything below desktop
width.

The light theme is not designed by hand: both themes come from one generator
(ADR-0011), and the dark one is tuned to the values above. Components are
shadcn/ui on Radix, restyled (ADR-0010).
