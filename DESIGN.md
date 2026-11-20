---
name: Atlas
description: A desktop learning harness that draws the whole topic as one line and reads it broad first, then deep.
colors:
  canvas: "#08090a"
  panel: "#101112"
  raised: "#171819"
  heading: "#f7f8f9"
  body: "#d6d8e0"
  muted: "#8c8e97"
  faint: "#64656e"
  edge: "#f7f8f914"
  edge-strong: "#f7f8f924"
  wash: "#f7f8f90d"
  wash-strong: "#f7f8f91a"
  accent: "#7a83e8"
  accent-wash: "#7a83e824"
  primary-fill: "#e5e6eb"
  primary-text: "#08090a"
  green: "#009343"
  amber: "#ad7400"
  red: "#d45652"
  light-canvas: "#fdfdfd"
  light-panel: "#f7f7f7"
  light-raised: "#ffffff"
  light-heading: "#0f1013"
  light-body: "#292a30"
  light-muted: "#5f6169"
  light-faint: "#868992"
  light-edge: "#0f101314"
  light-edge-strong: "#0f101329"
  light-wash: "#0f10130a"
  light-wash-strong: "#0f10130f"
  light-accent: "#5e6ad2"
  light-accent-wash: "#5e6ad21a"
  light-primary-fill: "#1f2024"
  light-primary-text: "#ffffff"
  light-green: "#008039"
  light-amber: "#966400"
  light-red: "#c04441"
typography:
  title:
    fontFamily: "Inter Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "28px"
    fontWeight: 510
    lineHeight: "34px"
    letterSpacing: "-0.022em"
    fontFeature: '"cv01", "ss03"'
  headline:
    fontFamily: "Inter Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "20px"
    fontWeight: 510
    lineHeight: 1.4
    letterSpacing: "-0.012em"
    fontFeature: '"cv01", "ss03"'
  body:
    fontFamily: "Inter Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "16px"
    fontWeight: 400
    lineHeight: 1.65
    letterSpacing: "-0.011em"
    fontFeature: '"cv01", "ss03"'
  input:
    fontFamily: "Inter Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "-0.011em"
    fontFeature: '"cv01", "ss03"'
  interface:
    fontFamily: "Inter Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.5
    fontFeature: '"cv01", "ss03"'
  small:
    fontFamily: "Inter Variable, ui-sans-serif, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.4
    fontFeature: '"cv01", "ss03"'
  label:
    fontFamily: "IBM Plex Mono, ui-monospace, Menlo, monospace"
    fontSize: "12px"
    fontWeight: 400
    lineHeight: 1.4
    letterSpacing: "0.04em"
rounded:
  control: "6px"
  option: "8px"
  field: "10px"
  panel: "12px"
spacing:
  route-row: "30px"
  header: "44px"
  foot: "64px"
  rail-folded: "72px"
  rail: "320px"
  side-panel: "400px"
  side-overlay: "420px"
  task-column: "640px"
  reading-column: "680px"
  list-column: "760px"
components:
  button-primary:
    backgroundColor: "{colors.primary-fill}"
    textColor: "{colors.primary-text}"
    typography: "{typography.interface}"
    rounded: "{rounded.control}"
    height: "28px"
    padding: "0 10px"
  button-primary-hover:
    backgroundColor: "{colors.heading}"
    textColor: "{colors.primary-text}"
  button-primary-form:
    backgroundColor: "{colors.primary-fill}"
    textColor: "{colors.primary-text}"
    rounded: "{rounded.control}"
    height: "32px"
    padding: "0 12px"
  button-default:
    backgroundColor: "{colors.wash}"
    textColor: "{colors.body}"
    typography: "{typography.interface}"
    rounded: "{rounded.control}"
    height: "28px"
    padding: "0 10px"
  button-default-hover:
    backgroundColor: "{colors.wash-strong}"
    textColor: "{colors.heading}"
  button-ghost:
    textColor: "{colors.muted}"
    typography: "{typography.interface}"
    rounded: "{rounded.control}"
    height: "28px"
    padding: "0 10px"
  button-ghost-hover:
    backgroundColor: "{colors.wash}"
    textColor: "{colors.heading}"
  input:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.heading}"
    typography: "{typography.input}"
    rounded: "{rounded.field}"
    height: "36px"
    padding: "0 12px"
  option-row:
    textColor: "{colors.body}"
    typography: "{typography.input}"
    rounded: "{rounded.option}"
    padding: "12px 14px"
  option-row-selected:
    backgroundColor: "{colors.accent-wash}"
    textColor: "{colors.heading}"
  figure:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.body}"
    rounded: "{rounded.panel}"
  figure-caption:
    textColor: "{colors.muted}"
    typography: "{typography.interface}"
    padding: "10px 16px"
  route-row:
    textColor: "{colors.muted}"
    typography: "{typography.interface}"
    rounded: "{rounded.control}"
    height: "{spacing.route-row}"
  route-row-current:
    backgroundColor: "{colors.wash}"
    textColor: "{colors.heading}"
  popover:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.body}"
    typography: "{typography.interface}"
    rounded: "{rounded.panel}"
    padding: "16px"
