//! Sentry object implementation, represents common functionality between
//! [`Map`](crate::Map), [`Breadcrumb`](crate::Breadcrumb),
//! [`Event`](crate::Event), and [`User`](crate::User).
use crate::{Error, List, Map, SentryString, Value};
use rmpv::{decode, Value as MpValue};
use std::{convert::TryInto, ffi::CString, iter::FromIterator, slice};

/// Private trait methods of [`Object`].
pub trait Sealed {
    /// Yields [`sys::Value`], ownership is retained.
    fn as_raw(&self) -> sys::Value;

    /// Yields [`sys::Value`], [`Object`] is consumed and caller is responsible
    /// for deallocating [`sys::Value`].
    fn take(self) -> sys::Value;

    /// Yield serialized Sentry object. Only used to iterate over keys.
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

/// Extention trait for types that behave like [`Map`]:
/// [`Breadcrumb`](crate::Breadcrumb), [`Event`](crate::Event), and
/// [`User`](crate::User).
pub trait Object: Sealed {
    /// Inserts a key-value pair into the [`Object`].
    ///
    /// # Panics
    /// Panics if Sentry failed to allocate memory.
    fn insert<S: Into<SentryString>, V: Into<Value>>(&mut self, key: S, value: V) {
        let object = self.as_raw();

        let key: CString = key.into().into();
        let value = value.into();

        match unsafe { sys::value_set_by_key(object, key.as_ptr(), value.take()) } {
            0 => (),
            _ => panic!("Sentry failed to allocate memory"),
        }
    }

    /// Removes a key from the [`Map`].
    ///
    /// # Errors
    /// Fails with [`Error::MapRemove`] if index wasn't found.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Map, Object};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut object = Map::new();
    /// object.insert("test", true);
    /// object.remove("test")?;
    /// assert_eq!(None, object.get("test"));
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

    /// Looks up a value in the [`Object`] with a key.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Map, Object, Value};
    /// let mut object = Map::new();
    /// object.insert("test", true);
    /// assert_eq!(Some(Value::Bool(true)), object.get("test"));
    /// ```
    fn get<S: Into<SentryString>>(&self, key: S) -> Option<Value> {
        let object = self.as_raw();

        let key: CString = key.into().into();

        match Value::from_raw(unsafe { sys::value_get_by_key_owned(object, key.as_ptr()) }) {
            Value::Null => None,
            value => Some(value),
        }
    }

    /// Returns the number of elements in the [`Object`].
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

    /// Returns true if the [`Object`] contains no elements.
    #[must_use]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Converts the [`Object`] to a [`Vec`].
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

                    object.insert(key.clone(), self.get(key).unwrap_or(crate::Value::Null));
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

        impl<'a, S: 'a, V: 'a> std::iter::FromIterator<&'a (S, V)> for $type
        where
            crate::SentryString: From<&'a S>,
            crate::Value: From<&'a V>,
        {
            fn from_iter<I: std::iter::IntoIterator<Item = &'a (S, V)>>(map: I) -> Self {
                use crate::Object;

                let mut object = Self::default();

                for (key, value) in map {
                    object.insert(key, value);
                }

                object
            }
        }

        impl<S: Into<crate::SentryString>, V: Into<crate::Value>> Extend<(S, V)> for $type {
            fn extend<T: std::iter::IntoIterator<Item = (S, V)>>(&mut self, iter: T) {
                use crate::Object;

                for (key, value) in iter {
                    self.insert(key, value);
                }
            }
        }

        impl<'a, S: 'a, V: 'a> Extend<&'a (S, V)> for $type
        where
            crate::SentryString: From<&'a S>,
            crate::Value: From<&'a V>,
        {
            fn extend<T: std::iter::IntoIterator<Item = &'a (S, V)>>(&mut self, iter: T) {
                use crate::Object;

                for (key, value) in iter {
                    self.insert(key, value);
                }
            }
        }
    };
}

/// Convert [`MpValue`] to [`Value`].
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

#[test]
#[allow(clippy::cognitive_complexity)]
fn object() -> anyhow::Result<()> {
    use crate::{List, Object};

    let mut object = Map::new();
    object.insert("test", true);

    let mut object2 = Map::new();
    object2.insert("test", true);

    #[allow(clippy::redundant_clone)]
    {
        assert_eq!(object, object.clone());
        assert_eq!(object, object2);
        assert_eq!(object, object2.clone());
        assert_ne!(object, Map::new());
        assert_ne!(object.clone(), Map::new());
        assert_ne!(object, Map::new().clone());
    }

    let mut object = Map::new();

    object.insert("test1", ());
    assert_eq!(object.get("test1"), None);

    object.insert(&String::from("test2"), ());
    assert_eq!(object.get(&String::from("test2")), None);

    object.insert("test3", true);
    assert_eq!(object.get("test3"), Some(true.into()));

    object.insert("test4", 4);
    assert_eq!(object.get("test4"), Some(4.into()));

    object.insert("test5", 5.5);
    assert_eq!(object.get("test5"), Some(5.5.into()));

    object.insert("test6", "6");
    assert_eq!(object.get("test6"), Some("6".into()));
    object.insert("test7", &String::from("7"));
    assert_eq!(object.get("test7"), Some((&String::from("7")).into()));

    object.insert("test8", List::new());
    assert_eq!(object.get("test8"), Some(List::new().into()));

    object.insert("test9", Map::new());
    assert_eq!(object.get(&String::from("test9")), Some(Map::new().into()));

    object.extend(&[("test10", "some"), ("test11", "test"), ("test12", "data")]);
    assert_eq!(object.get("test10"), Some("some".into()));
    assert_eq!(object.get("test11"), Some("test".into()));
    assert_eq!(object.get("test12"), Some("data".into()));
    object.extend(vec![
        ("test13", "some"),
        ("test14", "test"),
        ("test15", "data"),
    ]);
    object.extend(&vec![
        ("test16", "some"),
        ("test17", "test"),
        ("test18", "data"),
    ]);

    assert_eq!(object.len(), 18);

    let new_object: Vec<(SentryString, Value)> = vec![
        ("test1".into(), ().into()),
        ("test2".into(), ().into()),
        ("test3".into(), true.into()),
        ("test4".into(), 4.into()),
        ("test5".into(), 5.5.into()),
        ("test6".into(), "6".into()),
        ("test7".into(), "7".into()),
        ("test8".into(), List::new().into()),
        ("test9".into(), Map::new().into()),
        ("test10".into(), "some".into()),
        ("test11".into(), "test".into()),
        ("test12".into(), "data".into()),
        ("test13".into(), "some".into()),
        ("test14".into(), "test".into()),
        ("test15".into(), "data".into()),
        ("test16".into(), "some".into()),
        ("test17".into(), "test".into()),
        ("test18".into(), "data".into()),
    ];
    assert_eq!(object.to_vec(), new_object);
    assert_eq!(object, Map::from_iter(new_object.clone()));
    #[allow(clippy::redundant_clone)]
    {
        assert_eq!(object.clone(), Map::from_iter(new_object));
    }

    object.remove("test3")?;
    assert_eq!(object.get("test3"), None);
    assert_eq!(object.len(), 17);

    let object = Map::from_iter(&[("test1", ()), ("test2", ()), ("test3", ())]);
    assert_eq!(object.len(), 3);

    Ok(())
}
