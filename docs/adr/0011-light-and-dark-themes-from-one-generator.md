# ADR-0011: Light and dark themes from one generator

- Status: Accepted
- Date: 2026-11-18

## Context

Atlas supports a light and a dark theme. The design reference
(`docs/design/README.md`) describes Linear's dark marketing site only, so a
light theme cannot be copied from it.

Linear does not hand-write its themes. Per its
[August 2026 post](https://linear.app/now/styling-linear-for-the-future-stylex),
a theme starts from a base colour, an accent colour and a contrast value, from
which more than a hundred colour variables are derived in LCH space. A selected
row, a menu or a raised surface regenerates the theme with its own background as
the base, so labels, borders and controls re-derive against it.

The alternative was two hand-written token sets: dark copied from the reference,
light designed by hand.

## Decision

One generator takes a base colour, an accent colour and a contrast value and
derives every colour token in OKLCH. The light and the dark theme are two sets
of inputs to it.

Menus, popovers and raised surfaces derive their tokens from their own
background, scoped to their subtree.

The dark theme's inputs are tuned until the generated tokens match the
reference's values: canvas `#08090a`, panel `#0f1011`, headings `#f7f8f8`, prose
`#8a8f98`.

Components, catalog visuals and generated figures use tokens only, never literal
colours.

## Tradeoff

The generator has to exist and be tested before any screen looks right. The
generated dark theme approximates the reference's colours; it does not copy them
one by one. In return the two themes cannot drift apart, and further themes are
only new inputs.
