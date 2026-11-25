# Scripted section streams

The streams `tests/section_generation.rs` answers write-section and repair
requests with, for the sample course's section "What drives the plates".

| File                   | Stream                                                                                                                       |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `complete.md`          | Prose, a catalog visual, prose, a figure, check questions and a valid record whose summary and concept line carry sentinels. |
| `redefines-symbol.md`  | Prose and a valid record that gives the plan's symbol `v` a new meaning and fixes a new term.                                |
| `invalid-record.md`    | Prose and a record without a summary.                                                                                        |
| `no-record.md`         | Prose only.                                                                                                                  |
| `invalid-visual.md`    | Prose, a callout missing its body, and a valid record.                                                                       |
| `repaired-record.json` | A valid record, the answer to a record repair.                                                                               |
| `repaired-visual.json` | A valid callout, the answer to a block repair.                                                                               |
| `plan-limit.sse`       | A recorded Responses stream: text, then `response.failed` with `usage_limit_reached`.                                        |
| `incomplete.sse`       | A recorded Responses stream: text, then `response.incomplete` with the reason `max_output_tokens`.                           |
| `broken.sse`           | A recorded Responses stream: text, then the connection drops in the middle of an event.                                      |

The `.md` streams are sent in small chunks, then completed; the `.sse` streams
are replayed through the Responses stream parser, written in the shape of
`../responses/`.
