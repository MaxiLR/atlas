//! The secret store in one file, `secrets.json` in the app's data folder,
//! readable by its owner only. The file holds one JSON object keyed by record
//! name; each value is the opaque text the core encodes. No error carries a
//! value or any of the file, so none is ever logged.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions, Permissions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use atlas_core::ports::{SecretError, SecretRecord, SecretStore};
use uuid::Uuid;

/// The app's bundle identifier, which names its data folder.
pub const BUNDLE_IDENTIFIER: &str = "com.maxilr.atlas";

/// The secrets file inside the app's data folder.
pub const SECRETS_FILE: &str = "secrets.json";

/// The folder holding the file: its owner only.
const FOLDER_MODE: u32 = 0o700;

/// The file: readable and writable by its owner only.
const FILE_MODE: u32 = 0o600;

/// The secrets file in the app's data folder `data_dir`.
pub fn secrets_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SECRETS_FILE)
}

/// The secrets file the app keeps for the user whose home folder is `home`:
/// on the Mac the app's data folder is Application Support under the bundle
/// identifier. For `atlas-live`, which has no app to ask.
pub fn live_secrets_path(home: &Path) -> PathBuf {
    secrets_path(
        &home
            .join("Library")
            .join("Application Support")
            .join(BUNDLE_IDENTIFIER),
    )
}

/// The key of `record` in the file. The names are where the logins are found
/// after an update, so they never change.
pub fn record_name(record: SecretRecord) -> &'static str {
    match record {
        SecretRecord::PlanLogin => "plan-login",
    }
}

/// The file's contents: each record's value under its name.
type Logins = BTreeMap<String, String>;

/// The logins in one owner-only file. A lock serialises this process's
/// reads and writes of it.
pub struct SecretsFile {
    path: PathBuf,
    lock: Mutex<()>,
}

impl SecretsFile {
    /// The store on the file at `path`; nothing is created until a write.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    /// Writes the whole file, with `record` set to `value`, to a temporary
    /// file beside the old one, which stays as it was until
    /// [`Staged::commit`]. Holds the lock until then.
    pub fn stage(&self, record: SecretRecord, value: &str) -> Result<Staged<'_>, SecretError> {
        let guard = self.guard();
        let mut logins = self.load()?;
        logins.insert(record_name(record).to_owned(), value.to_owned());
        self.stage_locked(guard, &logins)
    }

    fn guard(&self) -> MutexGuard<'_, ()> {
        // The lock guards no data, so a panic while holding it leaves
        // nothing inconsistent.
        self.lock.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn folder(&self) -> &Path {
        self.path.parent().unwrap_or_else(|| Path::new("."))
    }

    /// The logins in the file, none when it is missing. A file readable by
    /// others is narrowed to its owner first.
    fn load(&self) -> Result<Logins, SecretError> {
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Logins::new()),
            Err(error) => return Err(self.failed("read", error)),
        };
        if metadata.permissions().mode() & 0o077 != 0 {
            fs::set_permissions(&self.path, Permissions::from_mode(FILE_MODE))
                .map_err(|error| self.failed("narrowed to its owner", error))?;
        }
        let bytes = fs::read(&self.path).map_err(|error| self.failed("read", error))?;
        // serde_json's messages can quote the file, so only the position is
        // kept.
        serde_json::from_slice(&bytes).map_err(|error| {
            SecretError(format!(
                "the secrets file {} is not a JSON object of text values \
                 (line {}, column {})",
                self.path.display(),
                error.line(),
                error.column()
            ))
        })
    }

    fn stage_locked<'a>(
        &'a self,
        guard: MutexGuard<'a, ()>,
        logins: &Logins,
    ) -> Result<Staged<'a>, SecretError> {
        let folder = self.folder();
        fs::create_dir_all(folder)
            .and_then(|()| fs::set_permissions(folder, Permissions::from_mode(FOLDER_MODE)))
            .map_err(|error| {
                SecretError(format!(
                    "the folder {} for the secrets file could not be prepared: {error}",
                    folder.display()
                ))
            })?;
        let text = serde_json::to_vec(logins)
            .map_err(|_| SecretError("the logins could not be encoded".to_owned()))?;
        let temporary = folder.join(format!(".{SECRETS_FILE}.{}.tmp", Uuid::new_v4()));
        let written = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(FILE_MODE)
            .open(&temporary)
            .and_then(|mut file| {
                file.write_all(&text)?;
                file.sync_all()
            });
        if let Err(error) = written {
            let _ = fs::remove_file(&temporary);
            return Err(SecretError(format!(
                "the temporary file {} could not be written: {error}",
                temporary.display()
            )));
        }
        Ok(Staged {
            store: self,
            temporary,
            _guard: guard,
        })
    }

    fn failed(&self, action: &str, error: io::Error) -> SecretError {
        SecretError(format!(
            "the secrets file {} could not be {action}: {error}",
            self.path.display()
        ))
    }
}

/// A whole new file written beside the old one, not yet in its place.
/// Dropped without [`Staged::commit`], it leaves the old file as it was.
pub struct Staged<'a> {
    store: &'a SecretsFile,
    temporary: PathBuf,
    _guard: MutexGuard<'a, ()>,
}

impl Staged<'_> {
    /// Moves the new file over the old one in one step.
    pub fn commit(self) -> Result<(), SecretError> {
        fs::rename(&self.temporary, &self.store.path).map_err(|error| {
            let _ = fs::remove_file(&self.temporary);
            self.store.failed("replaced", error)
        })
    }
}

impl SecretStore for SecretsFile {
    fn read(&self, record: SecretRecord) -> Result<Option<String>, SecretError> {
        let _guard = self.guard();
        Ok(self.load()?.remove(record_name(record)))
    }

    fn write(&self, record: SecretRecord, value: &str) -> Result<(), SecretError> {
        self.stage(record, value)?.commit()
    }

    fn delete(&self, record: SecretRecord) -> Result<(), SecretError> {
        let guard = self.guard();
        let mut logins = self.load()?;
        if logins.remove(record_name(record)).is_none() {
            return Ok(());
        }
        self.stage_locked(guard, &logins)?.commit()
    }
}
