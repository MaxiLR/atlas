# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

Tauri 2 desktop app for macOS on Apple silicon: a Rust core and a React 19
interface in the webview (TypeScript, Vite, Tailwind 4, shadcn/ui on Radix
Primitives, AI Elements, json-render). The design language is the web's, not
AppKit's. Decided in ADR-0001, ADR-0003, ADR-0010 and ADR-0012.

## Users

One candidate per installation: a self-directed adult at a Mac, learning a topic
that is new to them, technical or not, in focused sittings of thirty minutes or
more. The first candidate is the engineer; others run the open-source build with
their own ChatGPT plan.

## Product Purpose

Atlas teaches any topic broad first, then deep. It places the candidate with a
few questions, lays out the whole topic as an index before any reading, writes
each section when it is first opened, and keeps doubts out of the lesson by
answering them beside it. Success is a candidate who sees the whole territory
early, reads at their own level, and resumes any course where they left it.

## Positioning

A learning harness that is entirely Atlas's own, paid for by the ChatGPT plan
the candidate already has. Open source under MIT, free, local-only: no server of
ours and no account of ours. The lesson and the doubt are separate threads:
Define answers in place, a side chat answers beside the text, and neither
changes the section.

## Operating Context

- A desktop window, often beside another app; the minimum window width is 720 px.
- The core loop: sign in with ChatGPT, name a topic, placement, index, open a
  section, Next or jump through the index, Go deeper, Define, Ask.
- Around it: the course library with resume, reading progress, check questions,
  "Explain differently", the model picker, web-grounded sections with citations,
  PDF export, the usage meter.
- Every network call is made by the Rust core; the interface makes none.

## Capabilities and Constraints

- Vocabulary is fixed in `CONTEXT.md`: candidate, topic, course, library,
  concept ladder, placement, candidate profile, index, broad pass, section,
  version, deep dive, Next, check question, block, catalog, figure, Define,
  glossary, side chat, plan login, usage meter, window, pace mark.
- Lesson visuals come from a fixed catalog; the one free-form entry is a static
  SVG figure that may use only Atlas's colour and type tokens. No generated HTML
  or JavaScript, no interactive generated visuals.
- Courses are written in English in the first version.
- Light and dark themes come from one generator; the default follows the system.
  The exported PDF always uses the light theme.
- Excluded from the first version: voice, sync, Windows, Linux, iOS, sharing,
  editing generated content, a signed build.

## Brand Commitments

- The name is Atlas. The app icon is a placeholder; there is no logo.
- Linear's design language, made binding by the engineer in ADR-0010 and
  ADR-0011 and described in `docs/design/README.md`.
- Writing: direct, declarative, no exclamation marks, no hype words.

## Evidence on Hand

None: no real course content, screenshots, testimonials or brand assets. Design
drafts use authored sample lessons, labelled synthetic. Nothing may claim
endorsement by, or affiliation with, OpenAI, ChatGPT, Wikipedia or Linear.

## Product Principles

1. The whole territory before any depth.
2. Teach at the candidate's level, and keep correcting the estimate.
3. A doubt never derails the lesson.
4. Generated content looks like part of the app, and is never trusted as code.
5. The candidate's plan is theirs: show what is left, retry nothing silently.

## Accessibility & Inclusion

WCAG 2.2 AA contrast in both themes, full keyboard operation with visible focus,
and reduced motion respected.
