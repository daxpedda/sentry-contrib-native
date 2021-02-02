#![warn(
    clippy::cargo,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]

mod util;

use anyhow::Result;
use sentry::{Event, Level};
use sentry_contrib_native as sentry;

#[tokio::test(flavor = "multi_thread")]
async fn event() -> Result<()> {
    util::events_success(
        None,
        vec![
            (
                || Event::new().capture(),
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!("error", event.tags.get("level").unwrap());
                    assert_eq!("", event.message);
                    assert_eq!(None, event.tags.get("logger"));
                },
            ),
            (
                || {
                    let mut event = Event::new();
                    event.insert("extra", vec![("data", "test data")]);
                    event.capture()
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!("error", event.tags.get("level").unwrap());
                    assert_eq!(
                        "test data",
                        event.context.get("data").unwrap().as_str().unwrap()
                    );
                    assert_eq!("", event.message);
                    assert_eq!(None, event.tags.get("logger"));
                },
            ),
            (
                || Event::new_message(Level::Debug, None, "test message").capture(),
                |event| {
                    assert_eq!("test message", event.title);
                    assert_eq!("debug", event.tags.get("level").unwrap());
                    assert_eq!("test message", event.message);
                    assert_eq!(None, event.tags.get("logger"));
                },
            ),
            (
                || {
                    Event::new_message(Level::Debug, Some("test logger".into()), "test message")
                        .capture()
                },
                |event| {
                    assert_eq!("test message", event.title);
                    assert_eq!("debug", event.tags.get("level").unwrap());
                    assert_eq!("test message", event.message);
                    assert_eq!("test logger", event.tags.get("logger").unwrap());
                },
            ),
            (
                || {
                    let mut event = Event::new();
                    event.add_stacktrace(0);
                    event.capture()
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!("error", event.tags.get("level").unwrap());
                    assert_eq!("", event.message);
                    assert_eq!(None, event.tags.get("logger"));
                    assert!(event.entries.get("threads").is_some());
                },
            ),
            (
                || {
                    let mut event = Event::new();
                    event.add_exception(
                        vec![
                            ("type", "test exception"),
                            ("value", "test exception value"),
                        ],
                        0,
                    );
                    event.capture()
                },
                |event| {
                    assert_eq!("test exception: test exception value", event.title);
                    assert_eq!("error", event.tags.get("level").unwrap());
                    assert_eq!("", event.message);
                    assert_eq!(None, event.tags.get("logger"));
                    assert!(event.entries.get("exception").is_some());
                },
            ),
        ],
    )
    .await?;

    Ok(())
}
