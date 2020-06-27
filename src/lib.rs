#![warn(
    clippy::all,
    clippy::nursery,
    clippy::missing_docs_in_private_items,
    clippy::pedantic,
    missing_docs
)]
#![cfg_attr(
    feature = "nightly",
    feature(doc_cfg),
    feature(external_doc),
    doc(include = "../README.md")
)]
#![cfg_attr(not(feature = "nightly"), doc = "")]

mod before_send;
mod breadcrumb;
mod event;
mod ffi;
mod logger;
mod object;
mod options;
mod panic;
#[cfg(feature = "test")]
pub mod test;
mod transport;
mod user;
mod value;

pub use before_send::BeforeSend;
use before_send::{Data as BeforeSendData, BEFORE_SEND};
pub use breadcrumb::Breadcrumb;
pub use event::{Event, Interface, Uuid};
use ffi::{CPath, CToR, RToC};
#[cfg(feature = "custom-transport")]
pub use http;
pub use logger::Message;
use logger::LOGGER;
use object::{Map, Object};
use options::{global_read, global_write, Ownership};
pub use options::{Options, Shutdown};
pub use panic::set_hook;
use std::{
    convert::Infallible,
    fmt::{Display, Formatter, Result as FmtResult},
    os::raw::c_char,
    ptr,
};
use thiserror::Error;
#[cfg(feature = "custom-transport")]
pub use transport::{Dsn, Error as TransportError, Parts, Request};
pub use transport::{
    Envelope, RawEnvelope, Shutdown as TransportShutdown, Transport, API_VERSION, ENVELOPE_MIME,
    SDK_USER_AGENT,
};
pub use user::User;
pub use value::Value;

/// [`sentry-contrib-native`] errors.
#[derive(Debug, Error, PartialEq)]
pub enum Error {
    /// Sample rate outside of allowed range.
    #[error("sample rate outside of allowed range")]
    SampleRateRange,
    /// Failed to initialize Sentry.
    #[error("failed to initialize Sentry")]
    Init,
    /// Failed to remove value from list by index.
    #[error("failed to remove value from list by index")]
    ListRemove,
    /// Failed to remove value from map.
    #[error("failed to remove value from map")]
    MapRemove,
    /// Failed to convert to given type.
    #[error("failed to convert to given type")]
    TryConvert(Value),
    /// List of fingerprints is too long.
    #[error("list of fingerprints is too long")]
    Fingerprints,
    /// Failed at custom transport.
    #[cfg(feature = "custom-transport")]
    #[cfg_attr(feature = "nightly", doc(cfg(feature = "custom-transport")))]
    #[error("failed at custom transport")]
    Transport(#[from] TransportError),
}

impl From<Infallible> for Error {
    fn from(from: Infallible) -> Self {
        match from {}
    }
}

/// Sentry event level.
#[derive(Clone, Copy, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub enum Level {
    /// Debug.
    Debug,
    /// Info.
    Info,
    /// Warning.
    Warning,
    /// Error.
    Error,
    /// Fatal.
    Fatal,
}

impl Display for Level {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let text = match self {
            Self::Debug => "Debug",
            Self::Info => "Info",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Fatal => "Fatal",
        };
        write!(f, "{}", text)
    }
}

impl Level {
    /// Convert [`Level`] to [`i32`].
    fn into_raw(self) -> i32 {
        match self {
            Self::Debug => sys::Level::Debug as _,
            Self::Info => sys::Level::Info as _,
            Self::Warning => sys::Level::Warning as _,
            Self::Error => sys::Level::Error as _,
            Self::Fatal => sys::Level::Fatal as _,
        }
    }

    /// Convert [`i32`] to [`Level`].
    fn from_raw(level: i32) -> Self {
        match level {
            level if level == sys::Level::Debug as _ => Self::Debug,
            level if level == sys::Level::Info as _ => Self::Info,
            level if level == sys::Level::Warning as _ => Self::Warning,
            level if level == sys::Level::Error as _ => Self::Error,
            level if level == sys::Level::Fatal as _ => Self::Fatal,
            _ => unreachable!("failed to convert `i32` to `Level`"),
        }
    }
}

