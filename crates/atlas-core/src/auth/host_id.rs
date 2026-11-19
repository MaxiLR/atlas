//! The installation's host id, kept in a file of its own beside the course
//! records rather than in them.

use std::fs;
use std::io;
use std::path::Path;

use uuid::Uuid;

use super::HostId;

const PREFIX: &str = "urn:uuid:";

impl HostId {
    /// The host id kept in the file at `path`, generated and written there the
    /// first time. A file that does not hold a `urn:uuid:` is replaced.
    pub fn load_or_create(path: &Path) -> io::Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) if is_host_id(text.trim()) => return Ok(Self(text.trim().to_owned())),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let host_id = Self(format!("{PREFIX}{}", Uuid::new_v4()));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let partial = path.with_extension("partial");
        fs::write(&partial, &host_id.0)?;
        fs::rename(&partial, path)?;
        Ok(host_id)
    }
}

fn is_host_id(value: &str) -> bool {
    value
        .strip_prefix(PREFIX)
        .is_some_and(|uuid| Uuid::parse_str(uuid).is_ok())
}
