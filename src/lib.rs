#![warn(
    clippy::all,
    clippy::nursery,
    clippy::missing_docs_in_private_items,
    clippy::pedantic,
    missing_docs
)]
#![cfg_attr(
    feature = "nightly",
    feature(external_doc),
    doc(include = "../README.md")
)]
#![cfg_attr(not(feature = "nightly"), doc = "")]

#[macro_use]
mod object;
mod breadcrumb;
mod event;
mod ffi;
mod list;
mod map;
mod options;
mod panic;
mod user;
mod value;

pub use breadcrumb::Breadcrumb;
pub use event::{Event, Uuid};
use ffi::{CPath, CToR, RToC};
pub use list::List;
pub use map::Map;
pub use object::Object;
use object::Sealed;
use options::GLOBAL_LOCK;
pub use options::{Options, Shutdown};
pub use panic::set_hook;
use std::{convert::Infallible, os::raw::c_char, ptr};
use thiserror::Error;
pub use user::User;
pub use value::Value;

/// Sentry errors.
#[derive(Debug, Error, PartialEq)]
pub enum Error {
    /// Sample rate outside of allowed range.
    #[error("sample rate outside of allowed range")]
    SampleRateRange,
    /// Initializing Sentry failed.
    #[error("failed to initialize Sentry")]
    Init(Options),
    /// Failed to remove value from list by index.
    #[error("failed to remove value from list by index")]
    ListRemove,
    /// Failed to remove value from map.
    #[error("failed to remove value from map")]
    MapRemove,
    /// Failed to convert to type.
    #[error("failed to convert to type")]
    TryConvert(Value),
}

impl From<Infallible> for Error {
    fn from(from: Infallible) -> Self {
        match from {}
    }
}

/// Sentry levels for events and breadcrumbs.
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

