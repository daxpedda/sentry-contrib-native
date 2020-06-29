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
use sentry::{Event, Level, Options};
use sentry_contrib_native as sentry;
#[cfg(feature = "custom-transport")]
use test::custom_transport::Transport;

#[test]
fn event() -> Result<()> {
    sentry::test::set_hook();

    let mut runtime = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .thread_name("sentry-tokio")
        .build()
        .expect("failed to create tokio runtime");
    #[cfg(feature = "custom-transport")]
    let handle = runtime.handle().clone();

    runtime.block_on(async {
        let uuid = {
            let mut options = Options::new();
            options.set_debug(true);
            options.set_logger(|level, message| eprintln!("[{}]: {}", level, message));
            #[cfg(feature = "custom-transport")]
            options.set_transport(|options| Transport::new(handle, options));
            let _shutdown = options.init()?;

            Event::new_message(Level::Debug, None, "test message").capture()
        };

        assert_eq!("test message", test::check(uuid).await?.message);

        sentry::test::verify_panics();

        Ok(())
    })
}
