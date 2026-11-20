use std::ffi::OsStr;

use atlas_core::provider::ProviderChoice;

use crate::{FAKE_PROVIDER, provider_choice};

#[test]
fn the_variable_is_named_atlas_fake_provider() {
    assert_eq!(FAKE_PROVIDER, "ATLAS_FAKE_PROVIDER");
}

#[test]
fn without_the_variable_the_app_uses_the_plan() {
    assert_eq!(provider_choice(None), ProviderChoice::Plan);
}

#[test]
fn set_to_one_the_app_uses_the_fake_provider() {
    assert_eq!(provider_choice(Some(OsStr::new("1"))), ProviderChoice::Fake);
}

#[test]
fn any_other_value_keeps_the_plan() {
    for value in ["", "0", "false", "no", " 1", "11", "yes"] {
        assert_eq!(
            provider_choice(Some(OsStr::new(value))),
            ProviderChoice::Plan,
            "{value:?} keeps the plan"
        );
    }
}
