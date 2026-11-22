//! The secret store in one owner-only file. Not built yet.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use atlas_core::ports::{SecretError, SecretRecord, SecretStore};

/// The app's bundle identifier, which names its data folder.
pub const BUNDLE_IDENTIFIER: &str = "com.maxilr.atlas";

/// The secrets file inside the app's data folder.
pub const SECRETS_FILE: &str = "secrets.json";

/// The secrets file in the app's data folder `data_dir`.
pub fn secrets_path(_data_dir: &Path) -> PathBuf {
    PathBuf::new()
}

/// The secrets file the app keeps for the user whose home folder is `home`.
pub fn live_secrets_path(_home: &Path) -> PathBuf {
    PathBuf::new()
}

/// The logins in one file.
pub struct SecretsFile {
    _path: PathBuf,
    _lock: Mutex<()>,
}

impl SecretsFile {
    pub fn new(path: PathBuf) -> Self {
        Self {
            _path: path,
            _lock: Mutex::new(()),
        }
    }

    /// Writes the file with `record` set to `value` beside the old one,
    /// without replacing it.
    pub fn stage(&self, _record: SecretRecord, _value: &str) -> Result<Staged<'_>, SecretError> {
        Err(unbuilt())
    }
}

/// A whole new file written beside the old one, not yet in its place.
pub struct Staged<'a> {
    _store: &'a SecretsFile,
}

impl Staged<'_> {
    /// Moves the new file over the old one.
    pub fn commit(self) -> Result<(), SecretError> {
        Err(unbuilt())
    }
}

fn unbuilt() -> SecretError {
    SecretError("the secrets file is not built yet".to_owned())
}

impl SecretStore for SecretsFile {
    fn read(&self, _record: SecretRecord) -> Result<Option<String>, SecretError> {
        Err(unbuilt())
    }

    fn write(&self, _record: SecretRecord, _value: &str) -> Result<(), SecretError> {
        Err(unbuilt())
    }

    fn delete(&self, _record: SecretRecord) -> Result<(), SecretError> {
        Err(unbuilt())
    }
}
