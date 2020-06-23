//! FFI helper types to communicate with `sentry-native`.

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::{
    ffi::{CStr, CString},
    os::raw::c_char,
    path::PathBuf,
};
#[cfg(not(windows))]
use std::{mem, os::unix::ffi::OsStringExt};

/// Cross-platform return type for [`CPath::into_os_vec`].
#[cfg(windows)]
type COsString = u16;
/// Cross-platform return type for [`CPath::into_os_vec`].
#[cfg(not(windows))]
type COsString = c_char;

/// Trait for converting [`PathBuf`] to `Vec<COsString>`.
pub trait CPath {
    /// Re-encodes `self` into an OS compatible `Vec<COsString>`.
    ///
    /// # Panics
    /// Panics if `self` contains any null bytes.
    fn into_os_vec(self) -> Vec<COsString>;
}

impl CPath for PathBuf {
    fn into_os_vec(self) -> Vec<COsString> {
        #[cfg(windows)]
        let mut path: Vec<_> = self.into_os_string().encode_wide().collect();
        #[cfg(not(windows))]
        let mut path: Vec<_> = self
            .into_os_string()
            .into_vec()
            .into_iter()
            .map(|ch| unsafe { mem::transmute::<u8, i8>(ch) })
            .collect();

        if path.contains(&0) {
            panic!("found null byte")
        }

        path.push(0);

        path
    }
}

/// Trait for converting `*const c_char` to [`str`].
pub trait CToR {
    /// Converts the given value to a [`str`].
    ///
    /// # Panics
    /// Panics if `self` contains any invalid UTF-8.
    ///
    /// # Safety
    /// The same safety issues apply as in [`CStr::from_ptr`], except the null
    /// pointer check, but the main concern is the lifetime of the pointer.
    unsafe fn as_str<'a>(self) -> Option<&'a str>;
}

impl CToR for *const c_char {
    #[allow(unused_unsafe)]
    unsafe fn as_str<'a>(self) -> Option<&'a str> {
        if self.is_null() {
            None
        } else {
            Some(
                unsafe { CStr::from_ptr(self) }
                    .to_str()
                    .expect("invalid UTF-8"),
            )
        }
    }
}

/// Trait for converting [`str`] to [`CString`].
pub trait RToC {
    /// Re-encodes `self` into a [`CString`].
    ///
    /// # Panics
    /// Panics if any null bytes are found.
    fn into_cstring(self) -> CString;
}

impl RToC for String {
    fn into_cstring(self) -> CString {
        CString::new(self).expect("found null byte")
    }
}

#[cfg(test)]
macro_rules! invalid {
    ($name:ident, $test:expr) => {
        #[test]
        #[should_panic]
        fn $name() {
            $test;
        }
    };
}

#[cfg(test)]
mod cpath {
    #![allow(clippy::non_ascii_literal)]

    use crate::CPath;
    #[cfg(windows)]
    use std::os::windows::ffi::OsStringExt;
    use std::{ffi::OsString, path::PathBuf};
    #[cfg(not(windows))]
    use std::{mem, os::unix::ffi::OsStringExt};

    fn convert(string: &str) -> OsString {
        let path = PathBuf::from(string.to_owned()).into_os_vec();

        #[cfg(windows)]
        {
            OsString::from_wide(&path[..])
        }
        #[cfg(not(windows))]
        {
            OsString::from_vec(
                path.into_iter()
                    .map(|ch| unsafe { mem::transmute::<i8, u8>(ch) })
                    .collect(),
            )
        }
    }

    #[test]
    fn valid() {
        assert_eq!("abcdefgh\0", convert("abcdefgh"));
        assert_eq!("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️\0", convert("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️"));
    }

    invalid!(invalid_1, convert("abcdefgh\0"));
    invalid!(invalid_2, convert("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️\0"));
    invalid!(invalid_3, convert("\0abcdefgh"));
    invalid!(invalid_4, convert("\0🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️"));
    invalid!(invalid_5, convert("abcd\0efgh"));
    invalid!(invalid_6, convert("🤦‍♂️🤦‍♀️\0🤷‍♂️🤷‍♀️"));
}

#[cfg(test)]
mod ctor {
    #![allow(clippy::non_ascii_literal)]

    use crate::CToR;
    use std::{ffi::CString, os::raw::c_char, ptr};

    fn convert(string: &str) -> String {
        let string = CString::new(string).unwrap();
        unsafe { string.as_ptr().as_str() }.unwrap().to_owned()
    }

    #[test]
    fn valid() {
        assert_eq!("abcdefgh", convert("abcdefgh"));
        assert_eq!("abcd🤦‍♂️efgh", convert("abcd🤦‍♂️efgh"));
        assert_eq!("", convert(""));
        assert_eq!(None, unsafe { ptr::null::<c_char>().as_str() });
    }

    invalid!(invalid, {
        let string = CString::new(vec![0xfe, 0xfe, 0xff, 0xff]).unwrap();
        unsafe { string.as_ptr().as_str() };
    });
}

#[cfg(test)]
mod rtoc {
    #![allow(clippy::non_ascii_literal)]

    use crate::RToC;

    fn convert(string: &str) -> String {
        string
            .to_owned()
            .into_cstring()
            .to_str()
            .unwrap()
            .to_owned()
    }

    #[test]
    fn valid() {
        assert_eq!("abcdefgh", convert("abcdefgh"));
        assert_eq!("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️", convert("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️"));
    }

    invalid!(invalid_1, convert("abcdefgh\0"));
    invalid!(invalid_2, convert("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️\0"));
    invalid!(invalid_3, convert("\0abcdefgh"));
    invalid!(invalid_4, convert("\0🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️"));
    invalid!(invalid_5, convert("abcd\0efgh"));
    invalid!(invalid_6, convert("🤦‍♂️🤦‍♀️\0🤷‍♂️🤷‍♀️"));
}
