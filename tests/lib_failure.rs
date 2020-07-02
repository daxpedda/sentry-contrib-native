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
async fn lib_failure() -> Result<()> {
    util::events_failure(None,vec![]).await?;

    Ok(())
}
