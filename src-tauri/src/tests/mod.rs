//! The shell's pure parts. None of these tests touches the Keychain, the
//! app's data folder or the network, except the one ignored Keychain
//! round-trip, run by hand with `cargo test -p atlas-app -- --ignored`.

mod events;
mod keychain;
mod navigation;
mod provider_choice;