/// The state of user consent.
#[derive(Clone, Copy, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub enum Consent {
    /// Unknown.
    Unknown,
    /// Revoked.
    Revoked,
    /// Given.
    Given,
}

impl Consent {
    /// Convert [`sys::UserConsent`] to [`Consent`].
    fn from_raw(level: sys::UserConsent) -> Self {
        match level {
            sys::UserConsent::Unknown => Self::Unknown,
            sys::UserConsent::Revoked => Self::Revoked,
            sys::UserConsent::Given => Self::Given,
        }
    }
}

/// Shuts down the Sentry client and forces transports to flush out.
///
/// # Examples
/// ```
/// # use anyhow::Result;
/// # use sentry_contrib_native::{Options, shutdown};
/// fn main() -> Result<()> {
///     sentry_init()?;
///
///     // ...
///     // your application code
///     // ...
///
///     // call shutdown manually to make sure transports flush out
///     shutdown();
///     Ok(())
/// }
///
/// fn sentry_init() -> Result<()> {
///     let options = Options::new();
///     // `forget` to not automatically shutdown Sentry
///     options.init()?.forget();
///     Ok(())
/// }
/// ```
pub fn shutdown() {
    {
        let _lock = global_write();
        unsafe { sys::shutdown() };
    }

    // de-allocate `BEFORE_SEND`
    if let Some(before_send) = BEFORE_SEND.get() {
        before_send
            .lock()
            .expect("failed to deallocate `BEFORE_SEND`")
            .take();
    }

    // de-allocate `LOGGER`
    LOGGER
        .write()
        .expect("failed to deallocate `LOGGER`")
        .take();
}

/// Clears the internal module cache.
///
/// For performance reasons, Sentry will cache the list of loaded libraries when
/// capturing events. This cache can get out-of-date when loading or unloading
/// libraries at runtime. It is therefore recommended to call
/// [`clear_modulecache`] when doing so, to make sure that the next call to
/// [`Event::capture`] will have an up-to-date module list.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::clear_modulecache;
/// # mod libloading {
/// #     pub struct Library;
/// #     impl Library {
/// #         pub fn new(_: &str) -> anyhow::Result<()> {
/// #             Ok(())
/// #         }
/// #     }
/// # }
/// # fn main() -> anyhow::Result<()> {
/// libloading::Library::new("/path/to/liblibrary.so")?;
/// clear_modulecache();
/// # Ok(()) }
/// ```
pub fn clear_modulecache() {
    unsafe { sys::clear_modulecache() }
}

/// Gives user consent.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Consent, Options, user_consent_get, user_consent_give};
/// # fn main() -> anyhow::Result<()> {
/// let mut options = Options::new();
/// options.set_require_user_consent(true);
/// let _shutdown = options.init()?;
///
/// user_consent_give();
/// assert_eq!(Consent::Given, user_consent_get());
/// # Ok(()) }
/// ```
pub fn user_consent_give() {
    let _lock = global_read();
    unsafe { sys::user_consent_give() }
}

/// Revokes user consent.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Consent, Options, user_consent_get, user_consent_revoke};
/// # fn main() -> anyhow::Result<()> {
/// let mut options = Options::new();
/// options.set_require_user_consent(true);
/// let _shutdown = options.init()?;
///
/// user_consent_revoke();
/// assert_eq!(Consent::Revoked, user_consent_get());
/// # Ok(()) }
/// ```
pub fn user_consent_revoke() {
    let _lock = global_read();
    unsafe { sys::user_consent_revoke() }
}

