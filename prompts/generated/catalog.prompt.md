---
version: 2
---

A catalog visual is one JSON object: a json-render spec holding exactly one element.

```json
{ "root": "<key>", "elements": { "<key>": { "type": "<entry>", "props": {} } } }
```

- `root` is the key of the one element in `elements`.
- `type` is one of the entries below; `props` holds that entry's props and no others. A prop is required unless marked optional.
- Values are plain JSON. Do not use expressions such as `$state` or `$cond`, and do not add child elements or the fields `visible`, `on`, `repeat`, `slots` or `watch`.

## Entries

### `concept-map`

Concepts as nodes joined by labelled links, for how a handful of ideas relate.

- `caption` (string): One sentence saying what the visual shows.
- `nodes` (list of 2 or more objects)
  - `id` (string): Unique within the map.
  - `label` (string): The concept, a few words.
- `links` (list of 1 or more objects)
  - `from` (string): The id of a node.
  - `to` (string): The id of a node.
  - `label` (string): How the two relate, a few words.
  - `directed` (boolean, optional): True when the relation reads one way, from → to.

### `layered-figure`

Stacked layers, each with a label and a note, for structures built in layers.

- `caption` (string): One sentence saying what the visual shows.
- `layers` (list of 2 or more objects): Ordered from the top or outermost layer down.
  - `label` (string): A short name, a few words.
  - `note` (string): One sentence.

### `timeline`

Dated events in order, for how something developed.

- `caption` (string): One sentence saying what the visual shows.
- `events` (list of 2 or more objects): In date order, earliest first.
  - `date` (string): A date, year or period, as written.
  - `label` (string): What happened, a few words.
  - `note` (string, optional): One sentence.

### `step-through`

Ordered steps, each with a label and a note, for a process or a derivation.

- `caption` (string): One sentence saying what the visual shows.
- `steps` (list of 2 or more objects): In order, first step first.
  - `label` (string): A short name, a few words.
  - `note` (string): One sentence.

### `comparison`

A table comparing a few things (columns) on a few aspects (rows).

- `caption` (string): One sentence saying what the visual shows.
- `columns` (list of 2 or more strings): The things compared, one heading per column.
- `rows` (list of 1 or more objects)
  - `label` (string): The aspect compared in this row.
  - `cells` (list of any number of strings): One cell per column, in column order.

### `callout`

One point set apart from the prose: a warning, a key idea.

- `title` (string): The point, in one short sentence.
- `body` (string): One to three sentences.

### `mermaid-diagram`

A Mermaid flowchart, sequence or state diagram, for structure a catalog entry does not fit.

- `source` (string): Mermaid source, without init directives, click lines, links or HTML.
- `caption` (string): One sentence saying what the visual shows.

### `figure`

A free-form static SVG drawing, for what only a drawing shows.

- `svg` (string): One static <svg> element: no script, no foreignObject, no event attributes, no external references; colours only currentColor or the theme's variables.
- `caption` (string): One sentence saying what the visual shows.

### `check-question`

A multiple-choice question with four options, one right, testing one rung.

- `rung` (integer, at least 0): The id of the ladder rung it tests.
- `stem` (string): The question.
- `options` (list of exactly 4 objects)
  - `text` (string)
  - `misconception` (string or null): Null for the right option (the key). Every wrong option must name, in one sentence, the specific misconception that picking it reveals.
- `key` (integer from 0 to 3): The right option, counted from 0; its misconception is null and every other option's is a string.
- `explanation` (string): Why the right option is right, shown once answered.

### `citation`

A reference to a numbered source of the section.

- `number` (integer, at least 1): The source's number in the section.
- `title` (string): The source's title.
- `url` (string): The source's address, http or https.
