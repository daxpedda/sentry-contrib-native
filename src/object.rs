use crate::{Error, List, Map, SentryString, Value};
use rmpv::{decode, Value as MpValue};
use std::{convert::TryInto, ffi::CString, iter::FromIterator, slice};

pub trait Sealed {
    fn as_raw(&self) -> sys::Value;

    fn take(self) -> sys::Value;

    fn to_msgpack(&self) -> Vec<(MpValue, MpValue)> {
        let object = self.as_raw();

        let mut size_out = 0;

        let msg_raw = unsafe { sys::value_to_msgpack(object, &mut size_out) };
        let mut msg = unsafe { slice::from_raw_parts(msg_raw as _, size_out) };
        let value = decode::read_value(&mut msg).expect("message pack decoding failed");
        unsafe { sys::free(msg_raw as _) };

        if let MpValue::Map(map) = value {
            map
        } else {
            unreachable!("message pack decoding failed")
        }
    }
}

/// Extention trait for types that function like [`Map`]s:
/// [`Breadcrumb`](crate::Breadcrumb), [`Event`](crate::Event), and
/// [`User`](crate::User).
pub trait Object: Sealed {
    /// Sets a value to a key in the map.
    ///
    /// # Panics
    /// Panics if Sentry failed to allocate memory.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Map, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut event = Event::new();
    /// let mut object = Map::new();
    /// object.insert("test", true);
    /// assert_eq!(Value::Bool(true), object.get("test").unwrap());
    /// event.insert("test", object);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    fn insert<S: Into<SentryString>, V: Into<Value>>(&mut self, key: S, value: V) {
        let object = self.as_raw();

        let key: CString = key.into().into();
        let value = value.into();

        match unsafe { sys::value_set_by_key(object, key.as_ptr(), value.take()) } {
            0 => (),
            _ => panic!("Sentry failed to allocate memory"),
        }
    }

    /// This removes a value from the map by key.
    ///
    /// # Errors
    /// Fails with [`Error::MapRemove`] if index wasn't found.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Map, Object};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut event = Event::new();
    /// let mut object = Map::new();
    /// object.insert("test", true);
    /// object.remove("test")?;
    /// assert_eq!(None, object.get("test"));
    /// event.insert("test", object);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    fn remove<S: Into<SentryString>>(&mut self, key: S) -> Result<(), Error> {
        let object = self.as_raw();

        let key: CString = key.into().into();

        match unsafe { sys::value_remove_by_key(object, key.as_ptr()) } {
            0 => Ok(()),
            _ => Err(Error::MapRemove),
        }
    }

    /// Looks up a value in a map by key.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Map, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut event = Event::new();
    /// let mut object = Map::new();
    /// object.insert("test", true);
    /// assert_eq!(Value::Bool(true), object.get("test").unwrap());
    /// event.insert("test", object);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    fn get<S: Into<SentryString>>(&self, key: S) -> Option<Value> {
        let object = self.as_raw();

        let key: CString = key.into().into();

        match Value::from_raw(unsafe { sys::value_get_by_key_owned(object, key.as_ptr()) }) {
            Value::Null => None,
            value => Some(value),
        }
    }

    /// Returns the length of the given map or list.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Map, Object};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut event = Event::new();
    /// let mut object = Map::new();
    /// object.insert("test", true);
    /// assert_eq!(1, object.len());
    /// event.insert("test", object);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    #[must_use]
    fn len(&self) -> usize {
        let object = self.as_raw();

        unsafe { sys::value_get_length(object) }
    }

    /// Returns `true` if the [`Object`] has a length of 0.
    #[must_use]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Turns a [`Map`] to a [`Vec`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Map, Object, Value, SentryString};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut event = Event::new();
    /// let mut object = Map::new();
    /// object.insert("test1", 1);
    /// object.insert("test2", 2);
    /// object.insert("test3", 3);
    /// let vec = object.to_vec();
    /// assert_eq!((SentryString::new("test1"), Value::Int(1)), vec[0]);
    /// event.insert("test", object);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    fn to_vec(&self) -> Vec<(SentryString, Value)> {
        let map_mp = self.to_msgpack();
        let mut map = Vec::new();

        for (key, value) in map_mp {
            let key = if let MpValue::String(key) = key {
                SentryString::from_cstring(
                    CString::new(key.into_bytes()).expect("message pack decoding failed"),
                )
            } else {
                unreachable!("message pack decoding failed")
            };

            map.push((key, mp_to_sentry(value)))
        }

        map
    }

    /// Workaround for https://github.com/getsentry/sentry-native/issues/235
    fn add_stacktrace(&self, len: usize) {
        let v = self.as_raw();

        unsafe { sys::value_add_stacktrace(v, len) };
    }
}

