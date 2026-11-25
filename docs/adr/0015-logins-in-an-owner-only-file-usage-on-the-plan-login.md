# ADR-0015: Logins in an owner-only file, usage on the plan login

- Status: Accepted
- Date: 2026-11-22
- Supersedes: the Keychain line of ADR-0009, the second sign-in of ADR-0007
  and the line of ADR-0014 that the meter keeps its own login

## Context

Atlas kept its logins in the macOS Keychain. macOS asks for access to an item
whenever the binary reading it carries a signature other than the one that
created it: after every development build, and with any unsigned build, the
candidate meets a Keychain prompt.

The usage meter (ADR-0007) needed a second, Codex-style sign-in of its own, so
every candidate signed in twice.

## Decision

- Logins live in one file, `secrets.json`, in the app's data folder
  (`~/Library/Application Support/com.maxilr.atlas/` for the app), the way OMP,
  the Codex CLI and Headroom keep theirs. The file is one JSON object keyed by
  record name, readable and writable by its owner only (`0600`, in a `0700`
  folder). A write replaces the whole file through a temporary file and a
  rename, so a crash never leaves half a login; a file found readable by
  others is narrowed before use. `atlas-live` reads the same file.
- Atlas never reads, writes or deletes a Keychain item again. A login an
  earlier build saved there is left untouched and nothing is migrated, because
  reading the old item is exactly what triggers the prompt: the candidate
  starts signed out and signs in once more.
- The usage meter reads `wham/usage` with the plan login. There is no second
  sign-in.

## Tradeoff

- Any program running as the candidate can read a token that spends the
  candidate's ChatGPT plan. The engineer chose this (option C) over keeping the
  Keychain.
- The file is not encrypted, and the data-protection Keychain is not used.
- The `wham/usage` endpoint is undocumented and can change without notice.
