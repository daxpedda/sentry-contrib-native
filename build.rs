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

    if let Ok(handler) = env::var("DEP_SENTRY_NATIVE_HANDLER") {
        if cfg!(feature = "copy-handler") {
            let out_dir = env::var("OUT_DIR").expect("out dir not set");

            let out_dir = Path::new(&out_dir);
            let bin_dir = out_dir
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap();

            let handler = Path::new(&handler);
            let bin_path = bin_dir.join(
                handler
                    .file_name()
                    .expect("handler doesn't have a file name"),
            );

            println!(
                "cargo:warning=\"Copying {} to {}\"",
                handler.display(),
                bin_path.display()
            );
            std::fs::copy(&handler, &bin_path).expect("failed to copy sentry crash handler");
        } else {
            println!("cargo:rustc-env=SENTRY_HANDLER={}", handler);
        }
    }
}
