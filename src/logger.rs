//! Implementation details for
//! [`Options::set_logger`](crate::Options::set_logger).

use crate::{ffi, Level};
use once_cell::sync::Lazy;
use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    os::raw::{c_char, c_void},
    process,
    sync::RwLock,
};

/// Closure type for [`Options::set_logger`](crate::Options::set_logger).
type Logger = dyn Fn(Level, Message) + 'static + Send + Sync;

/// Globally stored closure for
/// [`Options::set_logger`](crate::Options::set_logger).
pub static LOGGER: Lazy<RwLock<Option<Box<Logger>>>> = Lazy::new(|| RwLock::new(None));

/// Message received for custom logger.
#[derive(Clone, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub enum Message {
    /// Message could be parsed into a valid UTF-8 [`String`].
    Utf8(String),
    /// Message could not be parsed into a valid UTF-8 [`String`] and is
    /// returned as a `Vec<u8>`.
    Raw(Vec<u8>),
}

impl Display for Message {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Utf8(text) => write!(f, "{}", text),
            Self::Raw(raw) => write!(f, "{}", String::from_utf8_lossy(raw)),
        }
    }
}

/// Function to give [`Options::set_logger`](crate::Options::set_logger) which
/// in turn calls user defined one.
#[allow(clippy::module_name_repetitions)]
pub extern "C" fn sentry_contrib_native_logger(
    level: i32,
    message: *const c_char,
    args: *mut c_void,
) {
    let lock = LOGGER.read();
    let logger = lock
        .as_ref()
        .ok()
        .and_then(|logger| logger.as_deref())
        .unwrap_or_else(|| process::abort());

    let level = ffi::catch(|| Level::from_raw(level));
    let message = if let Ok(message) = unsafe { vsprintf::vsprintf(message, args) } {
        Message::Utf8(message)
    } else {
        Message::Raw(
            unsafe { vsprintf::vsprintf_raw(message, args) }.unwrap_or_else(|_| process::abort()),
        )
    };

    ffi::catch(|| logger(level, message));
}

#[cfg(test)]
#[rusty_fork::test_fork]
fn logger() -> anyhow::Result<()> {
    use crate::Options;

    let mut options = Options::new();
    options.set_debug(true);
    options.set_logger(|level, message| {
        println!("[{}]: {}", level, message);
    });
    options.init()?;

    Ok(())
}
