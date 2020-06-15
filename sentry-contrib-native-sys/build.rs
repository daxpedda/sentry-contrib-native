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
//! - Exports path to `crashpad_handler(.exe)` as `DEP_SENTRY_NATIVE_HANDLER`.
//! - Links appropriate libraries.

use anyhow::Result;
use cmake::Config;
use std::{
    env,
    path::{Path, PathBuf},
};

fn main() -> Result<()> {
    // path to source.
    let source = PathBuf::from("sentry-native");
    // path to installation
    let install = if let Some(install) = env::var_os("SENTRY_NATIVE_INSTALL").map(PathBuf::from) {
        install
    } else {
        build(&source)?
    };

    println!("cargo:rerun-if-env-changed=SENTRY_NATIVE_INSTALL");

    if env::var("DEBUG")? == "false" {
        println!(
            "cargo:warning=not compiling with debug information, Sentry won't have source code access"
        );
    }

    println!("cargo:rustc-link-search={}", install.join("lib").display());
    println!("cargo:rustc-link-lib=sentry");

    match env::var("CARGO_CFG_TARGET_OS")
        .expect("target OS not specified")
        .as_str()
    {
        crashpad if crashpad == "windows" || crashpad == "macos" => {
            println!("cargo:rustc-link-lib=crashpad_client");
            println!("cargo:rustc-link-lib=crashpad_util");
            println!("cargo:rustc-link-lib=mini_chromium");

            let handler = if crashpad == "windows" {
                println!("cargo:rustc-link-lib=dbghelp");
                println!("cargo:rustc-link-lib=shlwapi");

                if cfg!(feature = "default-transport") {
                    println!("cargo:rustc-link-lib=winhttp");
                }

                "crashpad_handler.exe"
            } else {
                "crashpad_handler"
            };

            println!(
                "cargo:HANDLER={}",
                install.join("bin").join(handler).display()
            );
        }
        "linux" => {
            if cfg!(feature = "default-transport") {
                println!("cargo:rustc-link-lib=curl");
            }

            println!("cargo:rustc-link-lib=breakpad_client");
        }
        other => unimplemented!("target platform {} not implemented", other),
    }

    Ok(())
}

/// Build `sentry_native` with CMake.
fn build(source: &Path) -> Result<PathBuf> {
    let mut cmake_config = Config::new(source);
    cmake_config
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("SENTRY_BUILD_TESTS", "OFF")
        .define("SENTRY_BUILD_EXAMPLES", "OFF")
        .profile("RelWithDebInfo");

    if cfg!(not(feature = "default-transport")) {
        cmake_config.define("SENTRY_TRANSPORT", "none");
    }

    if cfg!(target_feature = "crt-static") {
        cmake_config.define("SENTRY_BUILD_RUNTIMESTATIC", "ON");
    }

    Ok(cmake_config.build())
}
