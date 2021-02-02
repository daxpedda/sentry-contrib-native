#![warn(
    clippy::cargo,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]

//! This build script only exists to enable the `OUT_DIR` environment variable
//! inside the crate.

fn main() {}
