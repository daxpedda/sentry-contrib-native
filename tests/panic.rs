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

#[tokio::test(threaded_scheduler)]
async fn panic() -> Result<()> {
    util::external_events_success(vec![("panic".into(), |event| {
        assert_eq!(
            "panicked at 'test panic', tests/res/panic.rs:32:5",
            event.title
        );
        assert_eq!(Some(5), event.context.get("column").unwrap().as_i64());
        assert_eq!(Some(32), event.context.get("line").unwrap().as_i64());
    })])
    .await
}
