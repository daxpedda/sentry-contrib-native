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
use sentry::Event;
use sentry_contrib_native as sentry;

#[tokio::test(threaded_scheduler)]
async fn lib_failure() -> Result<()> {
    util::events_failure(
        Some(|options| options.set_require_user_consent(true)),
        vec![
            || {
                sentry::user_consent_give();
                sentry::user_consent_revoke();
                Event::new().capture()
            },
            || {
                sentry::user_consent_give();
                sentry::user_consent_reset();
                Event::new().capture()
            },
        ],
    )
    .await?;

    Ok(())
}
