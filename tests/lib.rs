#![warn(
    clippy::cargo,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]

mod util;

use anyhow::Result;
use libloading::{Library, Symbol};
use sentry::{Consent, Event, Level, User};
use sentry_contrib_native as sentry;
use serde_json::Value;

#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::too_many_lines)]
async fn lib() -> Result<()> {
    util::events_success(
        Some(|options| options.set_require_user_consent(true)),
        vec![
            (
                || {
                    sentry::set_user_consent(Consent::Given);
                    Event::new().capture()
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                },
            ),
            (
                || {
                    // collect libs first before we load a foreign one
                    let mut event = Event::new();
                    event.add_stacktrace(0);
                    event.capture();

                    let lib_location = dylib::location();
                    let lib = unsafe { Library::new(&lib_location) }.unwrap();
                    sentry::clear_modulecache();
                    assert!(sentry::modules_list()
                        .contains(&lib_location.to_str().unwrap().to_string()));
                    let func: Symbol<extern "C" fn() -> bool> =
                        unsafe { lib.get(b"test\0") }.unwrap();
                    assert!(func());

                    let mut event = Event::new();
                    event.add_stacktrace(0);
                    event.capture()
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    let libs = event
                        .entries
                        .get("debugmeta")
                        .and_then(|v| v.get("images"))
                        .and_then(Value::as_array)
                        .unwrap();
                    assert!(libs.iter().any(|v| v
                        .get("code_file")
                        .and_then(Value::as_str)
                        .unwrap()
                        == dylib::location().to_str().unwrap()));
                },
            ),
            (
                || {
                    let mut user = User::new();
                    user.insert("id", 1);
                    user.set();
                    sentry::remove_user();
                    Event::new().capture()
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!(None, event.user);
                },
            ),
            (
                || {
                    sentry::set_tag("test-tag", "test");
                    let uuid = Event::new().capture();
                    sentry::remove_tag("test-tag");
                    uuid
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!("test", event.tags.get("test-tag").unwrap());
                },
            ),
            (
                || {
                    sentry::set_tag("test-tag", "test");
                    sentry::remove_tag("test-tag");
                    Event::new().capture()
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!(None, event.tags.get("test-tag"));
                },
            ),
            (
                || {
                    sentry::set_extra("test tag", "test");
                    let uuid = Event::new().capture();
                    sentry::remove_extra("test tag");
                    uuid
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!("test", event.context.get("test tag").unwrap());
                },
            ),
            (
                || {
                    sentry::set_extra("test tag", "test");
                    sentry::remove_extra("test tag");
                    Event::new().capture()
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!(None, event.context.get("test tag"));
                },
            ),
            (
                || {
                    sentry::set_context("test context", vec![("type", "os"), ("name", "Redox")]);
                    let uuid = Event::new().capture();
                    sentry::remove_context("test context");
                    uuid
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    let context = event.contexts.get("test context").unwrap();
                    assert_eq!("os", context.r#type);
                    assert_eq!("Redox", context.get("name").unwrap());
                },
            ),
            (
                || {
                    sentry::set_context("test context", vec![("type", "os"), ("name", "Redox")]);
                    sentry::remove_context("test context");
                    Event::new().capture()
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!(None, event.contexts.get("test context"));
                },
            ),
            (
                || {
                    sentry::set_transaction("test transaction");
                    let uuid = Event::new().capture();
                    sentry::remove_transaction();
                    uuid
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!("test transaction", event.tags.get("transaction").unwrap());
                },
            ),
            (
                || {
                    sentry::set_level(Level::Info);
                    let uuid = Event::new().capture();
                    sentry::set_level(Level::Error);
                    uuid
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!("info", event.tags.get("level").unwrap());
                },
            ),
        ],
    )
    .await?;

    Ok(())
}
