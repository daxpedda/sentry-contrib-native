//! Sentry event implementation.

use crate::{Level, Sealed, SentryString, GLOBAL_LOCK};
use std::{
    cmp::Ordering,
    ffi::{CStr, CString},
    fmt::{Display, Formatter, Result},
    hash::{Hash, Hasher},
    os::raw::c_char,
    ptr,
};

/// A Sentry event.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Event, Map, Object};
/// # use std::iter::FromIterator;
/// let mut event = Event::new();
/// let extra = Map::from_iter(vec![("some extra data", "test data")]);
/// event.insert("extra", extra);
/// event.capture();
/// ```
pub struct Event(Option<sys::Value>);

impl Default for Event {
    fn default() -> Self {
        Self::new()
    }
}

derive_object!(Event);

impl Event {
    /// Creates a new Sentry event.
    #[must_use]
    pub fn new() -> Self {
        Self(Some(unsafe { sys::value_new_event() }))
    }

    /// Creates a new Sentry message event.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Level, Map, Object};
    /// # use std::iter::FromIterator;
    /// let mut event = Event::new_message(Level::Debug, Some("test logger".into()), "test");
    /// let extra = Map::from_iter(vec![("some extra data", "test data")]);
    /// event.insert("extra", extra);
    /// event.capture();
    /// ```
    pub fn new_message<S: Into<SentryString>>(
        level: Level,
        logger: Option<SentryString>,
        text: S,
    ) -> Self {
        let logger = logger.map_or(ptr::null(), |logger| logger.as_cstr().as_ptr());
        let text: CString = text.into().into();

        Self(Some(unsafe {
            sys::value_new_message_event(level.into(), logger, text.as_ptr())
        }))
    }

    /// Adds a stacktrace to the [`Event`].
    pub fn value_add_stacktrace(&mut self, len: usize) {
        let event = self.as_raw();

        unsafe { sys::event_value_add_stacktrace(event, ptr::null_mut(), len) };
    }

    /// Sends the [`Event`].
    #[allow(clippy::must_use_candidate)]
    pub fn capture(self) -> Uuid {
        let event = self.take();

        {
            let _lock = GLOBAL_LOCK.read().expect("global lock poisoned");
            Uuid(unsafe { sys::capture_event(event) })
        }
    }
}

/// A Sentry UUID.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Event, Object};
/// let mut event = Event::new();
/// event.insert("test", true);
/// let uuid = event.capture();
/// println!("event sent has UUID {}", uuid);
/// ```
#[derive(Debug, Copy, Clone)]
pub struct Uuid(sys::Uuid);

impl PartialEq for Uuid {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Default for Uuid {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for Uuid {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let mut string = [0; 37];

        unsafe { sys::uuid_as_string(&self.0, string.as_mut_ptr()) };

        write!(
            f,
            "{}",
            unsafe { CStr::from_ptr(string.as_ptr()) }
                .to_str()
                .expect("UUID contained invalid UTF-8")
        )
    }
}

impl Eq for Uuid {}

impl PartialOrd for Uuid {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_bytes().partial_cmp(&other.as_bytes())
    }
}

impl Ord for Uuid {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_bytes().cmp(&other.as_bytes())
    }
}

impl Hash for Uuid {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state);
    }
}

impl Uuid {
    /// Creates a new empty Sentry UUID.
    #[must_use]
    pub fn new() -> Self {
        Self(unsafe { sys::uuid_nil() })
    }

    /// Creates a new empty UUID with the given `bytes`.
    #[must_use]
    pub const fn from_bytes(bytes: [c_char; 16]) -> Self {
        Self(sys::Uuid { bytes })
    }

    /// Returns the bytes of the [`Uuid`].
    #[must_use]
    pub const fn as_bytes(&self) -> [c_char; 16] {
        self.0.bytes
    }
}

impl From<[c_char; 16]> for Uuid {
    fn from(value: [c_char; 16]) -> Self {
        Self::from_bytes(value)
    }
}

#[test]
fn event() {
    Event::new().capture();
    Event::new_message(Level::Debug, None, "test").capture();
    Event::new_message(Level::Debug, Some("test".into()), "test").capture();

    let mut event = Event::new();
    event.value_add_stacktrace(0);
    event.capture();

    let mut event = Event::new_message(Level::Debug, None, "test");
    event.value_add_stacktrace(0);
    event.capture();
}

#[test]
fn uuid() {
    assert_eq!(
        "00000000-0000-0000-0000-000000000000",
        Uuid::new().to_string()
    );
}
