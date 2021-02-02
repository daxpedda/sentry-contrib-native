#![warn(
    clippy::cargo,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]
#![cfg(crashpad)]

mod util;

use anyhow::Result;

#[tokio::test(flavor = "multi_thread")]
async fn crash() -> Result<()> {
    util::external_events_failure(vec!["crash_failure".into()]).await
}
