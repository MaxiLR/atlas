---
version: 1
slug: "ui-src-reader"
primary_target: "ui/src/reader"
related_targets: []
---

# Reader

Scope: the course reader in `ui/`, the surface that carries Atlas's main task.
Mode: Read, inside an application shell.

Audience and job: one candidate at a Mac, in a focused sitting, reading a
generated section of a course and deciding where to go next. Content: the
index, one section (prose, catalog visuals, figures, maths, citations, a check
question), the usage meter. Constraints: the index stays visible beside the
reading column; the meter sits at the foot of the index; doubts open beside the
text and never change it; minimum window width 720 px.

Memorable moment: the whole topic drawn as one line, with the candidate's
position on it.

## Direction contract

THESIS: The index is one drawn figure of the whole topic: the broad pass is a
continuous line, each section a node, each deep dive a branch. It refuses the
category default of an indented file tree beside a document.

OWN-WORLD: Linear's language. Near-black canvas and a slightly lighter panel,
cool-white Inter Variable at weight 510 for heads, hairline translucent edges,
12 px panels, compact 28 px controls with 6 px corners, one indigo accent kept
for where the candidate is and what they marked. Figures are technical
drawings: thin strokes, dark faces, uppercase mono captions.

STORY: The candidate sees the whole territory and how far along it they are,
reads the open section in a calm column, continues along the line with Next or
branches with Go deeper, and settles a doubt beside the text without leaving it.

FIRST VIEWPORT: A 320 px rail: window controls and Library, course title and
read count, the route (chapter stations, section nodes, the travelled part
bright, the current node in the accent, deep dives branching under their
section), the usage meter at the foot. A 44 px header: breadcrumb left; version,
Glossary, Side chats right. A centred 680 px reading column: title at 28 px, a
one-line summary, 16 px prose, a figure. A 64 px foot: the next node drawn on
the line, Go deeper, and Next as the primary action at the column's right edge.

FORM: Route line, fifth of seven on the ordered list; seed:146cda263b3b1d6c.
Signature interaction: Go deeper draws the branch from the current node as the
proposals arrive; Next advances the bright part of the line. Motion is 150 to
250 ms ease-out and renders already drawn under reduced motion.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

## Unresolved

None.
