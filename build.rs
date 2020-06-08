use std::{env, path::Path};

fn main() {
    println!(
        "cargo:rustc-env=HANDLER={}",
        AsRef::<Path>::as_ref(&env::var_os("DEP_SENTRY_NATIVE_HANDLER").unwrap()).display()
    );
}
