#![warn(
    clippy::all,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]
// stable clippy seems to have an issue with await
#![allow(clippy::used_underscore_binding)]

mod util;

use anyhow::Result;
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[tokio::test(threaded_scheduler)]
async fn panic() -> Result<()> {
    let mut panic_test = PathBuf::from(env!("OUT_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap()
        .join("examples");

    #[cfg(not(target_os = "windows"))]
    {
        panic_test = panic_test.join("panic");
    }
    #[cfg(target_os = "windows")]
    {
        panic_test = panic_test.join("panic.exe");
    }

    let id: [u8; 16] = rand::random();
    let user_id = hex::encode(id);
    let mut child = Command::new(panic_test)
        .stdin(Stdio::piped())
        .spawn()
        .expect("make sure to build the panic example first!");
    child.stdin.as_mut().unwrap().write_all(&id)?;

    assert!(!child.wait()?.success());

    util::user_id(user_id).await
}
