use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use atlas_core::ports::{SecretRecord, SecretStore};
use tempfile::TempDir;

use crate::secrets::{SecretsFile, live_secrets_path, secrets_path};

const RECORD: SecretRecord = SecretRecord::PlanLogin;

/// A store on `<temp>/com.maxilr.atlas/secrets.json`, a folder that does not
/// exist yet.
fn store_in_temp() -> (TempDir, PathBuf, SecretsFile) {
    let temp = tempfile::tempdir().expect("a temp folder");
    let path = temp.path().join("com.maxilr.atlas").join("secrets.json");
    let store = SecretsFile::new(path.clone());
    (temp, path, store)
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).expect("metadata").permissions().mode() & 0o777
}

fn object_in(path: &Path) -> BTreeMap<String, String> {
    serde_json::from_slice(&fs::read(path).expect("the file")).expect("a JSON object of strings")
}

fn other_files_in(dir: &Path, except: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .expect("the folder")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path != except)
        .collect()
}

#[test]
fn a_login_round_trips_and_a_second_write_replaces_the_first() {
    let (_temp, path, store) = store_in_temp();
    for value in [
        "",
        r#"{"access_token":"a.b.c","expires_at":"2026-01-01T00:00:00Z"}"#,
        "naïve – ünïcödé ✓",
    ] {
        store.write(RECORD, value).expect("written");
        assert_eq!(store.read(RECORD), Ok(Some(value.to_owned())));
    }

    store.write(RECORD, "first").expect("written");
    store.write(RECORD, "second").expect("replaced");
    assert_eq!(store.read(RECORD), Ok(Some("second".to_owned())));

    // The file is one JSON object keyed by record name, and a fresh store on
    // the same file reads it.
    assert_eq!(
        object_in(&path),
        BTreeMap::from([("plan-login".to_owned(), "second".to_owned())])
    );
    assert_eq!(
        SecretsFile::new(path).read(RECORD),
        Ok(Some("second".to_owned()))
    );
}

#[test]
fn a_write_leaves_an_owner_only_folder_and_file() {
    let (_temp, path, store) = store_in_temp();
    store.write(RECORD, "token").expect("written");
    assert_eq!(mode(path.parent().expect("a folder")), 0o700);
    assert_eq!(mode(&path), 0o600);
}

#[test]
fn a_write_narrows_an_existing_folder_to_the_owner() {
    let (_temp, path, store) = store_in_temp();
    let folder = path.parent().expect("a folder");
    fs::create_dir_all(folder).expect("the folder");
    fs::set_permissions(folder, fs::Permissions::from_mode(0o755)).expect("widened");

    store.write(RECORD, "token").expect("written");
    assert_eq!(mode(folder), 0o700);
    assert_eq!(mode(&path), 0o600);
}

