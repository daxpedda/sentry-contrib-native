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

/// A simple [`Object`] implementation for [`Value::Map`].
pub struct Map(BTreeMap<String, Value>);

impl Map {
    /// Create a [`Value::Map`].
    pub fn new(value: BTreeMap<String, Value>) -> Self {
        Self(value)
    }
}

impl Object for Map {
    fn into_parts(self) -> (sys::Value, BTreeMap<String, Value>) {
        (unsafe { sys::value_new_object() }, self.0)
    }
}
