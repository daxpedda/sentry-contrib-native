//! Sentry supported panic handler.

use crate::{Event, Level, Map, Object};
use std::{
    convert::TryFrom,
    panic::{self, PanicInfo},
};

/// Panic handler to send an event with the current stacktrace to Sentry.
///
/// This will not work properly if used with `panic = "abort"` because
/// [`Shutdown`](crate::Shutdown) is never unwound. To fix this make sure you
/// make the panic handler itself call [`shutdown`](crate::shutdown).
///
/// # Examples
/// ```should_panic
/// # use anyhow::Result;
/// # use sentry_contrib_native::{Options, set_hook};
/// fn main() -> Result<()> {
///     // pass original panic handler provided by rust to retain it's functionality
///     set_hook(Some(std::panic::take_hook()));
///     // it can also be removed
///     set_hook(None);
///
///     let _shutdown = Options::new().init()?;
///
///     panic!("application panicked")
/// }
/// ```
///
/// If you are using `panic = "abort"` make sure to call
/// [`shutdown`](crate::shutdown) inside the panic handler.
/// ```
/// # use sentry_contrib_native::{set_hook, shutdown};
/// set_hook(Some(Box::new(|_| shutdown())));
/// ```
pub fn set_hook(hook: Option<Box<dyn Fn(&PanicInfo) + Sync + Send + 'static>>) {
    panic::set_hook(Box::new(move |panic_info| {
        let mut event = Event::new_message(
            Level::Error,
            Some("rust panic".into()),
            panic_info.to_string(),
        );

        if let Some(location) = panic_info.location() {
            let mut extra = Map::new();
            extra.insert("file", location.file());

            if let Ok(line) = i32::try_from(location.line()) {
                extra.insert("line", line);
            }

            if let Ok(column) = i32::try_from(location.column()) {
                extra.insert("column", column);
            }

            event.insert("extra", extra);
        }

        event.add_stacktrace(0);
        event.capture();

        if let Some(hook) = &hook {
            hook(panic_info)
        }
    }));
}

#[cfg(test)]
#[rusty_fork::test_fork]
#[should_panic]
fn hook() -> anyhow::Result<()> {
    use crate::Options;

    set_hook(None);

    let _shutdown = Options::new().init()?;
    panic!("this panic is a test");
}
