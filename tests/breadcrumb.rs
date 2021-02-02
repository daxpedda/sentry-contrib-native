#![warn(
    clippy::cargo,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]

mod util;

use anyhow::Result;
use sentry::{Breadcrumb, Event};
use sentry_contrib_native as sentry;
use serde_json::Value;

#[tokio::test(flavor = "multi_thread")]
async fn breadcrumb() -> Result<()> {
    util::events_success(
        None,
        vec![
            (
                || {
                    Breadcrumb::new(None, Some("test message".into())).add();
                    Event::new().capture()
                },
                |event| {
                    assert_eq!("<unlabeled event>", event.title);
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
