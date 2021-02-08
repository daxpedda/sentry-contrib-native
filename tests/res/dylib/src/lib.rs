#![warn(
    clippy::cargo,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]

//! Crate to test if sentry is correctly detecting dynamically loaded libraries.

use std::{
    env,
    path::{Path, PathBuf},
};

/// Simple test function to make sure the library has been correctly loaded.
#[allow(clippy::missing_const_for_fn)]
#[no_mangle]
pub extern "C" fn test() -> bool {
    true
}

/// Helper function to determine the location of the dynamic library.
///
/// # Panics
/// Panics if path is invalid.
#[must_use]
pub fn location() -> PathBuf {
    let mut path = PathBuf::from(env!("OUT_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap()
        .join("deps");

    #[cfg(target_os = "linux")]
    {
        path = path.join("libdylib.so");
    }
    #[cfg(target_os = "macos")]
    {
        path = path.join("libdylib.dylib");
    }
    #[cfg(target_os = "windows")]
    {
        path = path.join("dylib.dll");
    }

    path
}
