//! FFI helper functions, traits and types to communicate with `sentry-native`.

#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(doc)]
use std::process::abort;
use std::{
    ffi::{CStr, CString},
    os::raw::c_char,
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
    process,
};

#[cfg(not(target_os = "windows"))]
use std::{mem, os::unix::ffi::OsStringExt};

/// Cross-platform return type for [`CPath::into_os_vec`].
#[cfg(target_os = "windows")]
type COsString = u16;
/// Cross-platform return type for [`CPath::into_os_vec`].
#[cfg(not(target_os = "windows"))]
type COsString = c_char;

/// Helper trait to convert [`PathBuf`] to `Vec<COsString>`.
pub trait CPath {
    /// Re-encodes `self` into an OS compatible `Vec<COsString>`.
    fn into_os_vec(self) -> Vec<COsString>;
}

impl CPath for PathBuf {
    fn into_os_vec(self) -> Vec<COsString> {
        let path = self.into_os_string();

        #[cfg(target_os = "windows")]
        let path = path.encode_wide();
        #[cfg(all(
            not(target_os = "windows"),
            not(all(target_os = "linux", target_arch = "aarch64"))
        ))]
        let path = path
            .into_vec()
            .into_iter()
            .map(|ch| unsafe { mem::transmute::<u8, i8>(ch) });
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        let path = path.into_vec().into_iter();

        path.take_while(|ch| *ch != 0).chain(Some(0)).collect()
    }
}

/// Helper trait to convert `*const c_char` to [`str`].
pub trait CToR {
    /// Yields a [`str`] from `self`.
    ///
    /// # Panics
    /// Panics if `self` contains any invalid UTF-8.
    ///
    /// # Safety
    /// The same safety issues apply as in [`CStr::from_ptr`], except the null
    /// pointer check, but the main concern is the lifetime of the pointer.
    #[allow(clippy::wrong_self_convention)]
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
                    .expect("found invalid UTF-8"),
            )
        }
    }
}

/// Helper trait to convert [`String`] to [`CString`].
pub trait RToC {
    /// Re-encodes `self` into a [`CString`].
    fn into_cstring(self) -> CString;
}

impl RToC for String {
    fn into_cstring(mut self) -> CString {
        if let Some(position) = self.find('\0') {
            self.truncate(position);
        }

        CString::new(self).expect("found null byte")
    }
}

/// Catch unwinding panics and [`abort`] if any occured.
pub fn catch<R>(fun: impl FnOnce() -> R) -> R {
    match panic::catch_unwind(AssertUnwindSafe(fun)) {
        Ok(ret) => ret,
        Err(_) => process::abort(),
    }
}

#[cfg(test)]
mod cpath {
    #![allow(clippy::non_ascii_literal)]

    use crate::CPath;
    #[cfg(target_os = "windows")]
    use std::os::windows::ffi::OsStringExt;
    use std::{ffi::OsString, path::PathBuf};
    #[cfg(not(target_os = "windows"))]
    use std::{mem, os::unix::ffi::OsStringExt};

    fn convert(string: &str) -> OsString {
        let path = PathBuf::from(string.to_owned()).into_os_vec();

        #[cfg(target_os = "windows")]
        {
            OsString::from_wide(&path[..])
        }
        #[cfg(all(
            not(target_os = "windows"),
            not(all(target_os = "linux", target_arch = "aarch64"))
        ))]
        {
            OsString::from_vec(
                path.into_iter()
                    .map(|ch| unsafe { mem::transmute::<i8, u8>(ch) })
                    .collect(),
            )
        }
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        {
            OsString::from_vec(path.into_iter().collect())
        }
    }

    #[test]
    fn valid() {
        assert_eq!("abcdefgh\0", convert("abcdefgh"));
        assert_eq!("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️\0", convert("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️"));
        assert_eq!("abcdefgh\0", convert("abcdefgh\0"));
        assert_eq!("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️\0", convert("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️\0"));
        assert_eq!("\0", convert("\0abcdefgh"));
        assert_eq!("\0", convert("\0🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️"));
        assert_eq!("abcd\0", convert("abcd\0efgh"));
        assert_eq!("🤦‍♂️🤦‍♀️\0", convert("🤦‍♂️🤦‍♀️\0🤷‍♂️🤷‍♀️"));
    }
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

    #[test]
    #[should_panic]
    fn invalid() {
        let string = CString::new(vec![0xfe, 0xfe, 0xff, 0xff]).unwrap();
        unsafe { string.as_ptr().as_str() };
    }
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
        assert_eq!("abcdefgh", convert("abcdefgh\0"));
        assert_eq!("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️", convert("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️\0"));
        assert_eq!("", convert("\0abcdefgh"));
        assert_eq!("", convert("\0🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️"));
        assert_eq!("abcd", convert("abcd\0efgh"));
        assert_eq!("🤦‍♂️🤦‍♀️", convert("🤦‍♂️🤦‍♀️\0🤷‍♂️🤷‍♀️"));
    }
}

#[cfg(test)]
#[rusty_fork::fork_test(timeout_ms = 60000)]
#[should_panic]
fn catch_panic() {
    catch(|| panic!("test"));
}
