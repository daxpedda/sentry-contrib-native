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
use sentry_contrib_native::{Event, Level, Options};

#[tokio::test]
async fn event() -> Result<()> {
    let uuid = {
        let mut options = Options::new();
        options.set_debug(true);
        let _shutdown = Options::new().init()?;

        Event::new_message(Level::Debug, None, "test message").capture()
    };

    assert_eq!("test message", test::check(uuid).await?.message);

    Ok(())
}
