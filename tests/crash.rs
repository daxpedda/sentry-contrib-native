#![warn(
    clippy::all,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]
// stable clippy seems to have an issue with await
#![cfg(any(target_os = "macos", target_os = "windows"))]
#![allow(clippy::used_underscore_binding)]

mod util;

use anyhow::Result;

#[tokio::test(threaded_scheduler)]
async fn crash() -> Result<()> {
    util::external_events(vec![("crash".into(), |_| {})]).await
}
