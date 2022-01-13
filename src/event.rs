//! Sentry event implementation.

use crate::{CToR, Level, Map, Object, RToC, Value};
use std::{
    cmp::Ordering,
    collections::BTreeMap,
    ffi::CStr,
    fmt::{Display, Formatter, Result},
    hash::{Hash, Hasher},
    mem,
    ops::{Deref, DerefMut},
    ptr, slice,
};

/// A Sentry event.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::Event;
/// let mut event = Event::new();
/// event.insert("extra", vec![("data", "test data")]);
/// event.capture();
/// ```
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct Event {
    /// Event interface.
    pub interface: Interface,
    /// Event content.
    pub map: BTreeMap<String, Value>,
}

/// Sentry event interface.
#[derive(Clone, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub enum Interface {
    /// Plain interface.
    Event,
    /// Message interface.
    Message {
        /// Level.
        level: Level,
        /// Logger.
        logger: Option<String>,
        /// Message text.
        text: String,
    },
}

impl Default for Event {
    fn default() -> Self {
        Self::new()
    }
}

impl Object for Event {
    fn into_parts(self) -> (sys::Value, BTreeMap<String, Value>) {
        let event = match self.interface {
            Interface::Event => unsafe { sys::value_new_event() },
            Interface::Message {
                level,
                logger,
                text,
            } => {
                let logger = logger.map(RToC::into_cstring);
                let logger = logger.as_deref().map_or(ptr::null(), CStr::as_ptr);
                let text = text.into_cstring();

                unsafe { sys::value_new_message_event(level.into_raw(), logger, text.as_ptr()) }
            }
        };

        (event, self.map)
    }
}

impl Deref for Event {
    type Target = BTreeMap<String, Value>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl DerefMut for Event {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

impl Event {
    /// Creates a new Sentry event.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Event;
    /// let mut event = Event::new();
    /// ```
    #[must_use = "`Event` doesn't do anything without `Event::capture`"]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new() -> Self {
        Self {
            interface: Interface::Event,
            map: BTreeMap::new(),
        }
    }

    /// Creates a new Sentry message event.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Level};
    /// let mut event = Event::new_message(Level::Debug, Some("test logger".into()), "test");
    /// ```
    pub fn new_message<S: Into<String>>(level: Level, logger: Option<String>, text: S) -> Self {
        Self {
            interface: Interface::Message {
                level,
                logger,
                text: text.into(),
            },
            map: BTreeMap::new(),
        }
    }

    /// Inserts a key-value pair into the [`Event`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Event;
    /// let mut event = Event::new();
    /// event.insert("extra", vec![("data", "test data")]);
    /// ```
    pub fn insert<S: Into<String>, V: Into<Value>>(&mut self, key: S, value: V) {
        self.deref_mut().insert(key.into(), value.into());
    }

    /// Generate stacktrace.
    fn stacktrace(len: usize) -> BTreeMap<String, Value> {
        let event = unsafe {
            let value = sys::value_new_event();
            sys::event_value_add_stacktrace(value, ptr::null_mut(), len);
            Value::from_raw(value)
        };

        event
            .into_map()
            .ok()
            .and_then(|mut event| event.remove("threads"))
            .and_then(|threads| threads.into_map().ok())
            .expect("failed to get stacktrace")
    }

    /// Adds a stacktrace with `len` instruction pointers to the [`Event`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Level};
    /// let mut event = Event::new_message(Level::Debug, Some("test logger".into()), "test");
    /// event.add_stacktrace(0);
    /// event.capture();
    /// ```
    pub fn add_stacktrace(&mut self, len: usize) {
        self.insert("threads", Self::stacktrace(len));
    }

    /// Adds an exception to the [`Event`] along with a stacktrace with `len`
    /// instruction pointers. As a workaround for <https://github.com/getsentry/sentry-native/issues/235>,
    /// the stacktrace is moved to the `exception` object so that it shows up
    /// correctly in Sentry.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Event;
    /// let mut event = Event::new();
    /// event.add_exception(
    ///     vec![
    ///         ("type", "test exception"),
    ///         ("value", "test exception value"),
    ///     ],
    ///     0,
    /// );
    /// event.capture();
    /// ```
    pub fn add_exception<M: Map + Into<Value>>(&mut self, exception: M, len: usize) {
        let stacktrace = Self::stacktrace(len)
            .remove("values")
            .and_then(|values| values.into_list().ok())
            .and_then(|values| values.into_iter().next())
            .and_then(|thread| thread.into_map().ok())
            .and_then(|mut thread| thread.remove("stacktrace"))
            .filter(Value::is_map)
            .expect("failed to move stacktrace");

        let mut exception = exception
            .into()
            .into_map()
            .expect("`Map` isn't `Value::Map`");
        exception.insert("stacktrace".into(), stacktrace);
        self.insert("exception", exception);
    }

    /// Sends the [`Event`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Event;
    /// # use std::collections::BTreeMap;
    /// let mut event = Event::new();
    /// event.insert("extra", vec![("data", "test data")]);
    /// event.capture();
    /// ```
    #[allow(clippy::must_use_candidate)]
    pub fn capture(self) -> Uuid {
        let event = self.into_raw();
        Uuid(unsafe { sys::capture_event(event) })
    }
}

/// A Sentry UUID.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::Event;
/// let uuid = Event::new().capture();
/// println!("event sent has UUID: \"{}\"", uuid);
/// println!(
///     "event sent has Sentry service compatible UUID: \"{}\"",
///     uuid.to_plain()
/// );
/// ```
#[derive(Debug, Copy, Clone)]
pub struct Uuid(sys::Uuid);

impl Default for Uuid {
    fn default() -> Self {
        Self(unsafe { sys::uuid_nil() })
    }
}

impl Display for Uuid {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let mut string = [0; 37];

