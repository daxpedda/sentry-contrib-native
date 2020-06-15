//! Sentry breadcrumb implementation.

use crate::{Sealed, SentryString, GLOBAL_LOCK};
use std::{ffi::CString, ptr};

/// A Sentry breadcrumb.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Breadcrumb, Map, Object};
/// # use std::iter::FromIterator;
/// let mut breadcrumb = Breadcrumb::new(None, Some("test message".into()));
/// let data = Map::from_iter(vec![("some extra data", "test data")]);
/// breadcrumb.insert("data", data);
/// breadcrumb.add();
/// ```
pub struct Breadcrumb(Option<sys::Value>);

impl Default for Breadcrumb {
    fn default() -> Self {
        Self::new(None, None)
    }
}

derive_object!(Breadcrumb);

impl Breadcrumb {
    /// Creates a new Sentry breadcrumb.
    #[must_use]
    pub fn new(r#type: Option<SentryString>, message: Option<SentryString>) -> Self {
        let type_ = r#type.map_or(ptr::null(), |type_| CString::from(type_).as_ptr());
        let message = message.map_or(ptr::null(), |type_| CString::from(type_).as_ptr());

        Self(Some(unsafe { sys::value_new_breadcrumb(type_, message) }))
    }

    /// Adds the [`Breadcrumb`] to be sent in case of an [`Event::capture`].
    pub fn add(self) {
        let breadcrumb = self.take();

        {
            let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");
            unsafe {
                sys::add_breadcrumb(breadcrumb);
            }
        }
    }
}

#[test]
fn breadcrumb() {
    Breadcrumb::new(None, None).add();
    Breadcrumb::new(Some("test".into()), None).add();
    Breadcrumb::new(None, Some("test".into())).add();
    Breadcrumb::new(Some("test".into()), Some("test".into())).add();
}
