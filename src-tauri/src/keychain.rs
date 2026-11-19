//! The secret store on the macOS Keychain.

use atlas_core::ports::{SecretError, SecretRecord, SecretStore};

pub const SERVICE: &str = "";

pub fn account(_record: SecretRecord) -> &'static str {
    todo!()
}

pub fn encode(_value: &str) -> &[u8] {
    todo!()
}

pub fn decode(_record: SecretRecord, _bytes: Vec<u8>) -> Result<String, SecretError> {
    todo!()
}

pub struct Keychain;

impl Keychain {
    pub fn with_service(_service: String) -> Self {
        todo!()
    }
}

impl SecretStore for Keychain {
    fn read(&self, _record: SecretRecord) -> Result<Option<String>, SecretError> {
        todo!()
    }

    fn write(&self, _record: SecretRecord, _value: &str) -> Result<(), SecretError> {
        todo!()
    }

    fn delete(&self, _record: SecretRecord) -> Result<(), SecretError> {
        todo!()
    }
}