/// Resets the user consent (back to unknown).
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Consent, Options, user_consent_get, user_consent_give, user_consent_reset};
/// # fn main() -> anyhow::Result<()> {
/// let mut options = Options::new();
/// options.set_require_user_consent(true);
/// let _shutdown = options.init()?;
///
/// user_consent_give();
/// assert_eq!(Consent::Given, user_consent_get());
/// user_consent_reset();
/// assert_eq!(Consent::Unknown, user_consent_get());
/// # Ok(()) }
/// ```
pub fn user_consent_reset() {
    let _lock = global_read();
    unsafe { sys::user_consent_reset() }
}

/// Checks the current state of user consent.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Consent, user_consent_get, user_consent_give, Options};
/// # fn main() -> anyhow::Result<()> {
/// let mut options = Options::new();
/// options.set_require_user_consent(true);
/// let _shutdown = options.init()?;
///
/// user_consent_give();
/// assert_eq!(Consent::Given, user_consent_get());
/// # Ok(()) }
/// ```
#[must_use]
pub fn user_consent_get() -> Consent {
    Consent::from_raw(unsafe { sys::user_consent_get() })
}

/// Removes a user.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Options, remove_user, User};
/// let mut user = User::new();
/// user.insert("id".into(), 1.into());
/// user.set();
///
/// remove_user();
/// ```
pub fn remove_user() {
    unsafe { sys::remove_user() }
}

/// Sets a tag.
///
/// # Panics
/// Panics if `key` or `value` contain any null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::set_tag;
/// set_tag("test tag", "test");
/// ```
pub fn set_tag<S1: Into<String>, S2: Into<String>>(key: S1, value: S2) {
    let key = key.into().into_cstring();
    let value = value.into().into_cstring();

    unsafe { sys::set_tag(key.as_ptr(), value.as_ptr()) }
}

/// Removes the tag with the specified `key`.
///
/// # Panics
/// Panics if `key` contains any null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{remove_tag, set_tag};
/// set_tag("test tag", "test");
/// remove_tag("test tag");
/// ```
pub fn remove_tag<S: Into<String>>(key: S) {
    let key = key.into().into_cstring();
    unsafe { sys::remove_tag(key.as_ptr()) }
}

/// Sets extra information.
///
/// # Panics
/// - Panics if `key` contains any null bytes.
/// - Panics if `value` is a [`Value::String`] and contains null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::set_extra;
/// set_extra("extra stuff", "stuff");
/// ```
pub fn set_extra<S: Into<String>, V: Into<Value>>(key: S, value: V) {
    let key = key.into().into_cstring();
    let value = value.into().into_raw();

    unsafe { sys::set_extra(key.as_ptr(), value) }
}

/// Removes the extra with the specified `key`.
///
/// # Panics
/// Panics if `key` contains any null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{remove_extra, set_extra};
/// set_extra("extra stuff", "stuff");
/// remove_extra("extra stuff");
/// ```
pub fn remove_extra<S: Into<String>>(key: S) {
    let key = key.into().into_cstring();
    unsafe { sys::remove_extra(key.as_ptr()) }
}

/// Sets a context object.
///
/// # Panics
/// - Panics if `key` contains any null bytes.
/// - Panics if `value` is a [`Value::String`] and contains null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::set_context;
/// set_context("test context", "context x");
/// ```
pub fn set_context<S: Into<String>, V: Into<Value>>(key: S, value: V) {
    let key = key.into().into_cstring();
    let value = value.into().into_raw();

    unsafe { sys::set_context(key.as_ptr(), value) }
}

/// Removes the context object with the specified key.
///
/// # Panics
/// Panics if `key` contains any null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{remove_context, set_context};
/// set_context("test context", "context x");
/// remove_context("test context");
/// ```
pub fn remove_context<S: Into<String>>(key: S) {
    let key = key.into().into_cstring();

    unsafe { sys::remove_context(key.as_ptr()) }
}

