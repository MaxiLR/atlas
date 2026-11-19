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

## Licence

MIT, see `LICENSE`.
