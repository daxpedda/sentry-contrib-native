use crate::{Level, Sealed, SentryString, GLOBAL_LOCK};
use std::{ffi::CString, os::raw::c_char, ptr};

/// A sentry event.
pub struct Event(Option<sys::Value>);

object_drop!(Event);

impl Default for Event {
    fn default() -> Self {
        Self::new()
    }
}

object_sealed!(Event);
object_debug!(Event);
object_clone!(Event);
object_partial_eq!(Event);
object_from_iterator!(Event);
object_extend!(Event);

impl Event {
    /// Creates a new empty event value.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{Event, Object};
    /// # fn main() -> anyhow::Result<()> {
    /// let event = Event::new();
    /// event.insert("test", true);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self(Some(unsafe { sys::value_new_event() }))
    }

    /// Creates a new [`Event`] containing a logger.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{Event, Level};
    /// let event = Event::new_message(Level::Debug, Some("logger".into()), "test");
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

    /// Adds a stacktrace to an event.
    ///
    /// `len` stacktrace instruction pointers are attached to the event.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::Event;
    /// let event = Event::new();
    /// event.value_add_stacktrace(12);
    /// event.capture();
    /// ```
    pub fn value_add_stacktrace(&self, len: usize) {
        let event = self.unwrap();

        unsafe { sys::event_value_add_stacktrace(event, ptr::null_mut(), len) };
    }

    /// Sends a sentry event.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{Event, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let event = Event::new();
    /// event.insert("test", true);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    #[allow(clippy::must_use_candidate)]
    pub fn capture(self) -> Uuid {
        let event = self.take();

        {
            let _lock = GLOBAL_LOCK.read().expect("global lock poisoned");
            Uuid(unsafe { sys::capture_event(event) })
        }
    }
}

/// A sentry UUID.
#[derive(Debug, Copy, Clone)]
pub struct Uuid(pub(crate) sys::Uuid);

impl ToString for Uuid {
    fn to_string(&self) -> String {
        let str = CString::new([0; 37].to_vec()).unwrap().into_raw();

        unsafe { sys::uuid_as_string(&self.0, str) };

        unsafe { CString::from_raw(str) }
            .to_str()
            .unwrap()
            .to_owned()
    }
}

impl Uuid {
    /// Returns the bytes of the uuid.
    #[must_use]
    pub const fn as_bytes(&self) -> [c_char; 16] {
        self.0.bytes
    }
}