/// Sets the event fingerprint.
///
/// # Errors
/// Fails with [`Error::Fingerprints`] if `fingerprints` is longer than 32.
///
/// # Panics
/// Panics if [`String`]s in `fingerprints` contain any null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::set_fingerprint;
/// set_fingerprint(vec!["test"]);
/// ```
pub fn set_fingerprint<I: IntoIterator<Item = S>, S: Into<String>>(
    fingerprints: I,
) -> Result<(), Error> {
    let fingerprints: Vec<_> = fingerprints
        .into_iter()
        .map(Into::into)
        .map(RToC::into_cstring)
        .collect();

    if fingerprints.len() > 32 {
        Err(Error::Fingerprints)
    } else if fingerprints.is_empty() {
        Ok(())
    } else {
        let mut raw_fingerprints = [ptr::null(); 32];

        for (fingerprint, raw_fingerprint) in fingerprints.iter().zip(raw_fingerprints.iter_mut()) {
            *raw_fingerprint = fingerprint.as_ptr();
        }

        unsafe {
            sys::set_fingerprint(
                raw_fingerprints[0],
                raw_fingerprints[1],
                raw_fingerprints[2],
                raw_fingerprints[3],
                raw_fingerprints[4],
                raw_fingerprints[5],
                raw_fingerprints[6],
                raw_fingerprints[7],
                raw_fingerprints[8],
                raw_fingerprints[9],
                raw_fingerprints[10],
                raw_fingerprints[11],
                raw_fingerprints[12],
                raw_fingerprints[13],
                raw_fingerprints[14],
                raw_fingerprints[15],
                raw_fingerprints[16],
                raw_fingerprints[17],
                raw_fingerprints[18],
                raw_fingerprints[19],
                raw_fingerprints[20],
                raw_fingerprints[21],
                raw_fingerprints[22],
                raw_fingerprints[23],
                raw_fingerprints[24],
                raw_fingerprints[25],
                raw_fingerprints[26],
                raw_fingerprints[27],
                raw_fingerprints[28],
                raw_fingerprints[29],
                raw_fingerprints[30],
                raw_fingerprints[31],
                ptr::null::<c_char>(),
            )
        };

        Ok(())
    }
}

/// Removes the fingerprint.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{set_fingerprint, remove_fingerprint};
/// set_fingerprint(vec!["test"]);
/// remove_fingerprint();
/// ```
pub fn remove_fingerprint() {
    unsafe { sys::remove_fingerprint() }
}

/// Sets the transaction.
///
/// # Panics
/// Panics if `transaction` contains any null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::set_transaction;
/// set_transaction("test transaction");
/// ```
pub fn set_transaction<S: Into<String>>(transaction: S) {
    let transaction = transaction.into().into_cstring();
    unsafe { sys::set_transaction(transaction.as_ptr()) }
}

/// Removes the transaction.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{remove_transaction, set_transaction};
/// set_transaction("test transaction");
/// remove_transaction();
/// ```
pub fn remove_transaction() {
    unsafe { sys::remove_transaction() }
}

/// Sets the event level.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Level, set_level};
/// set_level(Level::Debug);
/// ```
pub fn set_level(level: Level) {
    unsafe { sys::set_level(level.into_raw()) }
}

/// Starts a new session.
///
/// # Examples
/// TODO
pub fn start_session() {
    let _lock = global_read();
    unsafe { sys::start_session() }
}

/// Ends a session.
///
/// # Examples
/// TODO
pub fn end_session() {
    let _lock = global_read();
    unsafe { sys::end_session() }
}

#[test]
fn error() -> Result<(), Error> {
    Ok::<_, Infallible>(())?;
    Ok(())
}

#[test]
fn level() {
    assert_eq!(-1, Level::Debug.into_raw());
    assert_eq!(0, Level::Info.into_raw());
    assert_eq!(1, Level::Warning.into_raw());
    assert_eq!(2, Level::Error.into_raw());
    assert_eq!(3, Level::Fatal.into_raw());

    assert_eq!(Level::Debug, Level::from_raw(-1));
    assert_eq!(Level::Info, Level::from_raw(0));
    assert_eq!(Level::Warning, Level::from_raw(1));
    assert_eq!(Level::Error, Level::from_raw(2));
    assert_eq!(Level::Fatal, Level::from_raw(3));
}

