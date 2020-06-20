//! Sentry object implementation, represents common functionality between
//! [`Map`](crate::Map), [`Breadcrumb`](crate::Breadcrumb),
//! [`Event`](crate::Event), and [`User`](crate::User).

use crate::{RToC, Value};
use std::collections::BTreeMap;

/// Private trait methods of [`Object`].
pub trait Object {
    /// Destructure [`Object`] into a raw [`sys::Value`] and a [`BTreeMap`] to
    /// add data to it.
    fn into_parts(self) -> (sys::Value, BTreeMap<String, Value>);

    /// Takes parts from [`Object::into_parts`] and stitches them together.
    fn into_raw(self) -> sys::Value
    where
        Self: Sized,
    {
        let (raw, map) = self.into_parts();

        for (key, value) in map {
            let key = key.into_cstring();
            let value = value;

            match unsafe { sys::value_set_by_key(raw, key.as_ptr(), value.into_raw()) } {
                0 => (),
                _ => panic!("Sentry failed to allocate memory"),
            }
        }

        raw
    }
}