---

# Design System: Atlas

## Overview

**Creative North Star: "The Route Line"**

Atlas draws the whole topic as one figure before the candidate reads a word of
it. The broad pass is a continuous vertical line down the rail; each chapter is
a station, each section a node, each deep dive a branch that leaves the line
under its section. The part already travelled is bright, the current node
carries the accent, and the reading column beside it is calm enough to stay in
for an hour. Everything else in the interface is chrome that recedes so the
line and the lesson can lead.

The world is Linear's design language, pinned by the engineer: a near-black
canvas with a slightly lighter panel, cool-white Inter Variable at weight 510
for heads, hairline translucent edges, compact 28 px controls, 12 px panels and
one indigo accent. It borrows the type, the palette, the density and the
technical-drawing figure treatment. It does not borrow the marketing site's
display sizes, pill buttons, proof cards or page flows. Light and dark are not
designed by hand: both come from one generator, and the dark theme is tuned to
the reference.

The density is an application's, not a page's: 13 px interface type, 30 px
index rows, a 44 px header and a 64 px foot frame a 680 px column of 16 px
prose. The interface is Read inside an application shell, so ambition shows as
precision and restraint.

**Key Characteristics:**

- One drawn figure of the whole topic, always visible, with the candidate's
  position marked on it.
- State is a mark, not a hue: hollow, filled, branched.
- A neutral ramp solved against WCAG contrast, one accent, state colours only
  where they carry meaning.
- Inter Variable at 400 and 510 only; IBM Plex Mono only for captions and
  figure labels.
- Flat surfaces separated by hairlines; the one soft shadow belongs to floating
  surfaces.

## Colors

A restrained, cool neutral ramp with a single indigo accent; the generator of
ADR-0011 computes every value in OKLCH from a base, an accent and a contrast
input, so the frontmatter's hex values are the rendered output, not the source.

### Primary

