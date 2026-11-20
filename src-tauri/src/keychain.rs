//! The secret store on the macOS Keychain: one generic-password item per
//! record under the service `com.maxilr.atlas`. Values are opaque text the
//! core encodes; no error carries one, so none is ever logged.

use atlas_core::ports::{SecretError, SecretRecord, SecretStore};
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

/// The service every item is filed under: the bundle identifier.
pub const SERVICE: &str = "com.maxilr.atlas";

/// The Keychain's status for an item that does not exist
/// (`errSecItemNotFound`).
const ITEM_NOT_FOUND: i32 = -25300;

/// The item's account name for `record`. The names are where the logins are
/// found after an update, so they never change.
pub fn account(record: SecretRecord) -> &'static str {
    match record {
        SecretRecord::PlanLogin => "plan-login",
        SecretRecord::UsageLogin => "usage-login",
    }
}

/// The item's data for a record's value: its UTF-8 bytes.
pub fn encode(value: &str) -> &[u8] {
    value.as_bytes()
}

/// The record's value from its item's data. Data that is not UTF-8 is an
/// error naming the item, without any of the data.
pub fn decode(record: SecretRecord, data: Vec<u8>) -> Result<String, SecretError> {
    String::from_utf8(data).map_err(|_| {
        SecretError(format!(
            "the Keychain item {} does not hold text",
            account(record)
        ))
    })
}

/// The Keychain's items under one service.
pub struct Keychain {
    service: String,
}

impl Keychain {
    /// Atlas's items.
    pub fn atlas() -> Self {
        Self::with_service(SERVICE.to_owned())
    }

    /// The items under another service, for a throwaway test item.
    pub fn with_service(service: String) -> Self {
        Self { service }
    }
}

fn failed(
    action: &str,
    record: SecretRecord,
    error: security_framework::base::Error,
) -> SecretError {
    SecretError(format!(
        "the Keychain could not {action} the item {}: {error}",
        account(record)
    ))
}

impl SecretStore for Keychain {
    fn read(&self, record: SecretRecord) -> Result<Option<String>, SecretError> {
        match get_generic_password(&self.service, account(record)) {
            Ok(data) => decode(record, data).map(Some),
            Err(error) if error.code() == ITEM_NOT_FOUND => Ok(None),
            Err(error) => Err(failed("read", record, error)),
        }
    }

    fn write(&self, record: SecretRecord, value: &str) -> Result<(), SecretError> {
        set_generic_password(&self.service, account(record), encode(value))
            .map_err(|error| failed("write", record, error))
    }

    fn delete(&self, record: SecretRecord) -> Result<(), SecretError> {
        match delete_generic_password(&self.service, account(record)) {
            Err(error) if error.code() != ITEM_NOT_FOUND => Err(failed("delete", record, error)),
            _ => Ok(()),
        }
    }
}
