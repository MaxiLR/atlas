# ADR-0005: Define in place, ask in a side chat

- Status: Accepted
- Date: 2026-11-18

## Context

A doubt while reading should be resolved without leaving or disturbing the
lesson, the way a reader looks something up beside a paper. Two kinds of doubt
were identified: what a term means, and a question worth a conversation.

## Decision

Selecting text offers two actions.

**Define** shows a short explanation at the selection, written for the candidate
profile and the surrounding sentence. It is one request with no conversation.
The definition is kept in the course glossary, the term keeps a faint dotted
underline, and reopening it costs no request. The popover also fetches the
Wikipedia summary of the term and links to the article.

**Ask** opens a side chat in the right-hand panel with its own history, anchored
to the passage, which keeps a faint margin mark that reopens it. A course holds
many side chats; one can start without a selection.

A side chat receives the selected passage, its section, the index titles and the
candidate profile. It never changes a section and never enters the context of
section generation.

Voice is not part of the first version. The plan route rejects audio input and
transcription (ADR-0002), so the later route is on-device dictation into the
same side chat.

## Tradeoff

Doubts raised in side chats do not make later sections adapt; only check
questions update the candidate profile. Marks add a little noise to the text. A
Wikipedia lookup is an unauthenticated request to a third party and can fetch
the wrong article for an ambiguous term.
