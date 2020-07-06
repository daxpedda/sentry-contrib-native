#![warn(
    clippy::all,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]
// stable clippy seems to have an issue with await
#![allow(clippy::used_underscore_binding)]

mod util;

use anyhow::Result;
use libloading::{Library, Symbol};
use sentry::{Event, Level, User};
use sentry_contrib_native as sentry;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn lib_path() -> PathBuf {
    let mut path = PathBuf::from(env!("OUT_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap()
        .join("deps");

    #[cfg(target_os = "linux")]
    {
        path = path.join("libdylib.so");
    }
    #[cfg(target_os = "macos")]
    {
        path = path.join("libdylib.dylib");
    }
    #[cfg(target_os = "windows")]
    {
        path = path.join("dylib.dll");
    }

    path
}

#[tokio::test(threaded_scheduler)]
async fn lib() -> Result<()> {
    util::events_success(
        Some(|options| options.set_require_user_consent(true)),
        vec![
            (
                || {
                    sentry::user_consent_give();
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

                    let lib = Library::new(lib_path()).unwrap();
                    let func: Symbol<extern "C" fn() -> bool> =
                        unsafe { lib.get(b"test\0") }.unwrap();
                    assert_eq!(true, func());

                    sentry::clear_modulecache();
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
                        == lib_path().to_str().unwrap()));
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
