#[cfg(not(windows))]
use std::os::unix::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::{
    ffi::{CStr, CString, OsStr},
    os::raw::c_char,
    path::Path,
};

#[cfg(windows)]
type COsString = u16;
#[cfg(not(windows))]
type COsString = u8;

pub trait CPath {
    fn to_vec(&self) -> Vec<COsString>;
}

impl CPath for Path {
    fn to_vec(&self) -> Vec<COsString> {
        // ␀
        #[cfg(windows)]
        let null_string = OsStr::new("\u{2400}").encode_wide();
        #[cfg(not(windows))]
        let null_string = OsStr::new("\u{2400}").as_bytes();

        #[cfg(windows)]
        let path = self.as_os_str().encode_wide();
        #[cfg(not(windows))]
        let path = self.as_os_str().as_bytes().iter().copied();
        let mut clean_string = Vec::new();

        for char in path {
            if char == 0 {
                clean_string.extend(null_string.clone())
            } else {
                clean_string.push(char)
            }
        }

        clean_string.push(0);

        clean_string
    }
}

pub trait CToR {
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
