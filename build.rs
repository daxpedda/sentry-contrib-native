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
    for (k, v) in env::vars() {
        println!("{} = {}", k, v);
    }

    if let Ok(p) = env::var("DEP_SENTRY_NATIVE_HANDLER") {
        println!("cargo:rustc-env=HANDLER={}", Path::new(&p).display(),);
    }
}
