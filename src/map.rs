//! Sentry map implementation.

/// A Sentry map value.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Event, Map, Object};
/// # use std::iter::FromIterator;
/// let mut event = Event::new();
///
/// let mut map = Map::new();
/// map.insert("test", true);
///
/// event.insert("extra", map);
/// event.capture();
/// ```
pub struct Map(Option<sys::Value>);

impl Default for Map {
    fn default() -> Self {
        Self::new()
    }
}

derive_object!(Map);

impl Map {
    /// Creates a new Sentry map.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Map;
    /// let mut map = Map::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self(Some(unsafe { sys::value_new_object() }))
    }

    /// Creates a [`Map`] from [`sys::Value`].
    ///
    /// # Safety
    /// This doesn't check if [`sys::Value`] really is a [`Map`].
    pub(crate) const unsafe fn from_raw(value: sys::Value) -> Self {
        Self(Some(value))
    }
}
