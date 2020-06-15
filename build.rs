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
    if let Ok(handler) = env::var("DEP_SENTRY_NATIVE_HANDLER") {
        if cfg!(feature = "copy-handler") {
            let out_dir = env::var("OUT_DIR").expect("out dir not set");

            // OUT_DIR will point to a directory unique to each crate, which be something like
            // target/debug/build/sentry-contrib-native-sys-f734ae671f48a2d5/out, so we go up
            // 3 parents to get to the root directory (target/debug in this case), which is
            // where the final binary artifacts will be placed by cargo, so we copy the
            // crashpad_handler to the same directory to fit with the default expectation the
            // handler is next to the executable it is monitoring, and so that scripts/programs
            // that want to package the crashpad_handler along with the executable don't have
            // to trawl through the target directory looking for it, as, AFAICT, there is no
            // convenient way to specify the output path of the handler where it is available
            // to eg other builds scripts, as noted by cargo
            //
            // > Note that metadata is only passed to immediate dependents, not transitive dependents.
            //
            // And we can't assume that this crate will be a direct dependency of the crate.
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

            std::fs::copy(&handler, &bin_path).expect("failed to copy sentry crash handler");
        }

        println!("cargo:rustc-env=SENTRY_HANDLER={}", handler);
    }
}
