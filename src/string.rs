use crate::Error;
use std::{
    convert::TryFrom,
    ffi::{CStr, CString},
    fmt::Debug,
};

/// A sentry string value.
#[allow(clippy::module_name_repetitions)]
#[derive(Clone, Debug, PartialEq)]
pub struct SentryString(CString);

impl Default for SentryString {
    fn default() -> Self {
        Self::new("")
    }
}

impl SentryString {
    /// Creates a new sentry string value.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{SentryString};
    /// # fn main() -> anyhow::Result<()> {
    /// let test = SentryString::new("test");
    /// assert_eq!("test", test.as_str()?);
    /// # Ok(()) }
    /// ```
    pub fn new<S: Into<Self>>(string: S) -> Self {
        string.into()
    }

    /// Yields a [`str`] slice.
    ///
    /// # Errors
    /// Fails with [`Error::StrUtf8`] if value conversion fails.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::SentryString;
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

impl From<CString> for SentryString {
    fn from(string: CString) -> Self {
        Self(string)
    }
}

impl From<SentryString> for CString {
    fn from(string: SentryString) -> Self {
        string.0
    }
}

impl From<&String> for SentryString {
    fn from(value: &String) -> Self {
        value.as_str().into()
    }
}

impl From<&str> for SentryString {
    fn from(value: &str) -> Self {
        // replacing `\0` with `␀`
        CString::new(value.replace("\0", "\u{2400}"))
            .expect("null character(s) failed to be replaced")
            .into()
    }
}

impl From<&&str> for SentryString {
    fn from(value: &&str) -> Self {
        // replacing `\0` with `␀`
        CString::new(value.replace("\0", "\u{2400}"))
            .expect("null character(s) failed to be replaced")
            .into()
    }
}

impl TryFrom<SentryString> for String {
    type Error = Error;

    fn try_from(value: SentryString) -> Result<Self, Self::Error> {
        value.0.into_string().map_err(Into::into)
    }
}
