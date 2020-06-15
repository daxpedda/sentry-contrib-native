//! FFI helper types to communicate with `sentry-native`.

#[cfg(not(windows))]
use std::os::unix::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::{
    ffi::{CStr, CString, OsStr},
    os::raw::c_char,
    path::Path,
};

/// Cross-platform return type for [`CPath::to_os_vec`].
#[cfg(windows)]
type COsString = u16;
/// Cross-platform return type for [`CPath::to_os_vec`].
#[cfg(not(windows))]
type COsString = i8;

/// Trait for converting [`Path`] to `Vec<COsString>`.
pub trait CPath {
    /// Re-encodes `self` into a C and OS compatible `Vec<COsString>`.
    ///
    /// This will replace any `0` bytes with `␀`.
    fn to_os_vec(&self) -> Vec<COsString>;
}

impl CPath for Path {
    fn to_os_vec(&self) -> Vec<COsString> {
        // ␀
        #[cfg(windows)]
        let null_string = OsStr::new("\u{2400}").encode_wide();
        #[cfg(not(windows))]
        let null_string = OsStr::new("\u{2400}")
            .as_bytes()
            .iter()
            .map(|ch| unsafe { &*(ch as *const _ as *const _) });

        #[cfg(windows)]
        let path = self.as_os_str().encode_wide();
        #[cfg(not(windows))]
        let path = self
            .as_os_str()
            .as_bytes()
            .iter()
            .map(|ch| unsafe { &*(ch as *const _ as *const _) })
            .copied();
        let mut clean_string = Vec::new();

        for ch in path {
            if ch == 0 {
                clean_string.extend(null_string.clone());
            } else {
                clean_string.push(ch);
            }
        }

        clean_string.push(0);

        clean_string
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
        ffi::{CStr, CString, OsStr, OsString},
        os::raw::c_char,
        path::Path,
        ptr,
    };

    #[test]
    fn cpath() {
        fn convert(string: &str) -> OsString {
            let path: &Path = OsStr::new(string).as_ref();
            let cpath = path.to_os_vec();

            #[cfg(windows)]
            {
                OsString::from_wide(&cpath)
            }
            #[cfg(not(windows))]
            {
                OsString::from_vec(
                    cpath
                        .iter()
                        .map(|ch| unsafe { &*(ch as *const _ as *const _) })
                        .copied()
                        .collect(),
                )
            }
        }

        assert_eq!("abcdefgh\0", convert("abcdefgh"));
        assert_eq!("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️\0", convert("🤦‍♂️🤦‍♀️🤷‍♂️🤷‍♀️"));

        assert_eq!("␀\0", convert("\0"));
        assert_eq!("␀␀\0", convert("\0\0"));
        assert_eq!("␀\0", convert("␀"));
        assert_eq!("␀␀\0", convert("␀␀"));

        assert_eq!("abcd␀efgh\0", convert("abcd\0efgh"));
        assert_eq!("abcd␀␀efgh\0", convert("abcd\0\0efgh"));
        assert_eq!("abc🤦‍♂️␀🤦‍♂️fgh\0", convert("abc🤦‍♂️\0🤦‍♂️fgh"));
        assert_eq!("abc🤦‍♂️␀␀🤦‍♂️fgh\0", convert("abc🤦‍♂️\0\0🤦‍♂️fgh"));

        assert_eq!("␀abcdefgh\0", convert("\0abcdefgh"));
        assert_eq!("␀␀abcdefgh\0", convert("\0\0abcdefgh"));
        assert_eq!("␀🤦‍♂️bcdefgh\0", convert("\0🤦‍♂️bcdefgh"));
        assert_eq!("␀␀🤦‍♂️bcdefgh\0", convert("\0\0🤦‍♂️bcdefgh"));

        assert_eq!("abcdefgh␀\0", convert("abcdefgh\0"));
        assert_eq!("abcdefgh␀␀\0", convert("abcdefgh\0\0"));
        assert_eq!("abcdefg🤦‍♂️␀\0", convert("abcdefg🤦‍♂️\0"));
        assert_eq!("abcdefg🤦‍♂️␀␀\0", convert("abcdefg🤦‍♂️\0\0"));

        assert_eq!("␀a␀\0", convert("\0a\0"));
        assert_eq!("␀␀a␀␀\0", convert("\0\0a\0\0"));
        assert_eq!("␀🤦‍♂️␀\0", convert("\0🤦‍♂️\0"));
        assert_eq!("␀␀🤦‍♂️␀␀\0", convert("\0\0🤦‍♂️\0\0"));

        assert_eq!("a␀a␀\0", convert("a\0a\0"));
        assert_eq!("a␀␀a␀␀\0", convert("a\0\0a\0\0"));
        assert_eq!("🤦‍♂️␀🤦‍♂️␀\0", convert("🤦‍♂️\0🤦‍♂️\0"));
        assert_eq!("🤦‍♂️␀␀🤦‍♂️␀␀\0", convert("🤦‍♂️\0\0🤦‍♂️\0\0"));

        assert_eq!("␀a␀a\0", convert("\0a\0a"));
        assert_eq!("␀␀a␀␀a\0", convert("\0\0a\0\0a"));
        assert_eq!("␀🤦‍♂️␀🤦‍♂️\0", convert("\0🤦‍♂️\0🤦‍♂️"));
        assert_eq!("␀␀🤦‍♂️␀␀🤦‍♂️\0", convert("\0\0🤦‍♂️\0\0🤦‍♂️"));
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
