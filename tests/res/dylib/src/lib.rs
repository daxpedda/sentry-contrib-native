use std::{
    env,
    path::{Path, PathBuf},
};

#[no_mangle]
pub extern "C" fn test() -> bool {
    true
}

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
