//! Sentry object implementation, represents common functionality between
//! [`Map`], [`Breadcrumb`], [`Event`], and [`User`].

#[cfg(doc)]
use crate::{Breadcrumb, Event, User};
use crate::{RToC, Value};
use std::collections::BTreeMap;

/// Implementation details of [`Object`].
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

            match unsafe { sys::value_set_by_key(raw, key.as_ptr(), value.into_raw()) } {
                0 => (),
                _ => panic!("Sentry failed to allocate memory"),
            }
        }

        raw
    }
}

/// Convenience trait to simplify passing a [`Value::Map`].
///
/// # Examples
/// ```
/// # use sentry_contrib_native::Map;
/// # use std::collections::BTreeMap;
/// fn accepts_map<M: Map>(map: M) {}
///
/// accepts_map(vec![("test", "test")]);
///
/// let mut map = BTreeMap::new();
/// map.insert("test", "test");
/// accepts_map(map);
/// ```
pub trait Map: Object {}

impl<K: Into<String>, V: Into<Value>> Map for Vec<(K, V)> {}
impl<K: Into<String>, V: Into<Value>> Object for Vec<(K, V)> {
    fn into_parts(self) -> (sys::Value, BTreeMap<String, Value>) {
        let map = self
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();

        (unsafe { sys::value_new_object() }, map)
    }
}

impl<K: Into<String>, V: Into<Value>> Map for BTreeMap<K, V> {}
impl<K: Into<String>, V: Into<Value>> Object for BTreeMap<K, V> {
    fn into_parts(self) -> (sys::Value, BTreeMap<String, Value>) {
        let map = self
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();

        (unsafe { sys::value_new_object() }, map)
    }
}
