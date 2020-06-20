//! Sentry event implementation.

use crate::{global_read, CToR, Level, Map, Object, RToC, Sealed, Value};
use std::{
    cmp::Ordering,
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
/// let extra = Map::from_iter(&[("some extra data", "test data")]);
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
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Event;
    /// let mut event = Event::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self(Some(unsafe { sys::value_new_event() }))
    }

    /// Creates a new Sentry message event.
    ///
    /// # Panics
    /// Panics if `logger` or `text` contain any null bytes.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Level};
    /// let mut event = Event::new_message(Level::Debug, Some("test logger".into()), "test");
    /// ```
    pub fn new_message<S: Into<String>>(level: Level, logger: Option<String>, text: S) -> Self {
        let logger = logger.map(RToC::into_cstring);
        let logger = logger
            .as_ref()
            .map_or(ptr::null(), |logger| logger.as_ptr());
        let text = text.into().into_cstring();

        Self(Some(unsafe {
            sys::value_new_message_event(level.into(), logger, text.as_ptr())
        }))
    }

    /// Adds a stacktrace to the [`Event`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Level};
    /// # use std::iter::FromIterator;
    /// let mut event = Event::new_message(Level::Debug, Some("test logger".into()), "test");
    /// event.add_stacktrace(0);
    /// event.capture();
    /// ```
    pub fn add_stacktrace(&mut self, len: usize) {
        let event = self.as_raw();

        unsafe { sys::event_value_add_stacktrace(event, ptr::null_mut(), len) };
    }

    /// Adds an exception to the [`Event`] along with a stacktrace. As a
    /// workaround for <https://github.com/getsentry/sentry-native/issues/235>,
    /// the stacktrace is moved to the `exception` object so that it shows up
    /// correctly in Sentry.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Level, Map, Object};
    /// # use std::iter::FromIterator;
    /// let mut event = Event::new();
    /// let exception = Map::from_iter(&[
    ///     ("type", "test exception"),
    ///     ("value", "test exception value"),
    /// ]);
    /// event.add_exception(exception, 0);
    /// event.capture();
    /// ```
    pub fn add_exception(&mut self, mut exception: Map, len: usize) {
        self.add_stacktrace(len);

        if let Some(Value::Map(threads)) = self.get("threads") {
            if let Some(Value::List(threads_values)) = threads.get("values") {
                if let Some(Value::Map(thread)) = threads_values.get(0) {
                    if let Some(Value::Map(stacktrace)) = thread.get("stacktrace") {
                        exception.insert("stacktrace", stacktrace);

                        if self.remove("threads").is_ok() {
                            self.insert("exception", exception);
                            return;
                        }
                    }
                }
            }
        }

        panic!("failed to move stacktrace");
    }

    /// Sends the [`Event`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Map, Object};
    /// # use std::iter::FromIterator;
    /// let mut event = Event::new();
    /// let extra = Map::from_iter(&[("some extra data", "test data")]);
    /// event.insert("extra", extra);
    /// event.capture();
    /// ```
    #[allow(clippy::must_use_candidate)]
    pub fn capture(self) -> Uuid {
        let event = self.take();

        {
            let _lock = global_read();
            Uuid(unsafe { sys::capture_event(event) })
        }
    }
}

/// A Sentry UUID.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::Event;
/// let uuid = Event::new().capture();
/// println!("event sent has UUID \"{}\"", uuid);
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
            unsafe { string.as_ptr().as_str() }.expect("invalid pointer")
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
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Uuid;
    /// assert_eq!(
    ///     "00000000-0000-0000-0000-000000000000",
    ///     Uuid::new().to_string()
    /// );
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self(unsafe { sys::uuid_nil() })
    }

    /// Creates a new empty UUID with the given `bytes`.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Uuid;
    /// Uuid::from_bytes([0; 16]);
    /// ```
    #[must_use]
    pub const fn from_bytes(bytes: [c_char; 16]) -> Self {
        Self(sys::Uuid { bytes })
    }

    /// Returns the bytes of the [`Uuid`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Uuid;
    /// assert_eq!([0; 16], Uuid::new().as_bytes());
    /// ```
    #[must_use]
    pub const fn as_bytes(self) -> [c_char; 16] {
        self.0.bytes
    }
}

impl From<[c_char; 16]> for Uuid {
    fn from(value: [c_char; 16]) -> Self {
        Self::from_bytes(value)
    }
}

impl From<Uuid> for [c_char; 16] {
    fn from(value: Uuid) -> Self {
        value.as_bytes()
    }
}

#[test]
fn event() -> anyhow::Result<()> {
    use crate::List;
    use std::convert::TryInto;

    Event::new().capture();

    let event = Event::new_message(Level::Debug, None, "test");
    assert_eq!(Some("debug"), event.get("level").unwrap().as_str());
    assert_eq!(None, event.get("logger"));
    assert_eq!(
        Some("test"),
        event
            .get("message")
            .unwrap()
            .as_map()
            .unwrap()
            .get("formatted")
            .unwrap()
            .as_str()
    );
    event.capture();

    let event = Event::new_message(Level::Debug, Some("test".into()), "test");
    assert_eq!(Some("debug"), event.get("level").unwrap().as_str());
    assert_eq!(Some("test"), event.get("logger").unwrap().as_str());
    assert_eq!(
        Some("test"),
        event
            .get("message")
            .unwrap()
            .as_map()
            .unwrap()
            .get("formatted")
            .unwrap()
            .as_str()
    );
    event.capture();

    let mut event = Event::new();
    event.add_stacktrace(0);
    event.capture();

    let mut event = Event::new_message(Level::Debug, None, "test");
    event.add_stacktrace(0);
    event.capture();

    let mut event = Event::new();
    let mut exception = Map::new();
    exception.insert("type", "test type");
    exception.insert("value", "test value");
    event.add_exception(exception, 0);

    let exception: Map = event.get("exception").unwrap().try_into()?;
    assert_eq!(Some("test type"), exception.get("type").unwrap().as_str());
    assert_eq!(Some("test value"), exception.get("value").unwrap().as_str());
    let stacktrace: Map = exception.get("stacktrace").unwrap().try_into()?;
    let frames: List = stacktrace.get("frames").unwrap().try_into()?;
    assert_ne!(None, frames.get(0).unwrap().as_map());

    event.capture();

    Ok(())
}

#[test]
fn uuid() {
    assert_eq!(
        "00000000-0000-0000-0000-000000000000",
        Uuid::new().to_string()
    );
}
