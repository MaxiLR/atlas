# ADR-0013: A course map and section records keep a long course coherent

- Status: Accepted
- Date: 2026-11-19
- Amends: ADR-0004 (what each generation call receives)

## Context

A course is written one section at a time, by calls that share nothing but what
Atlas sends (ADR-0002). ADR-0004 sent the candidate profile, the index titles
and short summaries of the sections already read, the oldest condensed first.
That selects by recency. It carries no terminology, notation or examples from
one section to the next, and it gives a section no statement of what it owns and
what it may assume. Over a long topic the explanations drift apart.

The research read for this decision says:

- Long-range coherence comes mostly from a detailed plan made before writing. A
  detailed hierarchical outline beat a plan-plus-state baseline by 22.5 points
  on coherence in human evaluation
  ([DOC](https://arxiv.org/abs/2212.10077),
  [Re3](https://arxiv.org/abs/2210.06774)).
- The second ingredient is a structured record of what has been written,
  injected into each request and updated after it
  ([Re3](https://arxiv.org/abs/2210.06774),
  [RecurrentGPT](https://arxiv.org/abs/2305.13304),
  [DOME](https://arxiv.org/abs/2412.13575)).
- Input length alone lowers quality, even when the relevant content is retrieved
  perfectly
  ([Du et al.](https://aclanthology.org/2025.findings-emnlp.1264/)); the
  guidance is the smallest set of high-signal tokens
  ([Anthropic](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)).
- No memory architecture wins across workloads; a structured store helps only
  when its schema matches the work
  ([Chen et al.](https://aclanthology.org/2026.aacl-long.211.pdf)).
- Tutors need an explicit plan and tracked learner state, structured so the
  model can use them
  ([ScaffoldLM](https://aclanthology.org/2026.acl-long.325.pdf),
  [LongTutor](https://aclanthology.org/2026.acl-long.1371.pdf)).

The plan route has no file search, documents no embeddings for a plan token, and
resends the whole context on every tool call
([preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)).

## Decision

1. **Course map.** The request that builds the index also returns, for each
   section, the concepts it introduces and the concepts it relies on, and the
   course's conventions: notation, terminology and running examples. Every
   concept relied on is introduced by an earlier section. A concept names its
   ladder rung where it has one. Deep-dive proposals extend the map. The map is
   not shown.
2. **Section record.** Each section closes, in the same response, with a record
   in place of the bare summary: a summary, the concepts it taught with one line
   each as worded, the notation and terms it fixed, the examples it used and
   what it promised a later section. The core validates it and stores it with
   the section version. The first fixing of a convention holds for the course.
3. **Selection by the core.** With no request, the core gives a section its map
   entry, the conventions, the records of the sections that introduced what it
   relies on wherever they sit, the previous section's record, the promises due
   to it and, for a deep dive, its parent's record. Other sections read are one
   line per chapter. A prerequisite the candidate has not read is named, so the
   section recaps it. The full text of another section is never sent.

Side chats stay outside all of this (ADR-0005).

## Tradeoff

The index response is larger, and each section spends roughly 150 to 300 more
output tokens on its record. The record is the model's own account of its
section: nothing checks it against the prose. The map is fixed when the index is
built, so a plan that turns out wrong is not corrected.

Set aside: a consolidation request after each chapter that rewrites a digest and
the map (it spends plan usage unasked and changes an index already shown); a
lookup tool for the section writer (each call is another full request on this
route); embedding search over section text (no supported endpoint, and it sends
other sections' text). The stored map and records leave the first two open for
later.