        unsafe { sys::uuid_as_string(&self.0, string.as_mut_ptr()) };

        write!(
            f,
            "{}",
            unsafe { string.as_ptr().as_str() }.expect("invalid pointer")
        )
    }
}

impl PartialEq for Uuid {
    fn eq(&self, other: &Self) -> bool {
        self.into_bytes() == other.into_bytes()
    }
}

impl Eq for Uuid {}

impl PartialOrd for Uuid {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.into_bytes().partial_cmp(&other.into_bytes())
    }
}

impl Ord for Uuid {
    fn cmp(&self, other: &Self) -> Ordering {
        self.into_bytes().cmp(&other.into_bytes())
    }
}

impl Hash for Uuid {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.into_bytes().hash(state);
    }
}

impl Uuid {
    /// Creates a new empty UUID with the given `bytes`.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Uuid;
    /// let uuid = Uuid::from_bytes([0; 16]);
    /// ```
    #[must_use]
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(sys::Uuid {
            bytes: unsafe { mem::transmute(bytes) },
        })
    }

    /// Returns the bytes of the [`Uuid`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Uuid};
    /// assert_eq!([0; 16], Event::new().capture().into_bytes());
    /// ```
    #[must_use]
    pub fn into_bytes(self) -> [u8; 16] {
        unsafe { mem::transmute(self.0.bytes) }
    }

    /// Yield the bytes of the [`Uuid`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Uuid};
    /// assert_eq!([0; 16], Event::new().capture().as_bytes());
    /// ```
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.0.bytes.as_ptr().cast(), self.0.bytes.len()) }
    }

    /// Yield the UUID without dashes.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Uuid};
    /// assert_eq!(
    ///     "00000000000000000000000000000000",
    ///     Event::new().capture().to_plain()
    /// );
    /// ```
    #[must_use]
    pub fn to_plain(self) -> String {
        let mut uuid = self.to_string();
        uuid.retain(|c| c != '-');
        uuid
    }
}

impl AsRef<[u8]> for Uuid {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl From<[u8; 16]> for Uuid {
    fn from(value: [u8; 16]) -> Self {
        Self::from_bytes(value)
    }
}

impl From<Uuid> for [u8; 16] {
    fn from(value: Uuid) -> Self {
        value.into_bytes()
    }
}

#[test]
fn event() {
    let event = Event::new();

    if let Interface::Message { .. } = event.interface {
        unreachable!("event is incorrectly a message");
    }

    event.capture();

    let event = Event::new_message(Level::Debug, Some("test".into()), "test");

    if let Interface::Message {
        level,
        logger,
        text,
    } = &event.interface
    {
        assert_eq!(&Level::Debug, level);
        assert_eq!(&Some("test".into()), logger);
        assert_eq!("test", text);
    } else {
        unreachable!("event is incorrectly plain");
    }

    event.capture();

    let mut event = Event::new();
    event.add_stacktrace(0);
    assert!(event.get("threads").is_some());
    event.capture();

    let mut event = Event::new_message(Level::Debug, None, "test");
    event.add_stacktrace(0);
    assert!(event.get("threads").is_some());
    event.capture();

    let mut event = Event::new();
    event.insert("extra", vec![("data", "test data")]);
    event.capture();

    let mut event = Event::new();
    event.add_exception(vec![("type", "test type"), ("value", "test value")], 0);

    let exception = event.get("exception").unwrap().as_map().unwrap();
    assert_eq!(Some("test type"), exception.get("type").unwrap().as_str());
    assert_eq!(Some("test value"), exception.get("value").unwrap().as_str());
    let stacktrace = exception.get("stacktrace").unwrap().as_map().unwrap();
    let frames = stacktrace.get("frames").unwrap().as_list().unwrap();
    assert_ne!(None, frames.get(0).unwrap().as_map());

    event.capture();
}

#[test]
fn uuid() {
    assert_eq!(Uuid::default(), Uuid::default());

    assert_eq!(
        "00000000-0000-0000-0000-000000000000",
        Uuid::default().to_string()
    );

    assert_eq!(
        "00000000000000000000000000000000",
        Uuid::default().to_plain()
    );

    assert_eq!(
        "00000000-0000-0000-0000-000000000000",
        Uuid::default().to_string()
    );

    assert_eq!(
        Uuid::default(),
        Uuid::from_bytes(Uuid::default().into_bytes())
    );

    assert_eq!([0; 16], Uuid::default().into_bytes());

    assert_eq!([0; 16], Uuid::default().as_bytes());
}
