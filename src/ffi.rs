//! FFI helper types to communicate with `sentry-native`.

#[cfg(not(windows))]
use std::os::unix::ffi::OsStringExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::{
    ffi::{CStr, CString},
    os::raw::c_char,
    path::PathBuf,
};

/// Cross-platform return type for [`CPath::to_os_vec`].
#[cfg(windows)]
type COsString = u16;
/// Cross-platform return type for [`CPath::to_os_vec`].
#[cfg(not(windows))]
type COsString = c_char;

/// Trait for converting [`PathBuf`] to `Vec<COsString>`.
pub trait CPath {
    /// Re-encodes `self` into a C and OS compatible `Vec<COsString>`.
    ///
    /// # Panics
    /// This will panic if any `0` bytes are found.
    fn to_os_vec(self) -> Vec<COsString>;
}

impl CPath for PathBuf {
    fn to_os_vec(self) -> Vec<COsString> {
        #[cfg(windows)]
        let mut path: Vec<_> = self.into_os_string().encode_wide().collect();
        #[cfg(not(windows))]
        let mut path: Vec<_> = self
            .into_os_string()
            .into_vec()
            .into_iter()
            .map(|ch| ch as _)
            .collect();

        if path.contains(&0) {
            panic!("found 0 byte in string")
        }

        path.push(0);

        path
    }
}

/// Trait for converting `*const c_char` to `Vec<COsString>`.
pub trait CToR {
    /// Converts the given value to a String.
    ///
    /// This creates an owned value, it is your responsibility to deallocate
    /// `self`.
    fn to_cstring(self) -> Option<CString>;
}

impl CToR for *const c_char {
    fn to_cstring(self) -> Option<CString> {
        if self.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(self) }.to_owned())
        }
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::cognitive_complexity, clippy::non_ascii_literal)]

    use crate::ffi::{CPath, CToR};
    use anyhow::Result;
    #[cfg(not(windows))]
    use std::os::unix::ffi::OsStringExt;
    #[cfg(windows)]
    use std::os::windows::ffi::OsStringExt;
    use std::{
        ffi::{CStr, CString, OsString},
        os::raw::c_char,
        path::PathBuf,
        ptr,
    };

    fn convert(string: &str) -> OsString {
        let path = PathBuf::from(string.to_owned()).to_os_vec();

        #[cfg(windows)]
        {
            OsString::from_wide(&path[..])
        }
        #[cfg(not(windows))]
        {
            OsString::from_vec(path.into_iter().map(|ch| ch as _).collect())
        }
    }

    #[test]
    fn cpath() {
        assert_eq!("abcdefgh\0", convert("abcdefgh"));
        assert_eq!("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️\0", convert("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️"));
    }

    #[test]
    #[should_panic]
    fn cpath_invalid() {
        convert("\0");
    }

    #[test]
    fn cstring() -> Result<()> {
        fn convert(string: &str) -> Result<Option<String>> {
            CStr::from_bytes_with_nul(string.as_bytes())?
                .as_ptr()
                .to_cstring()
                .map(CString::into_string)
                .transpose()
                .map_err(Into::into)
        }

        assert_eq!(Some("abcdefgh"), convert("abcdefgh\0")?.as_deref());
        assert_eq!(Some("abcd🤦‍♂️efgh"), convert("abcd🤦‍♂️efgh\0")?.as_deref());
        assert_eq!(Some(""), convert("\0")?.as_deref());
        assert_eq!(None, ptr::null::<c_char>().to_cstring());

        Ok(())
    }
}
