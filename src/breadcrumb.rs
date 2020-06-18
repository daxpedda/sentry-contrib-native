//! Sentry breadcrumb implementation.

use crate::{global_write, RToC, Sealed};
use std::ptr;

/// A Sentry breadcrumb.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Breadcrumb, Map, Object};
/// # use std::iter::FromIterator;
/// let mut breadcrumb = Breadcrumb::new(None, Some("test message".into()));
/// let data = Map::from_iter(&[("some extra data", "test data")]);
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
    ///
    /// # Panics
    /// Panics if `type` or `message` contain any null bytes.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Breadcrumb;
    /// # use std::iter::FromIterator;
    /// Breadcrumb::new(None, Some("test message".into())).add();
    /// ```
    #[must_use]
    pub fn new(r#type: Option<String>, message: Option<String>) -> Self {
        let ty = r#type.map(RToC::into_cstring);
        let ty = ty.as_ref().map_or(ptr::null(), |ty| ty.as_ptr());
        let message = message.map(RToC::into_cstring);
        let message = message
            .as_ref()
            .map_or(ptr::null(), |message| message.as_ptr());

        Self(Some(unsafe { sys::value_new_breadcrumb(ty, message) }))
    }

    /// Adds the [`Breadcrumb`] to be sent in case of an
    /// [`Event::capture`](crate::Event::capture).
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Breadcrumb;
    /// # use std::iter::FromIterator;
    /// Breadcrumb::new(None, Some("test message".into())).add();
    /// ```
    pub fn add(self) {
        let breadcrumb = self.take();

        {
            let _lock = global_write();
            unsafe {
                sys::add_breadcrumb(breadcrumb);
            }
        }
    }
}

#[test]
fn breadcrumb() {
    use crate::Object;

    Breadcrumb::new(Some("test".into()), Some("test".into())).add();

    let breadcrumb = Breadcrumb::new(None, None);
    assert_eq!(None, breadcrumb.get("type"));
    assert_eq!(None, breadcrumb.get("message"));
    breadcrumb.add();

    let breadcrumb = Breadcrumb::new(Some("test".into()), None);
    assert_eq!(Some("test"), breadcrumb.get("type").unwrap().as_str());
    assert_eq!(None, breadcrumb.get("message"));
    breadcrumb.add();

    let breadcrumb = Breadcrumb::new(None, Some("test".into()));
    assert_eq!(None, breadcrumb.get("type"));
    assert_eq!(Some("test"), breadcrumb.get("message").unwrap().as_str());
    breadcrumb.add();

    let breadcrumb = Breadcrumb::new(Some("test".into()), Some("test".into()));
    assert_eq!(Some("test"), breadcrumb.get("type").unwrap().as_str());
    assert_eq!(Some("test"), breadcrumb.get("message").unwrap().as_str());
    breadcrumb.add();
}
