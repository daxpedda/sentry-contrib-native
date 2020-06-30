#![warn(
    clippy::all,
    clippy::nursery,
    clippy::missing_docs_in_private_items,
    clippy::pedantic,
    missing_docs
)]
// stable clippy seems to have an issue with await
#![allow(clippy::used_underscore_binding)]

mod test;

use anyhow::Result;
use sentry::{Breadcrumb, Event};
use sentry_contrib_native as sentry;
use serde_json::Value;

#[tokio::test(threaded_scheduler)]
async fn event() -> Result<()> {
    test::events(
        None,
        vec![
            (
                || {
                    Breadcrumb::new(None, Some("test message".into())).add();
                    Event::new().capture()
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!("error", event.tags.get("level").unwrap());
                    assert!(event.context.is_empty());
                    assert_eq!("", event.message);
                    assert_eq!(None, event.tags.get("logger"));
                    let breadcrumb = event
                        .entries
                        .get("breadcrumbs")
                        .and_then(|v| v.get("values"))
                        .and_then(Value::as_array)
                        .and_then(|v| v.get(0))
                        .and_then(Value::as_object)
                        .unwrap();
                    assert_eq!(
                        Some("default"),
                        breadcrumb.get("type").and_then(Value::as_str)
                    );
                    assert_eq!(
                        Some("test message"),
                        breadcrumb.get("message").and_then(Value::as_str)
                    );
                },
            ),
            (
                || {
                    Breadcrumb::new(Some("test type".into()), Some("test message".into())).add();
                    Event::new().capture()
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!("error", event.tags.get("level").unwrap());
                    assert!(event.context.is_empty());
                    assert_eq!("", event.message);
                    assert_eq!(None, event.tags.get("logger"));
                    let breadcrumb = event
                        .entries
                        .get("breadcrumbs")
                        .and_then(|v| v.get("values"))
                        .and_then(Value::as_array)
                        .and_then(|v| v.get(1))
                        .and_then(Value::as_object)
                        .unwrap();
                    assert_eq!(
                        Some("test type"),
                        breadcrumb.get("type").and_then(Value::as_str)
                    );
                    assert_eq!(
                        Some("test message"),
                        breadcrumb.get("message").and_then(Value::as_str)
                    );
                },
            ),
            (
                || {
                    let mut breadcrumb = Breadcrumb::new(None, None);
                    breadcrumb.insert("data", vec![("test data", "test data value")]);
                    breadcrumb.add();
                    Event::new().capture()
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
                    assert_eq!("error", event.tags.get("level").unwrap());
                    assert!(event.context.is_empty());
                    assert_eq!("", event.message);
                    assert_eq!(None, event.tags.get("logger"));
                    let breadcrumb = event
                        .entries
                        .get("breadcrumbs")
                        .and_then(|v| v.get("values"))
                        .and_then(Value::as_array)
                        .and_then(|v| v.get(2))
                        .and_then(Value::as_object)
                        .unwrap();
                    assert_eq!(
                        Some("default"),
                        breadcrumb.get("type").and_then(Value::as_str)
                    );
                    assert_eq!(None, breadcrumb.get("message").and_then(Value::as_str));
                    assert_eq!(
                        Some("test data value"),
                        breadcrumb
                            .get("data")
                            .and_then(Value::as_object)
                            .and_then(|v| v.get("test data"))
                            .and_then(Value::as_str)
                    );
                },
            ),
        ],
    )
    .await?;

    Ok(())
}
