#![warn(
    clippy::cargo,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]

mod util;

use anyhow::Result;
use sentry::{Event, Options, Value};
use sentry_contrib_native as sentry;

#[tokio::test(flavor = "multi_thread")]
async fn before_send() -> Result<()> {
    util::events_success(
        Some(|options: &mut Options| {
            options.set_before_send(|mut value: Value| {
                let event = value.as_mut_map().unwrap();
                event.remove("extra");
                value
            });
        }),
        vec![(
            || {
                let mut event = Event::new();
                event.insert("extra", vec![("data", "test data")]);
                event.capture()
            },
            |event| {
                assert_eq!("<unlabeled event>", event.title);
                assert_eq!("error", event.tags.get("level").unwrap());
                assert!(event.context.is_empty());
                assert_eq!("", event.message);
                assert_eq!(None, event.tags.get("logger"));
            },
        )],
    )
    .await?;

    Ok(())
}
