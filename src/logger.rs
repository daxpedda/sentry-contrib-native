//! Implementation details for [`Options::set_logger`].

#[cfg(doc)]
use crate::Options;
use crate::{ffi, Level};
use once_cell::sync::Lazy;
#[cfg(doc)]
use std::process::abort;
use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    os::raw::{c_char, c_void},
    process,
    sync::RwLock,
};

/// Closure type for [`Options::set_logger`].
type Logger = dyn Fn(Level, Message) + 'static + Send + Sync;

/// Store [`Options::set_logger`] data to properly deallocate later.
pub static LOGGER: Lazy<RwLock<Option<Box<Logger>>>> = Lazy::new(|| RwLock::new(None));

/// Message received for custom logger.
#[derive(Clone, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub enum Message {
    /// Message could be parsed into a valid UTF-8 [`String`].
    Utf8(String),
    /// Message could not be parsed into a valid UTF-8 [`String`] and is
    /// stored as a `Vec<u8>`.
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

/// Function to pass to [`sys::options_set_logger`], which in turn calls the
/// user defined one.
///
/// This function will catch any unwinding panics and [`abort`] if any occured.
pub extern "C" fn logger(
    level: i32,
    message: *const c_char,
    args: *mut c_void,
    _userdata: *mut c_void,
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

    ffi::catch(|| logger(level, message))
}

#[cfg(test)]
#[rusty_fork::test_fork(timeout_ms = 60000)]
fn logger_test() -> anyhow::Result<()> {
    use crate::Options;
    use std::cell::RefCell;

    thread_local! {
        static LOGGED: RefCell<bool> = RefCell::new(false);
    }

    let mut options = Options::new();
    options.set_debug(true);
    options.set_logger(|level, message| {
        LOGGED.with(|logged| *logged.borrow_mut() = true);
        println!("[{}]: {}", level, message);
    });
    options.init()?;

    Ok(())
}

#[cfg(test)]
#[rusty_fork::test_fork(timeout_ms = 60000)]
#[should_panic]
fn catch_panic() -> anyhow::Result<()> {
    use crate::Options;

    let mut options = Options::new();
    options.set_debug(true);
    options.set_logger(|_, _| panic!("this is a test"));
    options.init()?;

    Ok(())
}
