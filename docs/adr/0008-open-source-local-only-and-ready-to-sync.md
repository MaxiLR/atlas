# ADR-0008: Open source, local-only, and ready to sync

- Status: Accepted
- Date: 2026-11-18

## Context

Other people will run Atlas with their own subscriptions. OpenAI limits plan
usage through Sign in with ChatGPT to "open-source projects, personal projects
that run locally, and selected private apps"; paid or remotely hosted apps must
join a waitlist.

A later requirement is to keep the course library in step between devices, with
Atlas possibly reaching iPad, iPhone or the browser.

## Decision

Atlas is open source under the MIT licence, free, and runs entirely on the
candidate's machine. There is no server of ours and no account of ours.

The first version targets macOS on Apple silicon. Nothing in it may block
Windows, Linux or iOS later.

Courses are stored locally in SQLite, shaped so that sync can be added without
a migration of meaning: every record has a UUID and a timestamp, section
versions are immutable, and nothing is keyed to one machine.

## Tradeoff

The system prompt and the catalog are public. There is no sync, no Windows and
no Linux build in the first version.
