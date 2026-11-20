# Atlas

A desktop app that teaches any topic broad first, then deep. It runs on the
learner's own ChatGPT plan, entirely on their machine. See `PRODUCT.md` for
what it is and `docs/adr/` for how it is built.

Atlas is a Tauri 2 app for macOS on Apple silicon: a Rust core
(`crates/atlas-core`), the desktop shell (`src-tauri`) and a React interface
(`ui/`).

## Prerequisites

- Rust stable (`rustup`) and the Xcode command line tools.
- Node 24 with npm.

## Build and run locally

Install the interface's dependencies (this also downloads the Chromium used by
the browser tests):

```sh
npm --prefix ui ci
```

Run the desktop app in development, with the interface served by Vite:

```sh
npm --prefix ui run tauri dev
```

Build the app bundle (written to `release/bundle/macos/Atlas.app` under Cargo's
target folder):

```sh
npm --prefix ui run tauri build
```

The app signs in with ChatGPT and keeps its tokens in the macOS Keychain,
under the service `com.maxilr.atlas`. Courses are stored in SQLite in the
app's data folder. To run it without a login, against a scripted fake
provider, set `ATLAS_FAKE_PROVIDER=1`:

```sh
ATLAS_FAKE_PROVIDER=1 npm --prefix ui run tauri dev
```

To work on the interface in a browser, without the desktop shell, run it
against the in-browser fake core:

```sh
npm --prefix ui run dev:fake
```

## Tests

```sh
cargo test --workspace
npm --prefix ui run test
npm --prefix ui run test:e2e
```

One test round-trips a throwaway item through the real Keychain and is left
out of the suite; run it by hand (macOS may ask for Keychain access):

```sh
cargo test -p atlas-app -- --ignored
```

`AGENTS.md` lists every check and the rules the suite holds to.

## The live command

No automated check talks to the real ChatGPT plan: the suite runs on a fake
provider and fixtures written from documentation. `atlas-live` closes that gap
for the engineer. It runs each harness task once against the real plan, on a
throwaway course in a temporary store that is removed afterwards, through the
same code paths as the app, and prints a report.

It uses the plan login the app saved in the Keychain and never signs in
itself, so sign in with ChatGPT in the app first (and turn plan usage on).
Without a login it says so and stops before any request. macOS may ask for
Keychain access the first time.

```sh
cargo run -p atlas-app --bin atlas-live -- --all
cargo run -p atlas-app --bin atlas-live -- --task write-section --task define
cargo run -p atlas-app --bin atlas-live -- --all --model <slug> --capture /tmp/atlas-streams
```

The shell embeds the built interface, so run `npm --prefix ui run build` once
before the first `cargo run`.

- `--task <name>`, repeatable, selects tasks: `draft-ladder`,
  `placement-question`, `build-index`, `write-section`, `propose-deep-dives`,
  `explain-differently`, `define`, `side-chat-reply`. `--all` selects every
  one. Tasks run in that order whatever the order given.
- `--model <slug>` uses a listed model instead of the first one.
- `--capture <dir>` writes the raw Responses streams the plan sent, one
  `<number>-<task>.sse` file each, as candidates for the fixtures in
  `crates/atlas-core/tests/fixtures/`. They hold the response bodies only,
  never a token. Point it outside the repository and review a stream before
  committing it as a fixture.

The run spends plan usage. It warns first and waits five seconds, then makes
one request per selected task (two for `write-section`) plus the repairs the
core asks for; nothing else reaches the model. A task that a selected one needs
but that is not selected is seeded in the store instead, with no request. The
command exits non-zero when a task failed.

### Reading the report

One line per task:

| Column       | Meaning                                                                                                                                                                                                                                     |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Result`     | `passed`; `FAILED`, with the error state on the line below (for example the plan's usage limit); or `skipped`, when a task it needs failed first.                                                                                           |
| `Structured` | For the data tasks (ladder, question, index, deep dives): `accepted` when the plan route answered a request with structured output, `fallback used` when it refused it and the schema went in the prompt, `-` when no request was answered. |
| `Search`     | For write section, explain differently and side-chat reply: `allowed` when the route answered a request with the hosted web search, `refused` when it refused it and the text was written without it, `-` when no request was answered.     |
| `Repairs`    | The repair requests the task needed: a structured answer, a block or a section record that failed validation.                                                                                                                               |
| `Model`      | The model the requests named.                                                                                                                                                                                                               |
| `Time`       | The time its requests took, repairs included.                                                                                                                                                                                               |

Beneath a task, its course memory:

- `build-index`, `map:` whether the course map was valid at once or only after
  its repair, and how many concepts and conventions it fixed.
- `write-section` (two sections, in order, so that the second one's context
  carries the first one's record) and `explain-differently`, one line per
  version: whether its record was confirmed, or failed twice and the record its
  map entry predicts was stored unconfirmed; how many of its notation and term
  entries were rejected as conflicting with a convention already fixed; and
  the size of the request's input in characters.
- `propose-deep-dives`, `map extension:` whether the extension was valid at
  once or after its repair.

Then the models the plan lists, in the order it reports them, and the usage
meter: one reading when a usage login exists, otherwise `no usage login`.

What to confirm with a real login (ADR-0002, ADR-0013):

- Structured output is accepted: `accepted` on the four data tasks.
  `fallback used` means the route refuses it and every data task relies on the
  schema in the prompt.
- Web search is allowed for the account: `allowed` on the three tasks that
  search. `refused` means sections and replies are written without sources.
- The first listed model is the newest: check the first line of the model list,
  which is the default.
- The course maps are valid: `valid at once` is the hope; `valid after its
repair` works but costs a request each time.
- The records are confirmed: `record confirmed` on every version. An
  unconfirmed record leaves later sections only the planned concepts to build
  on; rejected entries mean the model redefined something the course had
  already fixed.

The command does not judge the quality of what the prompts write, and does not
check a record against its prose; judge those by using the app.

## Licence

MIT, see `LICENSE`.
