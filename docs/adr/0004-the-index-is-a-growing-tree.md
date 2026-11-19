# ADR-0004: The index is a growing tree, generated one section at a time

- Status: Accepted
- Date: 2026-11-18
- Amended by: ADR-0013 (what each generation call receives)

## Context

The fastest way into a new topic is to cover all of it broadly first and then go
deep where the candidate chooses. The index also has to keep the context of each
model call small: on the plan route nothing is stored between calls, so
everything a call needs is sent with it (ADR-0002).

Alternatives were a fixed three-level plan made up front (a long first wait,
depth capped where the plan stops) and a flat index with a depth dial (deeper
means a longer page, not new sub-topics).

## Decision

The first index is two levels, chapters then sections, covering the whole topic
at overview depth: the broad pass. Next walks it in order.

Any section has "Go deeper", which generates child sections under it on demand,
to any depth. Deep dives are side branches.

A section's content is generated when first opened, stored, and reopened without
regenerating. "Explain differently" adds a version and keeps the earlier ones.

Each generation call receives the candidate profile, the index titles and short
summaries of the sections already read. It never receives the full text of
earlier sections.

## Tradeoff

The tree is uneven, and a deep dive is only as good as the summary of its
parent.
