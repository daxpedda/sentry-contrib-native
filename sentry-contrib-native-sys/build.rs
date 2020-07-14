#![warn(
    clippy::all,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]

//!
//! - Warns if debug information isn't enabled.
//! - Looks for `SENTRY_NATIVE_INSTALL`.
//! - If `SENTRY_NATIVE_INSTALL` isn't found, compiles `sentry-native` for you.
//! - Exports path to `crashpad_handler(.exe)` as
//!   `DEP_SENTRY_NATIVE_CRASHPAD_HANDLER`.
//! - Links appropriate libraries.

use anyhow::Result;
use cmake::Config;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Copy, Clone)]
enum Backend {
    Crashpad,
    Breakpad,
    InProc,
    None,
}

impl AsRef<str> for Backend {
    fn as_ref(&self) -> &str {
        match self {
            Backend::Crashpad => "crashpad",
            Backend::Breakpad => "breakpad",
            Backend::InProc => "inproc",
            Backend::None => "none",
        }
    }
}

impl Backend {
    /// Gets the backend we want to use, see https://github.com/getsentry/sentry-native#compile-time-options
    /// for details.
    fn new(target_os: &str) -> Backend {
        if cfg!(feature = "backend-crashpad") && (target_os == "macos" || target_os == "windows") {
            Backend::Crashpad
        } else if cfg!(feature = "backend-breakpad") && target_os == "linux" {
            Backend::Breakpad
        } else if cfg!(feature = "backend-inproc") {
            Backend::InProc
        } else if cfg!(feature = "backend-default") {
            match target_os {
                "windows" | "macos" => Backend::Crashpad,
                "linux" => Backend::Breakpad,
                "android" => Backend::InProc,
                _ => Backend::None,
            }
        } else {
            Backend::None
        }
    }
}

fn main() -> Result<()> {
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("target OS not specified");
    let backend = Backend::new(&target_os);

    // path to source.
    let source = PathBuf::from("sentry-native");

    // path to installation or to install to
    let install = if let Some(install) = env::var_os("SENTRY_NATIVE_INSTALL").map(PathBuf::from) {
        if fs::read_dir(&install)
            .ok()
            .and_then(|mut dir| dir.next())
            .is_none()
        {
            build(&source, Some(&install), backend)?
        } else {
            install
        }
    } else {
        build(&source, None, backend)?
    };

    println!("cargo:rerun-if-env-changed=SENTRY_NATIVE_INSTALL");

    if env::var("DEBUG")? == "false" {
        println!(
            "cargo:warning=not compiling with debug information, Sentry won't have source code access"
        );
    }

    // We need to check if there is a `lib64` instead of a `lib` dir, non-Debian
    // based distros will use that directory instead for 64-bit arches
    // See: https://cmake.org/cmake/help/v3.0/module/GNUInstallDirs.html
    let lib_dir = if install.join("lib64").exists() {
        "lib64"
    } else {
        "lib"
    };

    println!(
        "cargo:rustc-link-search={}",
        install.join(lib_dir).display()
    );
    println!("cargo:rustc-link-lib=sentry");

    match backend {
        Backend::Crashpad => {
            println!("cargo:rustc-link-lib=crashpad_client");
            println!("cargo:rustc-link-lib=crashpad_util");
            println!("cargo:rustc-link-lib=mini_chromium");

            let handler = if target_os == "windows" {
                "crashpad_handler.exe"
            } else {
                "crashpad_handler"
            };

            println!(
                "cargo:CRASHPAD_HANDLER={}",
                install.join("bin").join(handler).display()
            );
        }
        Backend::Breakpad => {
            println!("cargo:rustc-link-lib=breakpad_client");
        }
        Backend::InProc | Backend::None => {}
    }

    match target_os.as_str() {
        "windows" => {
            if cfg!(feature = "default-transport") {
                println!("cargo:rustc-link-lib=winhttp");
            }

            println!("cargo:rustc-link-lib=dbghelp");
            println!("cargo:rustc-link-lib=shlwapi");
        }
        "macos" => {
            if cfg!(feature = "default-transport") {
                println!("cargo:rustc-link-lib=curl");
            }

            println!("cargo:rustc-link-lib=framework=Foundation");
            println!("cargo:rustc-link-lib=dylib=c++");
        }
        "linux" => {
            if cfg!(feature = "default-transport") {
                println!("cargo:rustc-link-lib=curl");
            }

            println!("cargo:rustc-link-lib=dylib=stdc++");
        }
        "android" => {}
        other => unimplemented!("target platform {} not implemented", other),
    }

    Ok(())
}

/// Build `sentry_native` with `CMake`.
fn build(source: &Path, install: Option<&Path>, backend: Backend) -> Result<PathBuf> {
    let mut cmake_config = Config::new(source);
    cmake_config
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("SENTRY_BUILD_TESTS", "OFF")
        .define("SENTRY_BUILD_EXAMPLES", "OFF")
        .profile("RelWithDebInfo");

    if let Some(install) = install {
        fs::create_dir_all(install).expect("failed to create install directory");
        cmake_config.out_dir(install);
    }

    if cfg!(not(feature = "default-transport")) {
        cmake_config.define("SENTRY_TRANSPORT", "none");
    }

    cmake_config.define("SENTRY_BACKEND", backend.as_ref());

    if cfg!(target_feature = "crt-static") {
        cmake_config.define("SENTRY_BUILD_RUNTIMESTATIC", "ON");
    }

    Ok(cmake_config.build())
}
