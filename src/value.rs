//! Sentry value implementation.

use crate::{CToR, Error, Object, RToC};
use rmpv::decode;
use std::{
    collections::BTreeMap,
    convert::{TryFrom, TryInto},
    slice,
};

/// Represents a Sentry protocol value.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::Value;
/// assert!(Value::new(()).is_null());
/// assert!(Value::new(true).is_bool());
/// assert!(Value::new(10).is_int());
/// assert!(Value::new(10.).is_double());
/// assert!(Value::new("test").is_string());
/// assert!(Value::new(vec!["test 1", "test 2"]).is_list());
/// assert!(Value::new(vec![("test key 1", "test 1"), ("test key 2", "test 2")]).is_map());
/// ```
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Value {
    /// Null value.
    Null,
    /// Boolean.
    Bool(bool),
    /// Integer.
    Int(i32),
    /// Double.
    Double(f64),
    /// String.
    String(String),
    /// List.
    List(Vec<Value>),
    /// Map.
    Map(BTreeMap<String, Value>),
}

impl Default for Value {
    fn default() -> Self {
        Self::Null
    }
}

impl Value {
    /// Creates a new Sentry value.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// let value = Value::new("test");
    /// ```
    pub fn new<V: Into<Self>>(value: V) -> Self {
        value.into()
    }

    /// Creates a [`Value`] from [`sys::Value`]. This will deallocate the given
    /// `raw_value`.
    pub(crate) unsafe fn from_raw(raw_value: sys::Value) -> Self {
        let value = Self::from_raw_borrowed(raw_value);
        sys::value_decref(raw_value);
        value
    }

    /// Creates a [`Value`] from [`sys::Value`]. This will **not** deallocate
    /// the given `raw_value`.
    pub(crate) fn from_raw_borrowed(raw_value: sys::Value) -> Self {
        match unsafe { sys::value_get_type(raw_value) } {
            sys::ValueType::Null => Self::Null,
            sys::ValueType::Bool => match unsafe { sys::value_is_true(raw_value) } {
                0 => Self::Bool(false),
                1 => Self::Bool(true),
                error => unreachable!("{} couldn't be converted to a bool", error),
            },
            sys::ValueType::Int => Self::Int(unsafe { sys::value_as_int32(raw_value) }),
            sys::ValueType::Double => Self::Double(unsafe { sys::value_as_double(raw_value) }),
            sys::ValueType::String => Self::String(
                unsafe { sys::value_as_string(raw_value).as_str() }
                    .expect("invalid pointer")
                    .to_owned(),
            ),
            sys::ValueType::List => {
                let mut list = Vec::new();

                for index in 0..unsafe { sys::value_get_length(raw_value) } {
                    list.push(unsafe {
                        Self::from_raw_borrowed(sys::value_get_by_index(raw_value, index))
                    });
                }

                Self::List(list)
            }
            sys::ValueType::Object => {
                let mut size_out = 0;

                let msg_raw = unsafe { sys::value_to_msgpack(raw_value, &mut size_out) };

                let mut msg = unsafe { slice::from_raw_parts(msg_raw.cast(), size_out) };
                let value = decode::read_value(&mut msg).expect("message pack decoding failed");

                unsafe { sys::free(msg_raw.cast()) };

                let map = value.into_value();

                if map.is_map() {
                    map
                } else {
                    unreachable!("message pack decoding failed")
                }
            }
        }
    }

    /// Yields [`sys::Value`], [`Value`] is consumed and caller is responsible
    /// for deallocating [`sys::Value`].
    ///
    /// # Panics
    /// Panics if Sentry failed to allocate memory.
    pub(crate) fn into_raw(self) -> sys::Value {
        match self {
            Self::Null => unsafe { sys::value_new_null() },
            Self::Bool(value) => unsafe { sys::value_new_bool(value.into()) },
            Self::Int(value) => unsafe { sys::value_new_int32(value) },
            Self::Double(value) => unsafe { sys::value_new_double(value) },
            Self::String(value) => {
                let string = value.into_cstring();
                unsafe { sys::value_new_string(string.as_ptr()) }
            }
            Self::List(old_list) => {
                let list = unsafe { sys::value_new_list() };

                for value in old_list {
                    match unsafe { sys::value_append(list, value.into_raw()) } {
                        0 => (),
                        _ => panic!("Sentry failed to allocate memory"),
                    }
                }

                list
            }
            Self::Map(map) => map.into_raw(),
        }
    }

