#![warn(
    clippy::all,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs,
    rustdoc
)]

//! TODO

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
            "cargo:warning=not compiling with debuginfo, sentry won't have source code access"
        );
    }

    if !installed {
        build(&out_dir, &source, &install)?;
    }

    println!("cargo:rustc-link-lib=sentry");
    println!("cargo:rustc-link-lib=crashpad_client");
    println!("cargo:rustc-link-lib=crashpad_util");
    println!("cargo:rustc-link-lib=mini_chromium");
    println!("cargo:rustc-link-search={}", install.join("lib").display());

    #[cfg(windows)]
    {
        println!("cargo:rustc-link-lib=dbghelp");
        println!("cargo:rustc-link-lib=shlwapi");
        println!("cargo:rustc-link-lib=winhttp");
    }

    #[cfg(windows)]
    let handler = "crashpad_handler.exe";
    #[cfg(not(windows))]
    let handler = "crashpad_handler";

    println!(
        "cargo:HANDLER={}",
        install.join("bin").join(handler).display()
    );

    Ok(())
}

/// Build `sentry_native` with CMAKE.
fn build(out_dir: &Path, source: &Path, install: &Path) -> Result<()> {
    if !Command::new("cmake")
        .arg("--version")
        .status()
        .context("cmake command not found")?
        .success()
    {
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
        bail!("cmake configuration error");
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
