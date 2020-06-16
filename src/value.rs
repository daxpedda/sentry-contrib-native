use crate::{List, Map, Sealed, SentryString};
use std::ffi::{CStr, CString};

/// Represents a Sentry protocol value.
#[derive(Debug, Clone, PartialEq)]
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
    String(SentryString),
    /// List.
    List(List),
    /// Map.
    Map(Map),
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
    /// This will panic if any string contains `0` bytes.
    pub fn new<V: Into<Self>>(value: V) -> Self {
        value.into()
    }

    pub(crate) fn from_raw(raw_value: sys::Value) -> Self {
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
                let value = Self::String(SentryString::from_cstring(
                    unsafe { CStr::from_ptr(sys::value_as_string(raw_value)) }.to_owned(),
                ));
                unsafe { sys::value_decref(raw_value) };
                value
            }
            sys::ValueType::List => Self::List(unsafe { List::from_raw(raw_value) }),
            sys::ValueType::Object => Self::Map(unsafe { Map::from_raw(raw_value) }),
        }
    }

    pub(crate) fn take(self) -> sys::Value {
        match self {
            Self::Null => unsafe { sys::value_new_null() },
            Self::Bool(value) => unsafe { sys::value_new_bool(value.into()) },
            Self::Int(value) => unsafe { sys::value_new_int32(value) },
            Self::Double(value) => unsafe { sys::value_new_double(value) },
            Self::String(value) => {
                let string: CString = value.into();
                unsafe { sys::value_new_string(string.as_ptr()) }
            }
            Self::List(list) => list.take(),
            Self::Map(map) => map.take(),
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

    /// Returns [`Some`] with the inner value if `self` is [`Value::Bool`].
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(value) = self {
            Some(*value)
        } else {
            None
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

    /// Returns [`Some`] with the inner value if `self` is [`Value::Double`].
    #[must_use]
    pub fn as_double(&self) -> Option<f64> {
        if let Self::Double(value) = self {
            Some(*value)
        } else {
            None
        }
    }

    /// Returns [`Some`] with the inner value as a [`str`] if `self` is
    /// [`Value::String`] and valid UTF-8.
    #[must_use]
    pub fn as_string(&self) -> Option<&str> {
        if let Self::String(value) = self {
            value.as_str().ok()
        } else {
            None
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::List`].
    #[must_use]
    pub fn as_list(&self) -> Option<&List> {
        if let Self::List(value) = self {
            Some(value)
        } else {
            None
        }
    }

    /// Returns [`Some`] with the inner value if `self` is [`Value::Map`].
    #[must_use]
    pub fn as_map(&self) -> Option<&Map> {
        if let Self::Map(value) = self {
            Some(value)
        } else {
            None
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

impl From<SentryString> for Value {
    fn from(value: SentryString) -> Self {
        Self::String(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        SentryString::new(value).into()
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        SentryString::new(value).into()
    }
}

impl From<List> for Value {
    fn from(value: List) -> Self {
        Self::List(value)
    }
}

impl From<Map> for Value {
    fn from(value: Map) -> Self {
        Self::Map(value)
    }
}

impl<V: Into<Value> + Copy> From<&V> for Value {
    fn from(value: &V) -> Self {
        (*value).into()
    }
}
