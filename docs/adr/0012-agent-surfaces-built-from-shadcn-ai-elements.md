# ADR-0012: Agent surfaces are built from shadcn/ui AI Elements

- Status: Accepted
- Date: 2026-11-18

## Context

Wherever the candidate talks to the agent or watches it work, the interface
should feel like one seamless product: a reply that streams without the page
jumping, a clear sign of what the agent is doing, sources that open where they
are cited.

shadcn/ui ships [AI Elements](https://ui.shadcn.com/docs/ai-elements): patterns
for agentic interfaces. Each is a composition of shadcn/ui components copied
into the project; there is no package. They exist in the Radix flavour chosen in
ADR-0010 and are built on a few shared components (Message, Bubble, Trace, Code
Block). Two behaviours come as headless primitives in `@shadcn/react`: Message
Scroller, which follows streamed output and keeps the reader's place, and
Questionnaire, which presents one question at a time with single, multiple,
freeform and skipped answers.

## Decision

Atlas builds its agent-facing surfaces from AI Elements, restyled with the rest
of the component set (ADR-0010, ADR-0011), instead of composing its own.

| Atlas surface                                    | AI Element                                    |
| ------------------------------------------------ | --------------------------------------------- |
| Side chat                                        | Chat, Message, Message Scroller, Prompt Input |
| Follow-up prompts in a side chat or after Define | Suggestion                                    |
| Placement and check questions                    | Questionnaire                                 |
| A section while it is being generated            | Task, Reasoning                               |
| A web search the agent runs                      | Tool                                          |
| Citations in a web-grounded section              | Inline Citation, Sources                      |
| Model picker                                     | Model Selector                                |

Not used, because they are patterns for coding agents: Checkpoint, Confirmation,
File Tree, Queue, Stack Trace, Subagent, Terminal, Test Results.

AI Elements cover the agent's chrome; the visuals inside a lesson stay with the
catalog (ADR-0003). Where the model's output needs one of these patterns, such
as a check question or a citation, the catalog entry maps to the same component,
so there is still one component set.

The harness stays in the Rust core (ADR-0002). The patterns receive their data
through props from a small store fed by the core's typed events. The interface
does not use the Vercel AI SDK's `useChat` or its message shape: sections are
not conversations, and one protocol covers sections, Define and side chats.

Lesson prose and chat replies are styled with shadcn's Typeset: one CSS file
that sets rhythm from three values, follows the theme and does not shift earlier
blocks while a reply streams.

A side chat does not show how much of the context window it has used (the
Context pattern) in the first version.

## Tradeoff

The patterns are source in this repository, restyled, so upstream improvements
are merged by hand. shadcn's examples, its chatbot template and its helper for
testing conversations are wired to the AI SDK's `useChat`; Atlas's are not, so
they are a guide to composition, not code to copy whole.
