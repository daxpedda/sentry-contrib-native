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

use anyhow::{bail, Context, Result};
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() -> Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    // Path to source.
    let source = PathBuf::from("sentry-native");
    // Path to installation; either user-defined or path we will compile to.
    let (installed, install) =
        if let Some(install) = env::var("SENTRY_NATIVE_INSTALL").ok().map(PathBuf::from) {
            (true, install)
        } else {
            (false, out_dir.join("install"))
        };

    println!("cargo:rerun-if-env-changed=SENTRY_NATIVE_INSTALL");

    if env::var("DEBUG")? == "false" {
        println!(
            "cargo:warning=not compiling with debug information, Sentry won't have source code access"
        );
    }

    if !installed {
        build(&out_dir, &source, &install)?;
    }

    println!("cargo:rustc-link-search={}", install.join("lib").display());
    println!("cargo:rustc-link-lib=sentry");
    println!("cargo:rustc-link-lib=mini_chromium");

    let target_os = env::var_os("CARGO_CFG_TARGET_OS").unwrap();

    if target_os == "windows" {
        println!("cargo:rustc-link-lib=dbghelp");
        println!("cargo:rustc-link-lib=shlwapi");
        println!("cargo:rustc-link-lib=winhttp");
    }

    if target_os == "windows" || target_os == "macos" {
        println!("cargo:rustc-link-lib=crashpad_client");
        println!("cargo:rustc-link-lib=crashpad_util");

        let handler = if target_os == "windows" {
            "crashpad_handler.exe"
        } else {
            "crashpad_handler"
        };

        println!(
            "cargo:HANDLER={}",
            install.join("bin").join(handler).display()
        );
    }

    Ok(())
}

/// Build `sentry_native` with CMake.
fn build(out_dir: &Path, source: &Path, install: &Path) -> Result<()> {
    if !Command::new("cmake").arg("--version").status()?.success() {
        bail!("cmake command not found");
    }

    let build = out_dir
        .join("build")
        .to_str()
        .context("failed to parse path")?
        .to_owned();

    if !Command::new("cmake")
        .current_dir(source)
        .args(&["-B", &build, "-D", "BUILD_SHARED_LIBS=OFF"])
        .status()?
        .success()
    {
        bail!("CMake configuration error");
    }

    if !Command::new("cmake")
        .current_dir(source)
        .args(&[
            "--build",
            &build,
            "--parallel",
            "--config",
            "RelWithDebInfo",
        ])
        .status()?
        .success()
    {
        bail!("build error");
    }

    if !Command::new("cmake")
        .current_dir(source)
        .args(&["--install", &build, "--prefix"])
        .arg(install)
        .args(&["--config", "RelWithDebInfo"])
        .status()?
        .success()
    {
        bail!("install error");
    }

    Ok(())
}
