//! Sentry supported panic handler.

#[cfg(doc)]
use crate::{shutdown, Shutdown};
use crate::{Event, Level, Value};
#[cfg(doc)]
use std::process::abort;
use std::{
    collections::BTreeMap,
    convert::TryFrom,
    panic::{self, PanicInfo},
};

/// Panic handler to send an [`Event`] with the current stacktrace to Sentry.
///
/// `before_send` is a callback that is able to modify the [`Event`] before it
/// is captures.
///
/// `hook` is a callback that is run after the [`Event`] is captured.
///
/// # Notes
/// This will not work properly if used with `panic = "abort"` because
/// [`Shutdown`] is never unwound. To fix this make sure you make the panic
/// handler itself call [`shutdown`].
///
/// Rust doesn't allow panics inside of a panicking thread and reacts with an
/// [`abort`]: if a custom transport or a before-send callback was registered
/// that can panic, it might lead to any [`panic!`] being an [`abort`] instead.
///
/// # Examples
/// ```should_panic
/// # use anyhow::Result;
/// # use sentry_contrib_native::{Options, set_hook};
/// fn main() -> Result<()> {
///     // pass original panic handler provided by rust to retain it's functionality
///     set_hook(None, Some(std::panic::take_hook()));
///     // it can also be removed
///     set_hook(None, None);
///     // the `Event` sent by a panic can also be modified
///     set_hook(
///         Some(Box::new(|mut event| {
///             // do something with the event and then return it
///             event
///         })),
///         None,
///     );
///
///     let _shutdown = Options::new().init()?;
///
///     panic!("application panicked")
/// }
/// ```
/// If you are using `panic = "abort"` make sure to call [`shutdown`] inside the
/// panic handler.
/// ```
/// # use sentry_contrib_native::{set_hook, shutdown};
/// set_hook(None, Some(Box::new(|_| shutdown())));
/// ```
pub fn set_hook(
    before_send: Option<Box<dyn Fn(Event) -> Event + 'static + Send + Sync>>,
    hook: Option<Box<dyn Fn(&PanicInfo) + 'static + Send + Sync>>,
) {
    panic::set_hook(Box::new(move |panic_info| {
        let mut event = Event::new_message(
            Level::Error,
            Some("rust panic".into()),
            panic_info.to_string(),
        );

        if let Some(location) = panic_info.location() {
            let mut extra = BTreeMap::new();
            extra.insert("file", Value::from(location.file()));

            if let Ok(line) = i32::try_from(location.line()) {
                extra.insert("line", line.into());
            }

            if let Ok(column) = i32::try_from(location.column()) {
                extra.insert("column", column.into());
            }

            event.insert("extra", extra);
        }

        event.add_stacktrace(0);

        if let Some(before_send) = &before_send {
            event = before_send(event);
        }

        event.capture();

        if let Some(hook) = &hook {
            hook(panic_info);
        }
    }));
}

#[cfg(test)]
#[rusty_fork::fork_test(timeout_ms = 60000)]
fn hook() {
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        thread,
    };

    static BEFORE_SEND: AtomicBool = AtomicBool::new(false);
    static HOOK: AtomicBool = AtomicBool::new(false);

    set_hook(None, None);
    set_hook(
        Some(Box::new(|event| {
            BEFORE_SEND.store(true, Ordering::SeqCst);
            event
        })),
        Some(Box::new(|_| HOOK.store(true, Ordering::SeqCst))),
    );

    thread::spawn(|| panic!("this panic is a test"))
        .join()
        .unwrap_err();

    assert!(BEFORE_SEND.load(Ordering::SeqCst));
    assert!(HOOK.load(Ordering::SeqCst));
}
