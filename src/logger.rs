//! Implementation details for [`Options::set_logger`].

#[cfg(doc)]
use crate::Options;
use crate::{ffi, Level};
use once_cell::sync::Lazy;
#[cfg(doc)]
use std::process::abort;
use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    mem::ManuallyDrop,
    os::raw::{c_char, c_void},
    process,
    sync::Mutex,
};

/// How global [`Logger`] data is stored.
pub type Data = Box<Box<dyn Logger>>;

/// Store [`Options::set_logger`] data to properly deallocate later.
pub static LOGGER: Lazy<Mutex<Option<Data>>> = Lazy::new(|| Mutex::new(None));

/// Trait to help pass data to [`Options::set_logger`].
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Level, Logger, Message, Options};
/// # use std::sync::atomic::{AtomicUsize, Ordering};
/// # fn main() -> anyhow::Result<()> {
/// struct Log {
///     logged: AtomicUsize,
/// };
///
/// impl Logger for Log {
///     fn log(&self, level: Level, message: Message) {
///         self.logged.fetch_add(1, Ordering::SeqCst);
///         println!("[{}]: {}", level, message);
///     }
/// }
///
/// let mut options = Options::new();
/// options.set_logger(Log {
///     logged: AtomicUsize::new(0),
/// });
/// let _shutdown = options.init()?;
/// # Ok(()) }
/// ```
pub trait Logger: 'static + Send + Sync {
    /// Logger callback.
    ///
    /// # Notes
    /// The caller of this function will catch any unwinding panics and
    /// [`abort`] if any occured.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Level, Logger, Message};
    /// # use std::sync::atomic::{AtomicUsize, Ordering};
    /// struct Log {
    ///     logged: AtomicUsize,
    /// };
    ///
    /// impl Logger for Log {
    ///     fn log(&self, level: Level, message: Message) {
    ///         self.logged.fetch_add(1, Ordering::SeqCst);
    ///         println!("[{}]: {}", level, message);
    ///     }
    /// }
    /// ```
    fn log(&self, level: Level, message: Message);
}

impl<T: Fn(Level, Message) + 'static + Send + Sync> Logger for T {
    fn log(&self, level: Level, message: Message) {
        self(level, message);
    }
}

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
    userdata: *mut c_void,
) {
    let logger = userdata.cast::<Box<dyn Logger>>();
    let logger = ManuallyDrop::new(unsafe { Box::from_raw(logger) });

    let level = ffi::catch(|| Level::from_raw(level));

    let message = if let Ok(message) = unsafe { vsprintf::vsprintf(message, args) } {
        Message::Utf8(message)
    } else {
        Message::Raw(
            unsafe { vsprintf::vsprintf_raw(message, args) }.unwrap_or_else(|_| process::abort()),
        )
    };

    ffi::catch(|| logger.log(level, message));
}

#[cfg(test)]
#[rusty_fork::fork_test(timeout_ms = 60000)]
#[allow(clippy::items_after_statements)]
fn logger_test() -> anyhow::Result<()> {
    use crate::{Level, Logger, Message, Options};
    use std::{
        cell::RefCell,
        sync::atomic::{AtomicBool, Ordering},
    };

    thread_local! {
        static LOGGED: RefCell<bool> = RefCell::new(false);
    }

    struct Log {
        logged: AtomicBool,
    }

    impl Logger for Log {
        fn log(&self, level: Level, message: Message) {
            self.logged.store(true, Ordering::SeqCst);
            println!("[{}]: {}", level, message);
        }
    }

    impl Drop for Log {
        fn drop(&mut self) {
            LOGGED.with(|logged| *logged.borrow_mut() = *self.logged.get_mut());
        }
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
#[rusty_fork::fork_test(timeout_ms = 60000)]
#[should_panic]
fn catch_panic() -> anyhow::Result<()> {
    use crate::Options;

    let mut options = Options::new();
    options.set_debug(true);
    options.set_logger(|_, _| panic!("this is a test"));
    options.init()?;

    Ok(())
}
