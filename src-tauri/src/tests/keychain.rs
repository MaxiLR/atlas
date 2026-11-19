use std::collections::HashSet;

use atlas_core::ports::{SecretRecord, SecretStore};

use crate::keychain::{Keychain, SERVICE, account, decode, encode};

const RECORDS: [SecretRecord; 3] = [
    SecretRecord::PlanLogin,
    SecretRecord::IssuedClientId,
    SecretRecord::UsageLogin,
];

#[test]
fn items_live_under_the_bundle_identifier() {
    assert_eq!(SERVICE, "com.maxilr.atlas");
}

#[test]
fn each_record_is_its_own_item_under_a_stable_name() {
    // The names are where the logins are found after an update: they never
    // change.
    assert_eq!(account(SecretRecord::PlanLogin), "plan-login");
    assert_eq!(account(SecretRecord::IssuedClientId), "issued-client-id");
    assert_eq!(account(SecretRecord::UsageLogin), "usage-login");
    let names: HashSet<&str> = RECORDS.into_iter().map(account).collect();
    assert_eq!(names.len(), RECORDS.len(), "one item per record");
}

#[test]
fn a_record_round_trips_through_its_item_encoding() {
    for value in [
        "",
        r#"{"access_token":"a.b.c","expires_at":"2026-01-01T00:00:00Z"}"#,
        "oaiapp_7Qe2",
        "naïve – ünïcödé ✓",
    ] {
        for record in RECORDS {
            assert_eq!(
                decode(record, encode(value).to_vec()).as_deref(),
                Ok(value),
                "{record:?} keeps {value:?}"
            );
        }
    }
}

#[test]
fn an_item_that_is_not_text_is_an_error_that_shows_no_bytes() {
    let mut bytes = b"hunter2-".to_vec();
    bytes.push(0xff);
    let error = decode(SecretRecord::PlanLogin, bytes).expect_err("invalid UTF-8 is refused");
    let message = error.to_string();
    assert!(
        message.contains("plan-login"),
        "it names the item: {message}"
    );
    assert!(
        !message.contains("hunter2"),
        "it shows none of the value: {message}"
    );
}

/// Writes, reads, replaces and deletes a throwaway item in the real Keychain
/// under its own service, then checks that it is gone. Run by hand: it may
/// ask for Keychain access.
#[test]
#[ignore = "touches the real Keychain; run by hand with --ignored"]
fn a_throwaway_item_round_trips_through_the_real_keychain() {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a time after the epoch")
        .as_nanos();
    let keychain = Keychain::with_service(format!("{SERVICE}.throwaway-{nanos}"));
    let record = SecretRecord::UsageLogin;

    assert_eq!(keychain.read(record), Ok(None), "the item starts absent");
    keychain.write(record, "first ✓").expect("written");
    assert_eq!(keychain.read(record), Ok(Some("first ✓".to_owned())));
    keychain.write(record, "second").expect("replaced");
    assert_eq!(keychain.read(record), Ok(Some("second".to_owned())));
    keychain.delete(record).expect("deleted");
    assert_eq!(keychain.read(record), Ok(None), "the item is gone");
    assert_eq!(
        keychain.delete(record),
        Ok(()),
        "deleting it again succeeds"
    );
}
