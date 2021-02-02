#![warn(
    clippy::cargo,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]

mod util;

use anyhow::Result;
use sentry::{Consent, Event};
use sentry_contrib_native as sentry;

#[tokio::test(flavor = "multi_thread")]
async fn lib_failure() -> Result<()> {
    util::events_failure(
        Some(|options| options.set_require_user_consent(true)),
        vec![
            || {
                sentry::set_user_consent(Consent::Given);
                sentry::set_user_consent(Consent::Revoked);
                Event::new().capture()
            },
            || {
                sentry::set_user_consent(Consent::Given);
                sentry::set_user_consent(Consent::Unknown);
                Event::new().capture()
            },
        ],
    )
    .await?;

    Ok(())
}
