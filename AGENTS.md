# Working on Atlas

Atlas is a Tauri 2 desktop app: a Rust core (`crates/atlas-core`, no Tauri
dependency), a thin shell (`src-tauri`, package `atlas-app`) and a React
interface (`ui/`). Vocabulary is in `CONTEXT.md`, decisions in `docs/adr/`.

## Environment

- macOS on Apple silicon. Rust stable lives in `~/.cargo/bin`, which is not on
  `PATH`: prefix cargo commands with `PATH="$HOME/.cargo/bin:$PATH"`.
- Node 24 with npm only (no pnpm, no bun).
- Writes are allowed only in the repository, the temp folder and `~/.npm`.
- The desktop app cannot be launched here. `cargo test --workspace` compiling
  `src-tauri` is the proof that the shell builds.
- Do not set a Cargo target directory in the repository; the environment
  supplies a shared one.
- Playwright's Chromium is installed by `ui`'s `postinstall` into
  `ui/node_modules` (`PLAYWRIGHT_BROWSERS_PATH=0`), so setup needs the network
  and the tests do not.

## The suite

Setup:

```sh
npm --prefix ui ci --no-audit --no-fund
```

Checks, in order:

```sh
npm --prefix ui run typecheck
npm --prefix ui run lint
npm --prefix ui run format:check
npm --prefix ui run build
PATH="$HOME/.cargo/bin:$PATH" cargo fmt --all --check
PATH="$HOME/.cargo/bin:$PATH" cargo clippy --workspace --all-targets --locked -- -D warnings
```

Tests, in order:

```sh
PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace --locked
npm --prefix ui run test
npm --prefix ui run test:e2e
```

- `build` leaves `ui/dist`, which the shell embeds.
- `format:check` runs Prettier (default options) over the whole repository and
  honours the root `.prettierignore`; `npm --prefix ui run format` fixes it.
- Running the suite must leave `git status` clean: generated files that are
  committed must come out identical when regenerated.
- Playwright runs Chromium headless with `--no-sandbox`, on a free port chosen
  at run time, against the app in Vite mode `fake`. Six projects (720, 1024
  and 1440 px, light and dark) run `ui/e2e/matrix/`; the `default` project
  (1440 px, dark) runs `ui/e2e/*.spec.ts`.

## Generated TypeScript types

The protocol (requests, responses, events, errors and the views they carry) is
defined in Rust in `crates/atlas-core/src/protocol/` and exported with ts-rs
into `ui/src/core/generated/`. Regenerate with:

```sh
npm --prefix ui run gen:types
```

`npm --prefix ui run typecheck` regenerates them before type-checking, so a
change on one side that the other does not follow fails the check. Commit the
generated files; never edit them by hand. The export folder is set in
`.cargo/config.toml`; the folder is excluded from Prettier and ESLint.

Only `CoreRequest`, `CoreResponse`, `CoreEvent` and `CoreError` are marked for
export; a type is generated when one of them reaches it.
`crates/atlas-core/tests/protocol.rs` fails when the committed files differ
from what the protocol generates, or when a core-only or token-bearing type
reaches the interface.

## Module map

The core (`crates/atlas-core/src/`):