    /// Returns `true` if `self` is [`Value::Null`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// assert!(Value::new(()).is_null());
    /// ```
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Returns [`Some`] if `self` is [`Value::Null`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// assert_eq!(Some(()), Value::new(()).as_null());
    /// ```
    #[must_use]
    pub const fn as_null(&self) -> Option<()> {
        if let Self::Null = self {
            Some(())
        } else {
            None
        }
    }

    /// Returns [`Ok`] if `self` is [`Value::Null`].
    ///
    /// # Errors
    /// Fails with [`Error::TryConvert`] if `self` isn't a [`Value::Null`];
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Error, Value};
    /// assert_eq!(Ok(()), Value::new(()).into_null());
    /// assert_eq!(
    ///     Err(Error::TryConvert(Value::new(false))),
    ///     Value::new(false).into_null()
    /// );
    /// ```
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_null(self) -> Result<(), Error> {
        if let Self::Null = self {
            Ok(())
        } else {
            Err(Error::TryConvert(self))
        }
    }

    /// Returns `true` if `self` is [`Value::Bool`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// assert!(Value::new(true).is_bool());
    /// ```
    #[must_use]
    pub const fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::Bool`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// assert_eq!(Some(true), Value::new(true).as_bool());
    /// ```
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::Bool`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// let mut value = Value::new(true);
    /// value.as_mut_bool().map(|value| *value = false);
    ///
    /// assert_eq!(Some(false), value.as_bool());
    /// ```
    #[must_use]
    pub fn as_mut_bool(&mut self) -> Option<&mut bool> {
        if let Self::Bool(value) = self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns [`Ok`] with the inner value if `self` is [`Value::Bool`].
    ///
    /// # Errors
    /// Fails with [`Error::TryConvert`] if `self` isn't a [`Value::Bool`];
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Error, Value};
    /// assert_eq!(Ok(true), Value::new(true).into_bool());
    /// assert_eq!(
    ///     Err(Error::TryConvert(Value::new(()))),
    ///     Value::new(()).into_bool()
    /// );
    /// ```
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_bool(self) -> Result<bool, Error> {
        if let Self::Bool(value) = self {
            Ok(value)
        } else {
            Err(Error::TryConvert(self))
        }
    }

