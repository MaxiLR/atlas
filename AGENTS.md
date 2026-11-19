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

## Prompts

Each prompt is one file in `prompts/`, with front matter naming its version,
embedded in the core through `crates/atlas-core/src/harness/prompts.rs`. Bump
the version whenever the text changes; stored output records the prompt and
version that produced it.

## Rules

- Tests use fakes and injected paths: the core gets its database path, its
  model provider and its event sink through `CoreDeps`; tests use
  `FakeProvider` and a database in a temp dir, the interface uses the in-browser
  fake core. No test or check may use the network, the real Keychain or the
  real app-data folder.
- The interface makes no network request; every outside call goes through the
  core. The fake core exists only in Vite mode `fake` and is never part of the
  production bundle.
- The whole dependency set is declared up front. Manifests (`Cargo.toml`,
  `ui/package.json`) and lockfiles (`Cargo.lock`, `ui/package-lock.json`)
  change only for a truly new dependency.
- Format every changed file: Prettier for web, config and Markdown files,
  `cargo fmt` for Rust.
- shadcn/ui components live in `ui/src/components/ui/` (added with
  `npx shadcn@latest add`), AI Elements patterns in
  `ui/src/components/ai-elements/`, Typeset in `ui/src/typeset.css`.