| Module              | Owns                                                                                                                                                                                                                                 |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `lib.rs`, `core.rs` | `Core::new(deps)` and `Core::handle`, which routes each request to one feature method and runs `require_plan` for requests that spend plan usage.                                                                                    |
| `protocol/`         | The four tagged roots and the views the interface sees, one file per feature area. Generated to TypeScript.                                                                                                                          |
| `ports.rs`          | `Clock`, `SecretStore`, `Browser`, `EventSink`, `PdfExporter`, `Endpoints`, `SystemClock`.                                                                                                                                           |
| `testing/`          | In-memory fakes for every port (`FakePorts` builds `CoreDeps` from them), `FakeTokenSource`, and `FakeProvider`.                                                                                                                     |
| `features/<name>/`  | One per feature: its state built from `CoreDeps` and one method per request. session, external, models, library, navigation, placement, index, sections, versions, checks, deep_dives, define, wikipedia, side_chats, usage, export. |
| `domain/`           | Core-only types: ladder, candidate profile, course map, conventions, section record, keyed questions. They never implement `TS`.                                                                                                     |
| `store/`            | SQLite, migrations, repository functions.                                                                                                                                                                                            |
| `provider/`         | `ModelProvider` (with `list_models`), `TokenSource`, the fake in `fake.rs`, the plan provider in `chatgpt/`.                                                                                                                         |
| `auth/`             | Sign in with ChatGPT and the token types.                                                                                                                                                                                            |
| `harness/`          | `prompts.rs`, `structured/`, `splitter/`, `validate/`, `record/`, `context/`, `chat_context/`, `citations/`.                                                                                                                         |
| `placement/`        | The placement search over the ladder.                                                                                                                                                                                                |
| `index_tree/`       | The index tree: Next, index order, reading states.                                                                                                                                                                                   |
| `course_map/`       | Map validation and extension, convention merging.                                                                                                                                                                                    |
| `sanitise/`         | The figure sanitiser and `SanitisedSvg`.                                                                                                                                                                                             |
| `usage/`            | The usage reader.                                                                                                                                                                                                                    |
| `live/`             | The live command's runner and report.                                                                                                                                                                                                |

Tests in `crates/atlas-core/tests/` share `tests/support/`: `server.rs` serves
scripted routes on `127.0.0.1` at a free port and records requests;
`sample_course.rs` is a sample course as typed values (ladder, profile, an index
of three chapters with a deep dive, a valid map with conventions, a record per
read section).

The interface reaches the core only through `ui/src/core/`: the `CoreClient`
(`client.ts`, `tauri.ts`), the generated types and, in Vite mode `fake`, the
fake core in `fake/`.

## Prompts

Each prompt is one file in `prompts/`, with front matter naming its version,
embedded in the core through `crates/atlas-core/src/harness/prompts.rs`. Bump
the version whenever the text changes; stored output records the prompt and
version that produced it.

## Rules

- Tests use fakes and injected paths: the core gets its database path, its
  model provider, its event sink and its ports through `CoreDeps`; tests build
  it with `testing::FakePorts`, a `FakeProvider` and a database in a temp dir,
  and reach outside services only through `tests/support/server.rs`. The
  interface uses the in-browser fake core. No test or check may use the
  network, the real Keychain or the real app-data folder.
- The interface makes no network request; every outside call goes through the
  core. The fake core exists only in Vite mode `fake` and is never part of the
  production bundle.
- The whole dependency set is declared up front. Manifests (`Cargo.toml`,
  `ui/package.json`) and lockfiles (`Cargo.lock`, `ui/package-lock.json`)
  change only for a truly new dependency.
- Format every changed file: Prettier for web, config and Markdown files,
  `cargo fmt` for Rust.
- A feature ticket edits its own folder, its prompt files and its own test
  file. It never edits `lib.rs`, `core.rs`, `protocol/` or `src-tauri/` to land
  a feature; a feature not built yet answers `CoreError::Unavailable` naming
  itself.
- shadcn/ui components live in `ui/src/components/ui/` (added with
  `npx shadcn@latest add`), AI Elements patterns in
  `ui/src/components/ai-elements/`, Typeset in `ui/src/typeset.css`.
- Colours come only from the theme tokens. The generator in `ui/src/theme/`
  computes both themes at build time and the `atlas-theme` Vite plugin puts
  them in the page head as `--atlas-*` variables; `ui/src/styles/theme.css`
  maps them to Tailwind utilities and the shadcn variables. A
  `data-surface="raised"` subtree re-derives its tokens from its own ground; a
  `data-theme` subtree (`ThemeScope`) forces one theme.
