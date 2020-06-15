#![warn(
    clippy::all,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]

//! Re-export path to the crashpad handler to the crate.
//! This is only used during testing.

use std::{env, path::Path};

fn main() {
    let target_os = env::var_os("CARGO_CFG_TARGET_OS").unwrap();

    if target_os == "windows" || target_os == "macos" {
        println!(
            "cargo:rustc-env=HANDLER={}",
            AsRef::<Path>::as_ref(&env::var_os("DEP_SENTRY_NATIVE_HANDLER").unwrap()).display()
        );
    }
}