impl<T: Sealed> Object for T {}

macro_rules! derive_object {
    ($type:ty) => {
        impl Drop for $type {
            fn drop(&mut self) {
                if let Some(value) = self.0.take() {
                    unsafe { sys::value_decref(value) };
                }
            }
        }
        impl crate::Sealed for $type {
            fn as_raw(&self) -> sys::Value {
                self.0.expect("use after free")
            }

            fn take(mut self) -> sys::Value {
                self.0.take().expect("use after free")
            }
        }
        impl std::fmt::Debug for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                use crate::Object;

                formatter
                    .debug_map()
                    .entries(self.to_vec().into_iter())
                    .finish()
            }
        }
        impl Clone for $type {
            fn clone(&self) -> Self {
                use crate::{Object, Sealed};

                let mut object = Self::default();
                let map = self.to_msgpack();

                for (key, _) in map {
                    let key = if let rmpv::Value::String(key) = key {
                        crate::SentryString::from_cstring(
                            std::ffi::CString::new(key.into_bytes())
                                .expect("message pack decoding failed"),
                        )
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
        impl PartialEq for $type {
            fn eq(&self, other: &Self) -> bool {
                use crate::Object;

                self.to_vec() == other.to_vec()
            }
        }
        impl<S: Into<crate::SentryString>, V: Into<crate::Value>> std::iter::FromIterator<(S, V)>
            for $type
        {
            fn from_iter<I: std::iter::IntoIterator<Item = (S, V)>>(map: I) -> Self {
                use crate::Object;

                let mut object = Self::default();

                for (key, value) in map {
                    object.insert(key, value);
                }

                object
            }
        }
        impl Extend<(crate::SentryString, crate::Value)> for $type {
            fn extend<T: std::iter::IntoIterator<Item = (crate::SentryString, crate::Value)>>(
                &mut self,
                iter: T,
            ) {
                use crate::Object;

                for (key, value) in iter {
                    self.insert(key, value);
                }
            }
        }
    };
}

fn mp_to_sentry(mp_value: MpValue) -> Value {
    match mp_value {
        MpValue::Nil => Value::Null,
        MpValue::Boolean(value) => Value::Bool(value),
        MpValue::Integer(value) => Value::Int(
            value
                .as_i64()
                .and_then(|value| value.try_into().ok())
                .expect("message pack decoding failed"),
        ),
        MpValue::F64(value) => Value::Double(value),
        MpValue::String(value) => Value::String(SentryString::from_cstring(
            CString::new(value.into_bytes()).expect("message pack decoding failed"),
        )),
        MpValue::Array(value) => List::from_iter(value.into_iter().map(mp_to_sentry)).into(),
        MpValue::Map(value) => Value::Map(Map::from_iter(value.into_iter().map(|(key, value)| {
            let key = if let MpValue::String(key) = key {
                SentryString::from_cstring(
                    CString::new(key.into_bytes()).expect("message pack decoding failed"),
                )
            } else {
                unreachable!("message pack decoding failed")
            };

            (key, mp_to_sentry(value))
        }))),
        _ => unreachable!("message pack decoding failed"),
    }
}
