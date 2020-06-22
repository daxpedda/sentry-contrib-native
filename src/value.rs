//! Sentry value implementation.

use crate::{CToR, Error, Map, Object, RToC};
use rmpv::decode;
use std::{
    collections::BTreeMap,
    convert::{TryFrom, TryInto},
    slice,
};

/// Represents a Sentry protocol value.
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
    /// # Panics
    /// Panics if `value` is a [`Value::String`] and contains null bytes.
    pub fn new<V: Into<Self>>(value: V) -> Self {
        value.into()
    }

    /// Creates a [`Value`] from [`sys::Value`]. This will deallocate the given
    /// `raw_value` or take ownership of it.
    #[allow(unused_unsafe)]
    pub(crate) unsafe fn from_raw(raw_value: sys::Value) -> Self {
        match unsafe { sys::value_get_type(raw_value) } {
            sys::ValueType::Null => {
                unsafe { sys::value_decref(raw_value) };
                Self::Null
            }
            sys::ValueType::Bool => {
                let value = match unsafe { sys::value_is_true(raw_value) } {
                    0 => Self::Bool(false),
                    1 => Self::Bool(true),
                    error => unreachable!("{} couldn't be converted to a bool", error),
                };
                unsafe { sys::value_decref(raw_value) };
                value
            }
            sys::ValueType::Int => {
                let value = Self::Int(unsafe { sys::value_as_int32(raw_value) });
                unsafe { sys::value_decref(raw_value) };
                value
            }
            sys::ValueType::Double => {
                let value = Self::Double(unsafe { sys::value_as_double(raw_value) });
                unsafe { sys::value_decref(raw_value) };
                value
            }
            sys::ValueType::String => {
                let value = Self::String(
                    unsafe { sys::value_as_string(raw_value).as_str() }
                        .expect("invalid pointer")
                        .to_owned(),
                );
                unsafe { sys::value_decref(raw_value) };
                value
            }
            sys::ValueType::List => {
                let mut list = Vec::new();

                for index in 0..unsafe { sys::value_get_length(raw_value) } {
                    list.push(unsafe {
                        Self::from_raw(sys::value_get_by_index_owned(raw_value, index))
                    })
                }

                unsafe { sys::value_decref(raw_value) };
                Self::List(list)
            }
            sys::ValueType::Object => {
                let mut size_out = 0;

                let msg_raw = unsafe { sys::value_to_msgpack(raw_value, &mut size_out) };
                unsafe { sys::value_decref(raw_value) };

                let mut msg = unsafe { slice::from_raw_parts(msg_raw as _, size_out) };
                let value = decode::read_value(&mut msg).expect("message pack decoding failed");
                unsafe { sys::free(msg_raw as _) };

                let map = value.into_value();

                if map.is_map() {
                    map
                } else {
                    panic!("message pack decoding failed")
                }
            }
        }
    }

    /// Yields [`sys::Value`], [`Value`] is consumed and caller is responsible
    /// for deallocating [`sys::Value`].
    ///
    /// # Panics
    /// - Panics if `self` is a [`Value::String`] and contains any null bytes.
    /// - Panics if Sentry failed to allocate memory.
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
            Self::Map(old_map) => Map::new(old_map).into_raw(),
        }
    }

    /// Returns `true` if `self` is [`Value::Null`].
    #[must_use]
    pub fn is_null(&self) -> bool {
        if let Self::Null = self {
            true
        } else {
            false
        }
    }

    /// Returns [`Some`] if `self` is [`Value::Null`].
    #[must_use]
    pub fn as_null(&self) -> Option<()> {
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
    pub fn into_null(self) -> Result<(), Error> {
        if let Self::Null = self {
            Ok(())
        } else {
            Err(Error::TryConvert(self))
        }
    }

    /// Returns `true` if `self` is [`Value::Bool`].
    #[must_use]
    pub fn is_bool(&self) -> bool {
        if let Self::Bool(_) = self {
            true
        } else {
            false
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::Bool`].
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    /// Returns [`Ok`] with the inner value if `self` is [`Value::Bool`].
    ///
    /// # Errors
    /// Fails with [`Error::TryConvert`] if `self` isn't a [`Value::Bool`];
    pub fn into_bool(self) -> Result<bool, Error> {
        if let Self::Bool(value) = self {
            Ok(value)
        } else {
            Err(Error::TryConvert(self))
        }
    }

    /// Returns `true` if `self` is [`Value::Int`].
    #[must_use]
    pub fn is_int(&self) -> bool {
        if let Self::Int(_) = self {
            true
        } else {
            false
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::Int`].
    #[must_use]
    pub fn as_int(&self) -> Option<i32> {
        if let Self::Int(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    /// Returns [`Ok`] with the inner value if `self` is [`Value::Int`].
    ///
    /// # Errors
    /// Fails with [`Error::TryConvert`] if `self` isn't a [`Value::Int`];
    pub fn into_int(self) -> Result<i32, Error> {
        if let Self::Int(value) = self {
            Ok(value)
        } else {
            Err(Error::TryConvert(self))
        }
    }

    /// Returns `true` if `self` is [`Value::Double`].
    #[must_use]
    pub fn is_double(&self) -> bool {
        if let Self::Double(_) = self {
            true
        } else {
            false
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::Double`].
    #[must_use]
    pub fn as_double(&self) -> Option<f64> {
        if let Self::Double(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    /// Returns [`Ok`] with the inner value if `self` is [`Value::Double`].
    ///
    /// # Errors
    /// Fails with [`Error::TryConvert`] if `self` isn't a [`Value::Double`];
    pub fn into_double(self) -> Result<f64, Error> {
        if let Self::Double(value) = self {
            Ok(value)
        } else {
            Err(Error::TryConvert(self))
        }
    }

    /// Returns `true` if `self` is [`Value::String`].
    #[must_use]
    pub fn is_string(&self) -> bool {
        if let Self::String(_) = self {
            true
        } else {
            false
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::String`].
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
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
    pub fn into_string(self) -> Result<String, Error> {
        if let Self::String(value) = self {
            Ok(value)
        } else {
            Err(Error::TryConvert(self))
        }
    }

    /// Returns `true` if `self` is [`Value::List`].
    #[must_use]
    pub fn is_list(&self) -> bool {
        if let Self::List(_) = self {
            true
        } else {
            false
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::List`].
    #[must_use]
    pub fn as_list(&self) -> Option<&Vec<Self>> {
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
    pub fn into_list(self) -> Result<Vec<Self>, Error> {
        if let Self::List(value) = self {
            Ok(value)
        } else {
            Err(Error::TryConvert(self))
        }
    }

    /// Returns `true` if `self` is [`Value::Map`].
    #[must_use]
    pub fn is_map(&self) -> bool {
        if let Self::Map(_) = self {
            true
        } else {
            false
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::Map`].
    #[must_use]
    pub fn as_map(&self) -> Option<&BTreeMap<String, Self>> {
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
    pub fn into_map(self) -> Result<BTreeMap<String, Self>, Error> {
        if let Self::Map(value) = self {
            Ok(value)
        } else {
            Err(Error::TryConvert(self))
        }
    }
}

/// Convenience trait to convert [`rmpv::Value`] to [`Value`].
trait MP {
    /// Convert [`rmpv::Value`] to [`Value`].
    fn into_value(self) -> Value;
}

impl MP for rmpv::Value {
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
            Self::Array(value) => Value::List(value.into_iter().map(MP::into_value).collect()),
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

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Double(value)
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<String> for Value {
    fn from(value: String) -> Self {
        if value.contains('\0') {
            panic!("found null byte")
        }

        Self::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl From<Vec<Self>> for Value {
    fn from(value: Vec<Self>) -> Self {
        Self::List(value)
    }
}

impl From<Vec<(String, Self)>> for Value {
    fn from(value: Vec<(String, Self)>) -> Self {
        Self::Map(value.into_iter().collect())
    }
}

impl From<BTreeMap<String, Self>> for Value {
    fn from(value: BTreeMap<String, Self>) -> Self {
        Self::Map(value)
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
            vec![("test".into(), true.into())].into(),
        ]);
        assert_eq!(value, unsafe { Value::from_raw(value.clone().into_raw()) });

        let value = Value::from(vec![
            ("0".into(), ().into()),
            ("0".into(), true.into()),
            ("0".into(), 0.into()),
            ("0".into(), 0.0.into()),
            ("0".into(), "test".into()),
            ("0".into(), vec![Value::from(true)].into()),
            ("0".into(), vec![("test".into(), true.into())].into()),
        ]);
        assert_eq!(value, unsafe { Value::from_raw(value.clone().into_raw()) });
    }
}