#[test]
fn a_file_found_readable_by_others_is_narrowed_before_use() {
    let (_temp, path, store) = store_in_temp();
    fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
    fs::write(&path, r#"{"plan-login":"token"}"#).expect("an old file");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("widened");

    assert_eq!(store.read(RECORD), Ok(Some("token".to_owned())));
    assert_eq!(mode(&path), 0o600);
}

#[test]
fn a_missing_file_holds_no_login() {
    let (_temp, path, store) = store_in_temp();
    assert_eq!(store.read(RECORD), Ok(None));
    assert!(!path.exists(), "reading creates nothing");
}

#[test]
fn a_file_without_the_record_holds_no_login() {
    let (_temp, path, store) = store_in_temp();
    fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
    fs::write(&path, "{}").expect("an empty object");
    assert_eq!(store.read(RECORD), Ok(None));
}

#[test]
fn a_corrupt_file_is_an_error_that_names_it_and_shows_none_of_its_bytes() {
    for contents in [
        "hunter2-not-json",
        r#""hunter2""#,
        r#"{"plan-login":"hunter2""#,
        r#"{"plan-login":["hunter2"]}"#,
    ] {
        let (_temp, path, store) = store_in_temp();
        fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
        fs::write(&path, contents).expect("a corrupt file");

        let message = store
            .read(RECORD)
            .expect_err("a corrupt file is refused")
            .to_string();
        assert!(
            message.contains(&path.display().to_string()),
            "it names the file: {message}"
        );
        assert!(
            !message.contains("hunter2"),
            "it shows none of the file: {message}"
        );
    }
}

#[test]
fn a_failed_write_shows_none_of_the_value() {
    let temp = tempfile::tempdir().expect("a temp folder");
    let blocker = temp.path().join("not-a-folder");
    fs::write(&blocker, "").expect("a file where the folder should be");
    let store = SecretsFile::new(blocker.join("secrets.json"));

    let message = store
        .write(RECORD, "hunter2")
        .expect_err("no folder, no write")
        .to_string();
    assert!(
        message.contains(&blocker.display().to_string()),
        "it names where it failed: {message}"
    );
    assert!(
        !message.contains("hunter2"),
        "it shows none of the value: {message}"
    );
}

#[test]
fn deleting_an_absent_record_succeeds() {
    let (_temp, path, store) = store_in_temp();
    assert_eq!(store.delete(RECORD), Ok(()), "with no file");

    fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
    fs::write(&path, "{}").expect("a file without the record");
    assert_eq!(store.delete(RECORD), Ok(()), "with a file without it");
    assert_eq!(store.read(RECORD), Ok(None));
}

#[test]
fn deleting_a_login_removes_it() {
    let (_temp, _path, store) = store_in_temp();
    store.write(RECORD, "token").expect("written");
    store.delete(RECORD).expect("deleted");
    assert_eq!(store.read(RECORD), Ok(None));
    assert_eq!(store.delete(RECORD), Ok(()), "deleting it again succeeds");
}

#[test]
fn a_write_stopped_before_the_rename_leaves_the_old_file_intact() {
    let (_temp, path, store) = store_in_temp();
    store.write(RECORD, "old").expect("written");
    let before = fs::read(&path).expect("the old file");
    let folder = path.parent().expect("a folder").to_owned();

    // The new file is written in full beside the old one, then the process
    // stops before moving it into place.
    let staged = store.stage(RECORD, "new").expect("staged");
    let leftovers = other_files_in(&folder, &path);
    assert_eq!(leftovers.len(), 1, "a temporary file beside the old one");
    assert_eq!(mode(&leftovers[0]), 0o600, "created owner-only");
    drop(staged);

    assert_eq!(fs::read(&path).expect("the old file"), before);
    assert_eq!(store.read(RECORD), Ok(Some("old".to_owned())));

    // The leftover changes nothing for the next write either.
    store.write(RECORD, "newer").expect("written");
    assert_eq!(store.read(RECORD), Ok(Some("newer".to_owned())));
}

#[test]
fn a_committed_stage_replaces_the_file() {
    let (_temp, path, store) = store_in_temp();
    store.write(RECORD, "old").expect("written");
    store
        .stage(RECORD, "new")
        .expect("staged")
        .commit()
        .expect("moved into place");
    assert_eq!(store.read(RECORD), Ok(Some("new".to_owned())));
    assert_eq!(
        other_files_in(path.parent().expect("a folder"), &path),
        Vec::<PathBuf>::new(),
        "no temporary file is left"
    );
}

#[test]
fn concurrent_writes_leave_one_whole_login() {
    let (_temp, path, store) = store_in_temp();
    let store = Arc::new(store);
    let writers: Vec<_> = (0..8)
        .map(|n| {
            let store = store.clone();
            std::thread::spawn(move || store.write(RECORD, &format!("token-{n}")))
        })
        .collect();
    for writer in writers {
        writer.join().expect("a writer").expect("written");
    }
    let value = store.read(RECORD).expect("read").expect("a login");
    assert!(value.starts_with("token-"), "one whole login: {value}");
    assert_eq!(object_in(&path).len(), 1);
}

#[test]
fn the_app_keeps_its_secrets_in_its_data_folder() {
    let data_dir = Path::new("/data/com.maxilr.atlas");
    assert_eq!(
        secrets_path(data_dir),
        Path::new("/data/com.maxilr.atlas/secrets.json")
    );
}

#[test]
fn the_live_command_reads_the_apps_secrets_file() {
    let home = tempfile::tempdir().expect("a temp home");
    // The app's data folder on the Mac: Application Support under the bundle
    // identifier from `tauri.conf.json`.
    let identifier = crate::context().config().identifier.clone();
    assert_eq!(identifier, "com.maxilr.atlas");
    let app_data_dir = home
        .path()
        .join("Library")
        .join("Application Support")
        .join(&identifier);

    assert_eq!(live_secrets_path(home.path()), secrets_path(&app_data_dir));
    assert_eq!(
        live_secrets_path(home.path()),
        app_data_dir.join("secrets.json")
    );

    // A login the app writes there is the one the live command reads.
    SecretsFile::new(secrets_path(&app_data_dir))
        .write(RECORD, "token")
        .expect("written by the app");
    assert_eq!(
        SecretsFile::new(live_secrets_path(home.path())).read(RECORD),
        Ok(Some("token".to_owned()))
    );
}
