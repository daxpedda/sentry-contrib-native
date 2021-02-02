#![warn(
    clippy::cargo,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]

mod util;

use anyhow::Result;

#[tokio::test(flavor = "multi_thread")]
async fn panic() -> Result<()> {
    util::external_events_success(vec![("panic".into(), |event| {
        assert_eq!(
            "panicked at 'test panic', tests/res/panic.rs:31:5",
            event.title
        );
        assert_eq!(Some(5), event.context.get("column").unwrap().as_i64());
        assert_eq!(Some(31), event.context.get("line").unwrap().as_i64());
    })])
    .await
}
