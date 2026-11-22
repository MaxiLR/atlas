# ADR-0009: Scope of the first version

- Status: Accepted; its Keychain line is superseded by ADR-0015
- Date: 2026-11-18

## Context

The core loop is: sign in with ChatGPT, name a topic, placement, index, open a
section, Next or jump through the index, Go deeper, Define, Ask. This record
fixes what else the first version carries, so the spec starts from a settled
list.

## Decision

In the first version, beyond the core loop:

- The course library, with resume.
- Reading progress in the index: unread, read, deep dives opened.
- Check questions at the end of a section.
- "Explain differently".
- A model picker limited to what the plan login lists. The default is the first
  model listed, re-read at each launch, until the candidate picks one.
- Web-grounded sections with citations, through the hosted web-search tool. When
  the account's policy refuses search, the section is generated without it and
  says so.
- Export of a course to PDF.
- The usage meter (ADR-0007).

Also decided for the first version:

- Courses are written in English. A language selector per course is a later
  feature.
- A course can be deleted from the library after a confirmation; deletion is
  permanent.
- A PDF holds the course's tree as it stands: the generated sections of the
  broad pass and of the opened deep dives, with the glossary. Exporting
  generates nothing, and side chats are left out.
- Tokens are kept in the macOS Keychain.
- Maths is rendered with KaTeX.

Excluded from the first version: Claude in any form, API-key sign-in, voice,
export to Markdown, courses in other languages, Windows and Linux builds, sync
between devices, and a signed or notarised build.