impl From<Level> for i32 {
    fn from(level: Level) -> Self {
        match level {
            Level::Debug => sys::Level::Debug as _,
            Level::Info => sys::Level::Info as _,
            Level::Warning => sys::Level::Warning as _,
            Level::Error => sys::Level::Error as _,
            Level::Fatal => sys::Level::Fatal as _,
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

impl From<sys::UserConsent> for Consent {
    fn from(level: sys::UserConsent) -> Self {
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
///     // call forget because we are leaving the context and we don't want to shut down the Sentry client yet
///     options.init()?.forget();
///     Ok(())
/// }
/// ```
pub fn shutdown() {
    let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");
    unsafe { sys::shutdown() };
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
    unsafe { sys::clear_modulecache() };
}

/// Gives user consent.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{user_consent_give, Options};
/// # fn main() -> anyhow::Result<()> {
/// let mut options = Options::new();
/// options.set_require_user_consent(true);
/// let _shutdown = options.init()?;
///
/// user_consent_give();
/// # Ok(()) }
/// ```
pub fn user_consent_give() {
    unsafe { sys::user_consent_give() };
}

/// Revokes user consent.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{user_consent_revoke, Options};
/// # fn main() -> anyhow::Result<()> {
/// let mut options = Options::new();
/// options.set_require_user_consent(true);
/// let _shutdown = options.init()?;
///
/// user_consent_revoke();
/// # Ok(()) }
/// ```
pub fn user_consent_revoke() {
    unsafe { sys::user_consent_revoke() };
}

/// Resets the user consent (back to unknown).
///
/// # Examples
/// TODO
pub fn user_consent_reset() {
    unsafe { sys::user_consent_reset() };
}

/// Checks the current state of user consent.
/// TODO
#[must_use]
pub fn user_consent_get() -> Consent {
    unsafe { sys::user_consent_get() }.into()
}

/// Removes a user.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{remove_user, Options};
/// # fn main() -> anyhow::Result<()> {
/// let options = Options::new();
/// remove_user();
/// let _shutdown = options.init()?;
/// # Ok(()) }
/// ```
pub fn remove_user() {
    unsafe { sys::remove_user() };
}

/// Sets a tag.
///
/// # Panics
/// Panics if `key` or `value` contain any null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{set_tag, Event, Object, Value};
/// let mut event = Event::new();
/// event.insert("test", true);
/// set_tag("test_tag", "test");
/// event.capture();
/// ```
pub fn set_tag<S1: Into<String>, S2: Into<String>>(key: S1, value: S2) {
    let key = key.into().into_cstring();
    let value = value.into().into_cstring();

    {
        let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");
        unsafe { sys::set_tag(key.as_ptr(), value.as_ptr()) }
    }
}

/// Removes the tag with the specified key.
///
/// # Panics
/// Panics if `key` contains any null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{set_tag, remove_tag, Event, Object, Value};
/// # fn main() -> anyhow::Result<()> {
/// let mut event = Event::new();
/// event.insert("test", true);
/// set_tag("test_tag", "test");
/// remove_tag("test_tag");
/// event.capture();
/// # Ok(()) }
/// ```
pub fn remove_tag<S: Into<String>>(key: S) {
    let key = key.into().into_cstring();

    {
        let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");
        unsafe { sys::remove_tag(key.as_ptr()) }
    }
}

/// Sets extra information.
///
/// # Panics
/// - Panics if `key` contains any null bytes.
/// - Panics if `value` is a [`Value::String`] and contains null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{set_extra, Event, Object};
/// set_extra("ExtraTest", true);
/// let mut event = Event::new();
/// event.insert("test", true);
/// event.capture();
/// ```
pub fn set_extra<S: Into<String>, V: Into<Value>>(key: S, value: V) {
    let key = key.into().into_cstring();
    let value = value.into().take();

    {
        let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");
        unsafe { sys::set_extra(key.as_ptr(), value) };
    }
}

/// Removes the extra with the specified key.
///
/// # Panics
/// Panics if `key` contains any null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{set_extra, remove_extra, Event, Object};
/// set_extra("extra_test", true);
/// remove_extra("extra_test");
/// let mut event = Event::new();
/// event.insert("test", true);
/// event.capture();
/// ```
pub fn remove_extra<S: Into<String>>(key: S) {
    let key = key.into().into_cstring();

    {
        let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");
        unsafe { sys::remove_extra(key.as_ptr()) };
    }
}

/// Sets a context object.
///
/// # Panics
/// - Panics if `key` contains any null bytes.
/// - Panics if `value` is a [`Value::String`] and contains null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Event, Object, set_context};
/// let mut event = Event::new();
/// event.insert("test", true);
/// set_context("test_context", true);
/// event.capture();
/// ```
pub fn set_context<S: Into<String>, V: Into<Value>>(key: S, value: V) {
    let key = key.into().into_cstring();
    let value = value.into().take();

    {
        let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");
        unsafe { sys::set_context(key.as_ptr(), value) }
    }
}

/// Removes the context object with the specified key.
///
/// # Panics
/// Panics if `key` contains any null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{set_context, remove_context, Event, Object};
/// let mut event = Event::new();
/// event.insert("test", true);
/// set_context("test_context", true);
/// remove_context("test_context");
/// event.capture();
/// ```
pub fn remove_context<S: Into<String>>(key: S) {
    let key = key.into().into_cstring();

    {
        let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");
        unsafe { sys::remove_context(key.as_ptr()) };
    }
}

/// Sets the event fingerprint.
///
/// # Panics
/// Panics if [`String`]s in `fingerprints` contain any null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{set_fingerprint, Event, Object};
/// let mut event = Event::new();
/// event.insert("test", true);
/// set_fingerprint(vec!["test"]);
/// event.capture();
/// ```
pub fn set_fingerprint<I: IntoIterator<Item = S>, S: Into<String>>(fingerprints: I) {
    let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");

    for fingerprint in fingerprints {
        let fingerprint = fingerprint.into().into_cstring();
        unsafe { sys::set_fingerprint(fingerprint.as_ptr(), ptr::null::<c_char>()) };
    }
}

/// Removes the fingerprint.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{set_fingerprint, remove_fingerprint, Event, Object};
/// let mut event = Event::new();
/// event.insert("test", true);
/// set_fingerprint(vec!["test"]);
/// remove_fingerprint();
/// event.capture();
/// ```
pub fn remove_fingerprint() {
    let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");
    unsafe { sys::remove_fingerprint() };
}

/// Sets the transaction.
///
/// # Panics
/// Panics if `transaction` contains any null bytes.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{set_transaction, Event, Object};
/// let mut event = Event::new();
/// event.insert("test", true);
/// set_transaction("test_transaction");
/// event.capture();
/// ```
pub fn set_transaction<S: Into<String>>(transaction: S) {
    let transaction = transaction.into().into_cstring();

    {
        let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");
        unsafe { sys::set_transaction(transaction.as_ptr()) };
    }
}

/// Removes the transaction.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{remove_transaction, Event, Object};
/// remove_transaction();
///
/// let mut event = Event::new();
/// event.insert("test", true);
/// event.capture();
/// ```
pub fn remove_transaction() {
    unsafe { sys::remove_transaction() };
}

/// Sets the event level.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{set_level, Event, Object, Value, Level};
/// # fn main() -> anyhow::Result<()> {
/// set_level(Level::Debug);
/// let mut event = Event::new();
/// event.insert("test", true);
/// event.capture();
/// # Ok(()) }
/// ```
pub fn set_level(level: Level) {
    let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");
    unsafe { sys::set_level(level.into()) }
}

/// Starts a new session.
pub fn start_session() {
    let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");
    unsafe { sys::start_session() };
}

/// Ends a session.
pub fn end_session() {
    let _lock = GLOBAL_LOCK.read().expect("global lock poisoned");
    unsafe { sys::end_session() };
}