    /// Returns `true` if `self` is [`Value::Int`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// assert!(Value::new(10).is_int());
    /// ```
    #[must_use]
    pub const fn is_int(&self) -> bool {
        matches!(self, Self::Int(_))
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::Int`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// assert_eq!(Some(10), Value::new(10).as_int());
    /// ```
    #[must_use]
    pub const fn as_int(&self) -> Option<i32> {
        if let Self::Int(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::Int`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// let mut value = Value::new(10);
    /// value.as_mut_int().map(|value| *value = 5);
    ///
    /// assert_eq!(Some(5), value.as_int());
    /// ```
    #[must_use]
    pub fn as_mut_int(&mut self) -> Option<&mut i32> {
        if let Self::Int(value) = self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns [`Ok`] with the inner value if `self` is [`Value::Int`].
    ///
    /// # Errors
    /// Fails with [`Error::TryConvert`] if `self` isn't a [`Value::Int`];
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Error, Value};
    /// assert_eq!(Ok(10), Value::new(10).into_int());
    /// assert_eq!(
    ///     Err(Error::TryConvert(Value::new(false))),
    ///     Value::new(false).into_int()
    /// );
    /// ```
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_int(self) -> Result<i32, Error> {
        if let Self::Int(value) = self {
            Ok(value)
        } else {
            Err(Error::TryConvert(self))
        }
    }

    /// Returns `true` if `self` is [`Value::Double`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// assert!(Value::new(10.).is_double());
    /// ```
    #[must_use]
    pub const fn is_double(&self) -> bool {
        matches!(self, Self::Double(_))
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::Double`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// assert_eq!(Some(10.), Value::new(10.).as_double());
    /// ```
    #[must_use]
    pub const fn as_double(&self) -> Option<f64> {
        if let Self::Double(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::Double`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// let mut value = Value::new(10.);
    /// value.as_mut_double().map(|value| *value = 5.);
    ///
    /// assert_eq!(Some(5.), value.as_double());
    /// ```
    #[must_use]
    pub fn as_mut_double(&mut self) -> Option<&mut f64> {
        if let Self::Double(value) = self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns [`Ok`] with the inner value if `self` is [`Value::Double`].
    ///
    /// # Errors
    /// Fails with [`Error::TryConvert`] if `self` isn't a [`Value::Double`];
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Error, Value};
    /// assert_eq!(Ok(10.), Value::new(10.).into_double());
    /// assert_eq!(
    ///     Err(Error::TryConvert(Value::new(false))),
    ///     Value::new(false).into_double()
    /// );
    /// ```
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_double(self) -> Result<f64, Error> {
        if let Self::Double(value) = self {
            Ok(value)
        } else {
            Err(Error::TryConvert(self))
        }
    }

    /// Returns `true` if `self` is [`Value::String`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// assert!(Value::new("test").is_string());
    /// ```
    #[must_use]
    pub const fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::String`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// assert_eq!(Some("test"), Value::new("test").as_str());
    /// ```
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(value) = self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::String`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// let mut value = Value::new("test");
    /// value
    ///     .as_mut_str()
    ///     .map(|value| value.get_mut(0..1).unwrap().make_ascii_uppercase());
    ///
    /// assert_eq!(Some("Test"), value.as_str());
    /// ```
    #[must_use]
    pub fn as_mut_str(&mut self) -> Option<&mut str> {
        if let Self::String(value) = self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns [`Ok`] with the inner value if `self` is [`Value::String`].
    ///
    /// # Errors
    /// Fails with [`Error::TryConvert`] if `self` isn't a [`Value::String`];
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Error, Value};
    /// assert_eq!(Ok(String::from("test")), Value::new("test").into_string());
    /// assert_eq!(
    ///     Err(Error::TryConvert(Value::new(false))),
    ///     Value::new(false).into_string()
    /// );
    /// ```
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_string(self) -> Result<String, Error> {
        if let Self::String(value) = self {
            Ok(value)
        } else {
            Err(Error::TryConvert(self))
        }
    }

    /// Returns `true` if `self` is [`Value::List`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// assert!(Value::new(vec!["test 1", "test 2"]).is_list());
    /// ```
    #[must_use]
    pub const fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::List`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// assert_eq!(
    ///     Some(&vec!["test 1".into(), "test 2".into()]),
    ///     Value::new(vec!["test 1", "test 2"]).as_list()
    /// );
    /// ```
    #[must_use]
    pub const fn as_list(&self) -> Option<&Vec<Self>> {
        if let Self::List(value) = self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::List`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut value = Value::new(vec!["test 1", "test 2"]);
    /// value.as_mut_list().map(|value| value[0] = "test 3".into());
    ///
    /// assert_eq!(Some("test 3"), value.into_list()?[0].as_str());
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn as_mut_list(&mut self) -> Option<&mut Vec<Self>> {
        if let Self::List(value) = self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns [`Ok`] with the inner value if `self` is [`Value::List`].
    ///
    /// # Errors
    /// Fails with [`Error::TryConvert`] if `self` isn't a [`Value::List`];
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Error, Value};
    /// assert_eq!(
    ///     Ok(vec!["test 1".into(), "test 2".into()]),
    ///     Value::new(vec!["test 1", "test 2"]).into_list()
    /// );
    /// assert_eq!(
    ///     Err(Error::TryConvert(Value::new(false))),
    ///     Value::new(false).into_list()
    /// );
    /// ```
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_list(self) -> Result<Vec<Self>, Error> {
        if let Self::List(value) = self {
            Ok(value)
        } else {
            Err(Error::TryConvert(self))
        }
    }

    /// Returns `true` if `self` is [`Value::Map`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// assert!(Value::new(vec![("test key 1", "test 1"), ("test key 2", "test 2")]).is_map());
    /// ```
    #[must_use]
    pub const fn is_map(&self) -> bool {
        matches!(self, Self::Map(_))
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::Map`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// # use std::{collections::BTreeMap, iter::FromIterator};
    /// assert_eq!(
    ///     Some(&BTreeMap::from_iter(vec![
    ///         ("test key 1".into(), "test 1".into()),
    ///         ("test key 2".into(), "test 2".into())
    ///     ])),
    ///     Value::new(vec![("test key 1", "test 1"), ("test key 2", "test 2")]).as_map()
    /// );
    /// ```
    #[must_use]
    pub const fn as_map(&self) -> Option<&BTreeMap<String, Self>> {
        if let Self::Map(value) = self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::Map`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::Value;
    /// # use std::{collections::BTreeMap, iter::FromIterator};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut value = Value::new(vec![("test key 1", false), ("test key 2", false)]);
    /// value
    ///     .as_mut_map()
    ///     .and_then(|value| value.get_mut("test key 1"))
    ///     .and_then(|value| value.as_mut_bool())
    ///     .map(|value| *value = true);
    ///
    /// assert_eq!(Some(true), value.into_map()?["test key 1"].as_bool());
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn as_mut_map(&mut self) -> Option<&mut BTreeMap<String, Self>> {
        if let Self::Map(value) = self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns [`Ok`] with the inner value if `self` is [`Value::Map`].
    ///
    /// # Errors
    /// Fails with [`Error::TryConvert`] if `self` isn't a [`Value::Map`];
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Error, Value};
    /// # use std::{collections::BTreeMap, iter::FromIterator};
    /// assert_eq!(
    ///     Ok(BTreeMap::from_iter(vec![
    ///         ("test key 1".into(), "test 1".into()),
    ///         ("test key 2".into(), "test 2".into())
    ///     ])),
    ///     Value::new(vec![("test key 1", "test 1"), ("test key 2", "test 2")]).into_map()
    /// );
    /// assert_eq!(
    ///     Err(Error::TryConvert(Value::new(false))),
    ///     Value::new(false).into_map()
    /// );
    /// ```
    #[allow(clippy::missing_const_for_fn)]
    pub fn into_map(self) -> Result<BTreeMap<String, Self>, Error> {
        if let Self::Map(value) = self {
            Ok(value)
        } else {
            Err(Error::TryConvert(self))
        }
    }
}

/// Convenience trait to convert [`rmpv::Value`] to [`Value`].
trait Mp {
    /// Convert [`rmpv::Value`] to [`Value`].
    fn into_value(self) -> Value;
}

impl Mp for rmpv::Value {
    fn into_value(self) -> Value {
        match self {
            Self::Nil => Value::Null,
            Self::Boolean(value) => Value::Bool(value),
            Self::Integer(value) => Value::Int(
                value
                    .as_i64()
                    .and_then(|value| value.try_into().ok())
                    .expect("message pack decoding failed"),
            ),
            Self::F64(value) => Value::Double(value),
            Self::String(value) => {
                Value::String(value.into_str().expect("message pack decoding failed"))
            }
            Self::Array(value) => Value::List(value.into_iter().map(Mp::into_value).collect()),
            Self::Map(value) => Value::Map(
                value
                    .into_iter()
                    .map(|(key, value)| {
                        let key = if let Self::String(key) = key {
                            key.into_str().expect("message pack decoding failed")
                        } else {
                            unreachable!("message pack decoding failed")
                        };

                        (key, value.into_value())
                    })
                    .collect(),
            ),
            _ => unreachable!("message pack decoding failed"),
        }
    }
}

impl From<()> for Value {
    fn from(_: ()) -> Self {
        Self::Null
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Self::Int(value)
    }
}

impl From<u8> for Value {
    fn from(value: u8) -> Self {
        Self::Int(value.into())
    }
}

impl From<i8> for Value {
    fn from(value: i8) -> Self {
        Self::Int(value.into())
    }
}

impl From<u16> for Value {
    fn from(value: u16) -> Self {
        Self::Int(value.into())
    }
}

impl From<i16> for Value {
    fn from(value: i16) -> Self {
        Self::Int(value.into())
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Double(value)
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Self::Double(value.into())
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<String> for Value {
    fn from(value: String) -> Self {
        assert!(!value.contains('\0'), "found null byte");

        Self::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl<V: Into<Self>> From<Vec<V>> for Value {
    fn from(value: Vec<V>) -> Self {
        Self::List(value.into_iter().map(Into::into).collect())
    }
}

impl<K: Into<String>, V: Into<Self>> From<Vec<(K, V)>> for Value {
    fn from(value: Vec<(K, V)>) -> Self {
        Self::Map(
            value
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        )
    }
}

impl<K: Into<String>, V: Into<Self>> From<BTreeMap<K, V>> for Value {
    fn from(value: BTreeMap<K, V>) -> Self {
        Self::Map(
            value
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        )
    }
}

impl<V: Into<Self> + Copy> From<&V> for Value {
    fn from(value: &V) -> Self {
        (*value).into()
    }
}

impl TryFrom<Value> for () {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        value.into_null()
    }
}

impl TryFrom<Value> for bool {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        value.into_bool()
    }
}

impl TryFrom<Value> for i32 {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        value.into_int()
    }
}

impl TryFrom<Value> for f64 {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        value.into_double()
    }
}

impl TryFrom<Value> for String {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        value.into_string()
    }
}

impl TryFrom<Value> for Vec<Value> {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        value.into_list()
    }
}

impl TryFrom<Value> for BTreeMap<String, Value> {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        value.into_map()
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::non_ascii_literal)]

    use crate::Value;

    #[test]
    fn value() {
        let value = Value::new(());
        assert_eq!(value, unsafe { Value::from_raw(value.clone().into_raw()) });

        let value = Value::new(false);
        assert_eq!(value, unsafe { Value::from_raw(value.clone().into_raw()) });

        let value = Value::new(true);
        assert_eq!(value, unsafe { Value::from_raw(value.clone().into_raw()) });

        let value = Value::new(-100);
        assert_eq!(value, unsafe { Value::from_raw(value.clone().into_raw()) });

        let value = Value::new(0);
        assert_eq!(value, unsafe { Value::from_raw(value.clone().into_raw()) });

        let value = Value::new(0.);
        assert_eq!(value, unsafe { Value::from_raw(value.clone().into_raw()) });

        let value = Value::new(1_000_000.);
        assert_eq!(value, unsafe { Value::from_raw(value.clone().into_raw()) });

        let value = Value::new("asdasdasd");
        assert_eq!(value, unsafe { Value::from_raw(value.clone().into_raw()) });

        let value = Value::new("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️");
        assert_eq!(value, unsafe { Value::from_raw(value.clone().into_raw()) });

        let value = Value::List(vec![
            ().into(),
            true.into(),
            0.into(),
            0.0.into(),
            "test".into(),
            vec![Value::from(true)].into(),
            vec![("test", true)].into(),
        ]);
        assert_eq!(value, unsafe { Value::from_raw(value.clone().into_raw()) });

        let value = Value::from(vec![
            ("0", Value::from(())),
            ("0", true.into()),
            ("0", 0.into()),
            ("0", 0.0.into()),
            ("0", "test".into()),
            ("0", vec![Value::from(true)].into()),
            ("0", vec![("test", true)].into()),
        ]);
        assert_eq!(value, unsafe { Value::from_raw(value.clone().into_raw()) });
    }
}

#[test]
#[allow(clippy::cognitive_complexity)]
fn value_new() {
    assert!(Value::new(()).is_null());
    assert!(Value::new(true).is_bool());

    assert!(Value::new(10).is_int());
    assert!(Value::new(10_i32).is_int());
    assert!(Value::new(10_u8).is_int());
    assert!(Value::new(10_i8).is_int());
    assert!(Value::new(10_u16).is_int());
    assert!(Value::new(10_i16).is_int());

    assert!(Value::new(10.0).is_double());
    assert!(Value::new(10.0_f64).is_double());
    assert!(Value::new(10.0_f32).is_double());

    assert!(Value::new("test").is_string());
    assert!(Value::new(String::from("test")).is_string());

    assert!(Value::new(vec![()]).is_list());
    assert!(Value::new(vec![true]).is_list());
    assert!(Value::new(vec![10]).is_list());
    assert!(Value::new(vec![10.]).is_list());
    assert!(Value::new(vec!["test"]).is_list());
    assert!(Value::new(vec![String::from("test")]).is_list());
    assert!(Value::new(vec![vec![("test", ())]]).is_list());

    assert!(Value::new(vec![vec![()]])
        .as_list()
        .map(|v| v[0].is_list())
        .unwrap());
    assert!(Value::new(vec![vec![true]])
        .as_list()
        .map(|v| v[0].is_list())
        .unwrap());
    assert!(Value::new(vec![vec![10]])
        .as_list()
        .map(|v| v[0].is_list())
        .unwrap());
    assert!(Value::new(vec![vec![10.]])
        .as_list()
        .map(|v| v[0].is_list())
        .unwrap());
    assert!(Value::new(vec![vec!["test"]])
        .as_list()
        .map(|v| v[0].is_list())
        .unwrap());
    assert!(Value::new(vec![vec![String::from("test")]])
        .as_list()
        .map(|v| v[0].is_list())
        .unwrap());
    assert!(Value::new(vec![vec![vec![("test", ())]]])
        .as_list()
        .map(|v| v[0].is_list())
        .unwrap());

    assert!(Value::new(vec![("test", ())]).is_map());
    assert!(Value::new(vec![("test", true)]).is_map());
    assert!(Value::new(vec![("test", 10)]).is_map());
    assert!(Value::new(vec![("test", 10.)]).is_map());
    assert!(Value::new(vec![("test", "test")]).is_map());
    assert!(Value::new(vec![(String::from("test"), String::from("test"))]).is_map());
    assert!(Value::new(vec![("test", vec![()])]).is_map());

    assert!(Value::new(vec![("test", vec![("test", ())])])
        .as_map()
        .map(|v| v["test"].is_map())
        .unwrap());
    assert!(Value::new(vec![("test", vec![("test", true)])])
        .as_map()
        .map(|v| v["test"].is_map())
        .unwrap());
    assert!(Value::new(vec![("test", vec![("test", 10)])])
        .as_map()
        .map(|v| v["test"].is_map())
        .unwrap());
    assert!(Value::new(vec![("test", vec![("test", 10.)])])
        .as_map()
        .map(|v| v["test"].is_map())
        .unwrap());
    assert!(Value::new(vec![("test", vec![("test", "test")])])
        .as_map()
        .map(|v| v["test"].is_map())
        .unwrap());
    assert!(
        Value::new(vec![("test", vec![("test", String::from("test"))])])
            .as_map()
            .map(|v| v["test"].is_map())
            .unwrap()
    );
    assert!(Value::new(vec![("test", vec![("test", vec![()])])])
        .as_map()
        .map(|v| v["test"].is_map())
        .unwrap());
}

#[test]
#[allow(clippy::cognitive_complexity)]
fn value_methods() {
    use std::iter::FromIterator;

    let failure = Value::new(false);

    assert!(Value::new(()).is_null());
    assert_eq!(Some(()), Value::new(()).as_null());
    assert_eq!(Ok(()), Value::new(()).into_null());
    assert_eq!(
        Err(Error::TryConvert(failure.clone())),
        failure.clone().into_null()
    );

    assert!(Value::new(true).is_bool());
    assert_eq!(Some(true), Value::new(true).as_bool());
    assert_eq!(Some(&mut true), Value::new(true).as_mut_bool());
    assert_eq!(Ok(true), Value::new(true).into_bool());
    assert_eq!(
        Err(Error::TryConvert(Value::new(()))),
        Value::new(()).into_bool()
    );

    assert!(Value::new(10).is_int());
    assert_eq!(Some(10), Value::new(10).as_int());
    assert_eq!(Some(&mut 10), Value::new(10).as_mut_int());
    assert_eq!(Ok(10), Value::new(10).into_int());
    assert_eq!(
        Err(Error::TryConvert(failure.clone())),
        failure.clone().into_int()
    );

    assert!(Value::new(10.).is_double());
    assert_eq!(Some(10.), Value::new(10.).as_double());
    assert_eq!(Some(&mut 10.), Value::new(10.).as_mut_double());
    assert_eq!(Ok(10.), Value::new(10.).into_double());
    assert_eq!(
        Err(Error::TryConvert(failure.clone())),
        failure.clone().into_double()
    );

    assert!(Value::new("test").is_string());
    assert_eq!(Some("test"), Value::new("test").as_str());
    let mut test = String::from("test");
    assert_eq!(Some(test.as_mut_str()), Value::new("test").as_mut_str());
    assert_eq!(Ok(String::from("test")), Value::new("test").into_string());
    assert_eq!(
        Err(Error::TryConvert(failure.clone())),
        failure.clone().into_string()
    );

    let list = vec![Value::from("test 1"), "test 2".into()];
    assert!(Value::new(list.clone()).is_list());
    assert_eq!(Some(&list), Value::new(list.clone()).as_list());
    let mut list2 = list.clone();
    assert_eq!(Some(&mut list2), Value::new(list.clone()).as_mut_list());
    assert_eq!(Ok(list.clone()), Value::new(list).into_list());
    assert_eq!(
        Err(Error::TryConvert(failure.clone())),
        failure.clone().into_list()
    );

    let map = BTreeMap::from_iter(vec![
        (String::from("test key 1"), Value::from("test 1")),
        ("test key 2".into(), "test 2".into()),
    ]);
    assert!(Value::new(map.clone()).is_map());
    assert_eq!(Some(&map), Value::new(map.clone()).as_map());
    let mut map2 = map.clone();
    assert_eq!(Some(&mut map2), Value::new(map.clone()).as_mut_map());
    assert_eq!(Ok(map.clone()), Value::new(map).into_map());
    assert_eq!(Err(Error::TryConvert(failure.clone())), failure.into_map());
}