#[cfg(test)]
#[rusty_fork::test_fork(timeout_ms = 1000)]
fn consent() -> anyhow::Result<()> {
    assert_eq!(Consent::Unknown, crate::user_consent_get());

    crate::user_consent_give();
    assert_eq!(Consent::Unknown, crate::user_consent_get());

    crate::user_consent_revoke();
    assert_eq!(Consent::Unknown, crate::user_consent_get());

    crate::user_consent_reset();
    assert_eq!(Consent::Unknown, crate::user_consent_get());

    let _shutdown = Options::new().init()?;

    crate::user_consent_give();
    assert_eq!(Consent::Given, crate::user_consent_get());

    crate::user_consent_revoke();
    assert_eq!(Consent::Revoked, crate::user_consent_get());

    crate::user_consent_reset();
    assert_eq!(Consent::Unknown, crate::user_consent_get());

    Ok(())
}

#[test]
fn fingerprint() -> anyhow::Result<()> {
    for len in 1..33 {
        let mut fingerprints = Vec::with_capacity(len);

        for fingerprint in 0..len {
            fingerprints.push(fingerprint.to_string());
        }

        crate::set_fingerprint(fingerprints)?;
    }

    Ok(())
}

#[test]
#[should_panic]
fn fingerprint_invalid() {
    let mut fingerprints = Vec::with_capacity(33);

    for fingerprint in 0..33 {
        fingerprints.push(fingerprint.to_string());
    }

    crate::set_fingerprint(fingerprints).unwrap()
}

#[cfg(test)]
#[rusty_fork::test_fork(timeout_ms = 30000)]
fn threaded_stress() -> anyhow::Result<()> {
    use std::thread;

    fn spawns(tests: Vec<fn(i32)>) {
        let mut spawns = Vec::with_capacity(tests.len());
        for test in tests {
            let handle = thread::spawn(move || {
                let mut handles = Vec::with_capacity(100);

                for index in 0..100 {
                    handles.push(thread::spawn(move || test(index)))
                }

                handles
            });
            spawns.push(handle)
        }

        for spawn in spawns {
            for handle in spawn.join().unwrap() {
                handle.join().unwrap()
            }
        }
    }

    test::set_hook();

    let mut options = Options::new();
    options.set_require_user_consent(true);
    let shutdown = options.init()?;

    spawns(vec![
        |_| crate::clear_modulecache(),
        |_| crate::user_consent_give(),
        |_| crate::user_consent_revoke(),
        |_| crate::user_consent_reset(),
        |_| {
            let _ = crate::user_consent_get();
        },
        |index| {
            let mut user = User::new();
            user.insert("id".into(), index.into());
            user.set();
        },
        |_| crate::remove_user(),
        |index| crate::set_tag(index.to_string(), index.to_string()),
        |index| crate::remove_tag(index.to_string()),
        |index| crate::set_extra(index.to_string(), index),
        |index| crate::remove_extra(index.to_string()),
        |index| crate::set_context(index.to_string(), index),
        |index| crate::remove_context(index.to_string()),
        |index| crate::set_fingerprint(vec![index.to_string()]).unwrap(),
        |_| crate::remove_fingerprint(),
        |index| crate::set_transaction(index.to_string()),
        |_| crate::remove_transaction(),
        |index| {
            crate::set_level(match index % 5 {
                0 => Level::Debug,
                1 => Level::Info,
                2 => Level::Warning,
                3 => Level::Error,
                4 => Level::Fatal,
                _ => unreachable!(),
            })
        },
        |_| crate::start_session(),
        |_| crate::end_session(),
    ]);

    shutdown.shutdown();

    test::verify_panics();

    Ok(())
}
