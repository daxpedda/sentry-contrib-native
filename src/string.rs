use crate::Error;
use std::{
    ffi::{CStr, CString},
    fmt::Debug,
};

/// A Sentry string value.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, Hash, Eq, Ord, PartialEq, PartialOrd)]
pub struct SentryString(CString);

impl Default for SentryString {
    fn default() -> Self {
        Self::new("")
    }
}

impl PartialEq<String> for SentryString {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == Ok(other)
    }
}

impl PartialEq<str> for SentryString {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == Ok(other)
    }
}

impl SentryString {
    /// Creates a new Sentry string value.
    ///
    /// # Panics
    /// This will panic if any `0` bytes are found.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{SentryString};
    /// # fn main() -> anyhow::Result<()> {
    /// let test = SentryString::new("test");
    /// assert_eq!("test", test.as_str()?);
    /// # Ok(()) }
    /// ```
    pub fn new<S: Into<Self>>(string: S) -> Self {
        string.into()
    }

    /// Creates a new Sentry string value from a [`CString`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::SentryString;
    /// # use std::ffi::CString;
    /// # fn main() -> anyhow::Result<()> {
    /// let test = SentryString::from_cstring(CString::new("test")?);
    /// assert_eq!("test", test.as_str()?);
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn from_cstring(string: CString) -> Self {
        Self(string)
    }

    /// Yields a [`str`] slice.
    ///
    /// # Errors
    /// Fails with [`Error::StrUtf8`] if value conversion fails.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::SentryString;
    /// # fn main() -> anyhow::Result<()> {
    /// let test = SentryString::new("test");
    /// assert_eq!("test", test.as_str()?);
    /// # Ok(()) }
    /// ```
    pub fn as_str(&self) -> Result<&str, Error> {
        Ok(self.0.to_str()?)
    }

    /// Extracts a [`CStr`] slice containing the entire string.
    #[must_use]
    pub fn as_cstr(&self) -> &CStr {
        &self.0
    }
}

impl<S: ToString> From<S> for SentryString {
    fn from(value: S) -> Self {
        Self(CString::new(value.to_string()).expect("null character(s) failed to be replaced"))
    }
}

impl From<SentryString> for CString {
    fn from(string: SentryString) -> Self {
        string.0
    }
}
