#![warn(
    clippy::all,
    clippy::nursery,
    clippy::missing_docs_in_private_items,
    clippy::pedantic,
    missing_docs
)]
// stable clippy seems to have an issue with await
#![allow(clippy::used_underscore_binding)]

#[cfg(not(feature = "default-transport"))]
mod custom_transport;
mod test;

use anyhow::Result;
#[cfg(not(feature = "default-transport"))]
use custom_transport::Transport;
use sentry::{Event, Level, Options};
use sentry_contrib_native as sentry;

#[tokio::test]
async fn event() -> Result<()> {
    sentry::test::set_hook();

    let uuid = {
        let mut options = Options::new();
        options.set_debug(true);
        options.set_logger(|_, s| eprintln!("{}", s));
        #[cfg(not(feature = "default-transport"))]
        options.set_transport(Transport::new);
        let _shutdown = Options::new().init()?;

        Event::new_message(Level::Debug, None, "test message").capture()
    };

    assert_eq!("test message", test::check(uuid).await?.message);

    sentry::test::verify_panics();

    Ok(())
}
