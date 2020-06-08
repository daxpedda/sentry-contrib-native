use crate::{Error, List, Map, Sealed, SentryString};
use std::{
    convert::TryFrom,
    ffi::{CStr, CString},
};

/// Represents a sentry protocol value.
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
}

impl From<sys::Value> for Value {
    fn from(raw_value: sys::Value) -> Self {
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
                    unsafe { CStr::from_ptr(sys::value_as_string(raw_value)) }
                        .to_owned()
                        .into(),
                );
                unsafe { sys::value_decref(raw_value) };
                value
            }
            sys::ValueType::List => Self::List(unsafe { List::from_raw(raw_value) }),
            sys::ValueType::Object => Self::Map(unsafe { Map::from_raw(raw_value) }),
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

impl From<&String> for Value {
    fn from(value: &String) -> Self {
        SentryString::new(value).into()
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        SentryString::new(value).into()
    }
}

impl From<&&str> for Value {
    fn from(value: &&str) -> Self {
        SentryString::new(value).into()
    }
}

impl From<CString> for Value {
    fn from(value: CString) -> Self {
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

impl TryFrom<Value> for () {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        if let Value::Null = value {
            Ok(())
        } else {
            Err(Error::TryConvert)
        }
    }
}

impl TryFrom<Value> for bool {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        if let Value::Bool(value) = value {
            Ok(value)
        } else {
            Err(Error::TryConvert)
        }
    }
}

impl TryFrom<Value> for i32 {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        if let Value::Int(value) = value {
            Ok(value)
        } else {
            Err(Error::TryConvert)
        }
    }
}

impl TryFrom<Value> for f64 {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        if let Value::Double(value) = value {
            Ok(value)
        } else {
            Err(Error::TryConvert)
        }
    }
}

impl TryFrom<Value> for SentryString {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        if let Value::String(value) = value {
            Ok(value)
        } else {
            Err(Error::TryConvert)
        }
    }
}

impl TryFrom<Value> for List {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        if let Value::List(value) = value {
            Ok(value)
        } else {
            Err(Error::TryConvert)
        }
    }
}

impl TryFrom<Value> for Map {
    type Error = Error;

    fn try_from(value: Value) -> Result<Self, Error> {
        if let Value::Map(value) = value {
            Ok(value)
        } else {
            Err(Error::TryConvert)
        }
    }
}
