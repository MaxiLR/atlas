# ADR-0010: shadcn/ui on Radix, restyled to Linear's design language

- Status: Accepted
- Date: 2026-11-18

## Context

Atlas should look and behave like Linear, so the component foundation was to
follow what Linear uses. Linear publishes no component library:

- Its components are internal. Radix's
  [case study](https://www.radix-ui.com/primitives/case-studies/linear) names
  the system Orbiter; Linear's
  [August 2026 post](https://linear.app/now/styling-linear-for-the-future-stylex)
  says it has shared components but no formal design system.
- Behaviour comes from Radix Primitives, adopted piece by piece, with Linear's
  own styling on top.
- Styling moved from styled-components to StyleX in August 2026, with CSS
  Modules as the escape hatch, for render-time cost and to stop a large team
  restyling components from outside.

Alternatives were Radix Primitives with StyleX and every component built by hand
(Linear's actual stack), shadcn/ui on Base UI (shadcn's default since July
2026), and Arc, whose rules forbid restyling its components.

## Decision

Atlas uses shadcn/ui in its Radix flavour (`shadcn init -b radix`) with Tailwind
4: Radix Primitives underneath, components as source files in this repository,
restyled to Linear's design language with the tokens of ADR-0011.

There is one component set. The app's chrome and the catalog of generated
visuals (ADR-0003) both use it:

- The json-render registry maps every catalog name to a component in this
  repository, not to the implementations shipped in `@json-render/shadcn`. That
  package (built on `radix-ui` and Tailwind 4) supplies catalog definitions
  where they fit and is a starting point for the source, nothing more.
- Learning visuals that shadcn/ui has no equivalent for are built here on the
  same primitives and tokens.
- A generated figure may use only the theme's tokens, so it follows the theme
  like every other component.

## Tradeoff

The behaviour layer is the one Linear uses; the styling engine is not. The look
is matched by tokens and component styling, not by sharing StyleX.

Restyled components are ours to maintain: upstream shadcn/ui changes do not
arrive by upgrading a package.