- **Route Indigo** (accent): where the candidate is (the current node, the
  selected option), what the candidate marked (a defined term's dotted
  underline, a side chat's margin mark and its quoted passage), and keyboard
  focus. Its translucent **Indigo Wash** (accent-wash) fills a selected option
  row and the text selection.

### Neutral

- **Night Canvas** (canvas): the reading ground and the screens outside a
  course.
- **Rail Panel** (panel): the rail, the side panel, figure faces, text fields.
- **Raised Slate** (raised): menus, popovers, dialogs. A `data-surface="raised"`
  subtree re-derives every token from this ground, so its labels and edges keep
  their contrast.
- **Cool White** (heading): titles, current items, chapter stations' labels.
- **Reading Grey** (body): lesson prose, chat replies, read section titles,
  the travelled part of the line and read nodes. Contrast 14:1 on the canvas.
- **Support Grey** (muted): secondary text, captions, unread section titles,
  ghost buttons. Contrast 6.1:1.
- **Mark Grey** (faint): the resting line, unread nodes' rings, rules and the
  boundary of an unchecked control. 3.45:1, a mark and never text.
- **Hairline** (edge, edge-strong): translucent heading ink at 8 % and 14 %
  (16 % in light) for borders; the stronger one bounds fields and popovers.
- **Ink Wash** (wash, wash-strong): translucent heading ink at 5 % and 10 %
  (4 % and 6 % in light) for hover, pressed and current fills.
- **Primary Fill** (primary-fill on primary-text): the one primary action on a
  screen, near-white on dark, near-black on light.

### State

- **Green, Amber, Red**: right and wrong answers, usage high and critical,
  destructive actions. Solved to 5:1 on the canvas. They appear only where they
  carry that meaning.

### Named Rules

**The Three Jobs Rule.** The accent marks position, what the candidate marked,
and focus. Nothing else: not hover, not links, not decoration, not a primary
button.

**The Faint Is Not Text Rule.** Mark Grey sits at 3.45:1, under the AA text
floor. It draws nodes, the resting line, rules and unread marks; any text that
looks faint uses Support Grey.

**The Generator Rule.** Colours come only from the theme tokens. Change the
generator's input, never a value downstream of it; both themes, every derived
surface and the WCAG floors follow by construction.

## Typography

**Body Font:** Inter Variable (with ui-sans-serif, system-ui), features `cv01`
and `ss03`
**Label/Mono Font:** IBM Plex Mono (with ui-monospace, Menlo)

**Character:** One neo-grotesque doing every job at two weights, so hierarchy
comes from size, colour and weight 510 rather than from bold; the mono is a
draughtsman's lettering, used where a technical drawing would carry it.

### Hierarchy

- **Title** (510, 28 px, 34 px): the section title at the head of the reading
  column. The largest type in the app.
- **Headline** (510, 20 px, 1.4): section headings inside a lesson, the
  question on a placement or check question.
- **Body** (400, 16 px, 1.65, -0.011em): lesson prose in Reading Grey, held to
  a 680 px measure. A chat reply sets the same Typeset styles smaller.
- **Input** (400, 15 px, 1.5): text fields, option rows, list titles.
- **Interface** (400 or 510, 13 px, 1.5): buttons, the route, the breadcrumb,
  captions, menus.
- **Small** (400, 12 px, 1.4): figures beside the usage meter, keyboard hints.
- **Label** (IBM Plex Mono 400, 12 px, 0.04em, uppercase): figure and table
  numbers in a caption bar and labels drawn inside a figure. Nothing else.

### Named Rules

**The Never Bold Rule.** Weights are 400 and 510. A heading is set apart by
size and the heading colour, never by 600 or 700.

**The Mono Caption Rule.** IBM Plex Mono appears only in figure captions and
inside figures. Code, keys and data use Inter.

## Layout

Inside a course the window is a three-region shell. A **rail of 320 px** holds
the window controls and Library, the course title and read count, the route,
and the usage meter at its foot. A **44 px header** carries the breadcrumb on
the left and the version switch, Glossary, Side chats and the course menu on the
right. A **reading column of 680 px** sits centred under it: title, a one-line
summary, the prose and its figures. A **64 px foot** spans the column: the next
node drawn on the line, Go deeper, and Next as the primary action at the
column's right edge. A **side panel of 400 px** opens at the right on demand for
side chats and the glossary; the reading column keeps its measure.

Screens outside a course (library, placement, sign-in, settings) have no rail:
a 44 px window bar and one centred column, 760 px for lists and 640 px for a
single task.

The minimum window is 720 px. Below 1024 px the rail folds to 72 px and shows
the line alone, nodes without titles, so the territory and the position stay
visible; a header button opens the titles as an overlay. The usage meter folds
to its bars, with figures on hover and focus. The side panel opens as an
overlay over the reading column, at most 420 px. Multi-column rows stack.

The route is laid on a 30 px row grid; every station, node and branch sits on
it, so the drawn line and the list of titles never drift apart.

## Elevation & Depth

Flat by default. Depth comes from tonal steps, solved by contrast against the
canvas (panel at 1.055:1, raised up to 1.12:1, capped at white in the light
theme), and from hairlines. Panels and
figures have a hairline edge and no shadow. Only floating surfaces (menus,
popovers, dialogs, sheets and the narrow overlays of the rail and side panel)
take the soft shadow, together with a stronger hairline ring.

### Shadow Vocabulary

- **Float** (dark:
  `0 2px 32px color-mix(in oklab, canvas 25%, transparent), 0 8px 24px color-mix(in oklab, canvas 35%, transparent)`;
  light: the same in heading ink at 8 % and 10 %): under floating surfaces
  only.

### Named Rules

**The Flat Ground Rule.** If it does not float above the page, it has no
shadow. A card, a figure or a panel is separated by its tone and a hairline.

## Shapes

Gently squared. Corners grow with the size of the thing: 6 px for compact
controls and index rows, 8 px for option rows and menus, 10 px for text fields,
12 px for panels, figures and popovers. Borders are hairlines, never thicker
than 1 px. There are no pills and no fully round controls; the only circles are
the route's nodes.

The route's marks are the system's one drawn geometry: a hollow 5 px station
ring for a chapter, a filled 3 px disc for a read section, a hollow ring of the
same outer edge for an unread one, a ringed accent dot for the current node,
and branches as curved 1 px connectors.

## Components

### Buttons

Compact and quiet; they change fill and brightness, never hue.

- **Shape:** gently squared (6 px), 28 px high, 13 px at weight 510.
- **Primary:** Primary Fill with Primary Text, 28 px; 32 px as the primary
  action of a form. One per screen. Hover lifts the fill to Cool White.
- **Default:** Ink Wash with a hairline and Reading Grey text; hover moves to
  the strong wash and Cool White text. A toggle that is on or a trigger whose
  menu is open keeps the hover fill.
- **Ghost:** no fill, Support Grey text; hover takes the wash and Cool White.
- **Hover / Focus / Press:** 150 ms ease-out on fill, edge and colour; focus is
  a 2 px Route Indigo outline at a 2 px offset; a press scales to 0.97.
  Transitions stop under reduced motion.

### Inputs / Fields

- **Style:** Rail Panel face, strong hairline, 10 px corners, 36 px high,
  15 px text in Cool White, Support Grey placeholder.
- **Focus:** the edge turns Route Indigo with a 3 px Indigo Wash ring.
- **Error / Disabled:** the edge turns Red; disabled loses its face and dims
  its text to Support Grey.

### Option rows

The placement questions, check questions and model picker share one row.

- **Style:** 8 px corners, a hairline, 12 px by 14 px padding, 15 px text,
  a radio mark and an optional key chip.
- **Selected:** the accent edge and the Indigo Wash, text to Cool White.
- **Hover / Focus:** the strong hairline and the wash; focus is the accent
  outline around the row.

### Figures

The signature the world lends, used in lessons and in the index alike.

- **Corner Style:** 12 px, hairline edge, no shadow.
- **Background:** the visual on the Rail Panel face.
- **Caption bar:** under a hairline, 10 px by 16 px; the figure number in
  uppercase mono, the caption in 13 px Support Grey.
- **Drawing:** thin strokes in the text colour, faces in the panel and canvas
  tokens, labels in the mono label style. Catalog visuals (comparison, callout,
  check question) separate with hairlines, not filled tiles.

### Menus and popovers

- **Style:** Raised Slate, a strong hairline ring, the Float shadow; 8 px
  corners for menus and 12 px for popovers.
- **Motion:** a 100 ms fade with a 0.98 zoom from the trigger's side; none
  under reduced motion.

### Navigation: the Route Line

The index is one drawn figure, not a tree.

- **Line:** a 1 px band in Mark Grey from the first station to the last node;
  the travelled part is drawn over it in Reading Grey.
- **Rows:** 30 px, 13 px titles: chapters in Cool White at 510, read sections
  in Reading Grey, unread in Support Grey. The current section's title sits on
  the wash with a hairline inset ring.
- **Marks:** state is a mark, not a hue, as described under Shapes; it holds
  in both themes and without colour vision.
- **Branches:** a section's deep dives leave the line on a curved connector in
  Support Grey.
- **Interaction:** hovering a row takes the wash and scales its node to 1.25.
  Go deeper draws the branch from the current node as the proposals arrive
  (250 ms); Next advances the bright part of the line (200 ms). Both render
  already drawn under reduced motion.
- **Narrow:** folded to 72 px, the line and marks remain and the titles hide.

### Reader foot

- **Style:** 64 px, a top hairline on the canvas, centred to the column.
- **Content:** the next node drawn on its own short line with its title and
  chapter, Go deeper (default button), Next (primary button).

## Do's and Don'ts

### Do:

- **Do** take every colour from the theme tokens and let the generator decide
  both themes.
- **Do** keep lesson prose in Reading Grey at 16 px/1.65 inside the 680 px
  measure.
- **Do** keep the whole route visible at every window width, folded to its
  marks below 1024 px.
- **Do** show state with the route's marks (hollow, filled, branched) so it
  reads without colour.
- **Do** run state changes at 150 ms ease-out, scale a press to 0.97, and stop
  all of it under reduced motion.
- **Do** name controls by their action: Start course, Check answer, Go deeper.

### Don't:

- **Don't** use the accent for hover, links, decoration or the primary button;
  it has three jobs.
- **Don't** set any text in Mark Grey.
- **Don't** use weight 600 or bold, or display sizes above 28 px.
- **Don't** draw pill-shaped buttons or 44 px marketing actions.
- **Don't** put a shadow on a panel, card or figure; only floating surfaces
  take the Float shadow.
- **Don't** separate catalog visuals with filled tiles; use hairlines.
- **Don't** use IBM Plex Mono outside captions and figure labels.
- **Don't** use green, amber or red where they do not mean right, wrong, high
  usage or danger.
- **Don't** use exclamation marks or hype words.
