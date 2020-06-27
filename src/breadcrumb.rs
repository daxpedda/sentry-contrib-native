//! Sentry breadcrumb implementation.

#[cfg(doc)]
use crate::Event;
use crate::{global_write, Object, RToC, Value};
use std::{
    collections::BTreeMap,
    ffi::CStr,
    ops::{Deref, DerefMut},
    ptr,
};

/// A Sentry breadcrumb.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::Breadcrumb;
/// # use std::collections::BTreeMap;
/// let mut breadcrumb = Breadcrumb::new(None, Some("test message".into()));
/// let mut data = BTreeMap::new();
/// data.insert("some extra data".into(), "test data".into());
/// breadcrumb.insert("data".into(), data.into());
/// breadcrumb.add();
/// ```
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct Breadcrumb {
    /// Breadcrumb type.
    pub ty: Option<String>,
    /// Breadcrumb message.
    pub message: Option<String>,
    /// Breadcrumb content.
    pub map: BTreeMap<String, Value>,
}

impl Default for Breadcrumb {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl Object for Breadcrumb {
    fn into_parts(self) -> (sys::Value, BTreeMap<String, Value>) {
        let ty = self.ty.map(RToC::into_cstring);
        let ty = ty.as_deref().map_or(ptr::null(), CStr::as_ptr);
        let message = self.message.map(RToC::into_cstring);
        let message = message.as_deref().map_or(ptr::null(), CStr::as_ptr);

        (unsafe { sys::value_new_breadcrumb(ty, message) }, self.map)
    }
}

impl Deref for Breadcrumb {
    type Target = BTreeMap<String, Value>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl DerefMut for Breadcrumb {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

impl Breadcrumb {
    /// Creates a new Sentry breadcrumb.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Breadcrumb;
    /// let mut breadcrumb = Breadcrumb::new(None, Some("test message".into()));
    /// ```
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(ty: Option<String>, message: Option<String>) -> Self {
        Self {
            ty,
            message,
            map: BTreeMap::new(),
        }
    }

    /// Adds the [`Breadcrumb`] to be sent in case of an [`Event::capture`].
    ///
    /// # Panics
    /// Panics if any [`String`] in `self` contains a null byte.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Breadcrumb;
    /// Breadcrumb::new(None, Some("test message".into())).add();
    /// ```
    pub fn add(self) {
        let breadcrumb = self.into_raw();

        {
            let _lock = global_write();
            unsafe { sys::add_breadcrumb(breadcrumb) }
        }
    }
}

#[test]
fn breadcrumb() {
    let breadcrumb = Breadcrumb::new(Some("test".into()), Some("test".into()));
    assert_eq!(Some("test".into()), breadcrumb.ty);
    assert_eq!(Some("test".into()), breadcrumb.message);
    breadcrumb.add()
}
