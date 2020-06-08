use crate::{Object, Sealed, SentryString, GLOBAL_LOCK};
use rmpv::Value as MpValue;
use std::ffi::CString;

/// A sentry breadcrumb.
pub struct Breadcrumb(Option<sys::Value>);

object_drop!(Breadcrumb);
object_sealed!(Breadcrumb);
object_debug!(Breadcrumb);

impl Clone for Breadcrumb {
    fn clone(&self) -> Self {
        let object = Self::new("placeholder", "placeholder");
        let map = self.to_msgpack();

        for (key, _) in map {
            let key = if let MpValue::String(key) = key {
                CString::new(key.into_bytes()).expect("message pack decoding failed")
            } else {
                unreachable!("message pack decoding failed")
            };

            object.insert(
                key.clone(),
                self.get(key.clone()).expect("message pack decoding failed"),
            );
        }

        object
    }
}

object_partial_eq!(Breadcrumb);
object_extend!(Breadcrumb);

impl Breadcrumb {
    /// Creates a new breadcrumb with a specific type and message.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{Breadcrumb, Object};
    /// let breadcrumb = Breadcrumb::new("test", "test");
    /// breadcrumb.insert("test", true);
    /// breadcrumb.add();
    /// ```
    pub fn new<S1: Into<SentryString>, S2: Into<SentryString>>(r#type: S1, message: S2) -> Self {
        let type_: CString = r#type.into().into();
        let message: CString = message.into().into();

        Self(Some(unsafe {
            sys::value_new_breadcrumb(type_.as_ptr(), message.as_ptr())
        }))
    }

    /// Adds the breadcrumb to be sent in case of an event.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{Breadcrumb, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let breadcrumb = Breadcrumb::new("test", "test");
    /// breadcrumb.insert("test", true);
    /// breadcrumb.add();
    /// # Ok(()) }
    /// ```
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
