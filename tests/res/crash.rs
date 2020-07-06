#[cfg(feature = "custom-transport")]
#[path = "../util/custom_transport.rs"]
#[rustfmt::skip]
mod custom_transport;

use anyhow::Result;
#[cfg(feature = "custom-transport")]
use custom_transport::Transport;
use libloading::{Library, Symbol};
use sentry::{Breadcrumb, Options, User};
use sentry_contrib_native as sentry;
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
    ptr,
};

fn lib_path() -> PathBuf {
    let mut path = PathBuf::from(env!("OUT_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .unwrap()
        .join("deps");

    #[cfg(target_os = "linux")]
    {
        path = path.join("libdylib.so");
    }
    #[cfg(target_os = "macos")]
    {
        path = path.join("libdylib.dylib");
    }
    #[cfg(target_os = "windows")]
    {
        path = path.join("dylib.dll");
    }

    path
}

#[tokio::main(threaded_scheduler)]
async fn main() -> Result<()> {
    sentry::set_hook(None, None);

    let mut options = Options::new();
    options.set_debug(true);
    options.set_release("1.0");
    options.set_environment("production");
    options.set_distribution("release-pgo");
    /*options.add_attachment(
        "test attachment",
        "C:/rust/sentry-contrib-native/tests/res/attachment.txt",
    );*/
    #[cfg(feature = "custom-transport")]
    options.set_transport(Transport::new);
    let _shutdown = options.init()?;

    // breadcrumb
    Breadcrumb::new(Some("test type".into()), Some("test message".into())).add();

    // dylib
    let lib = Library::new(lib_path()).unwrap();
    let func: Symbol<extern "C" fn() -> bool> = unsafe { lib.get(b"test\0") }.unwrap();
    assert_eq!(true, func());

    // tag
    sentry::set_tag("test-tag", "test");

    // extra
    sentry::set_extra("test tag", "test");

    // context
    sentry::set_context("test context", vec![("type", "os"), ("name", "Redox")]);

    // transaction
    sentry::set_transaction("test transaction");

    // user
    {
        let mut buffer = [0; 16];
        io::stdin().read_exact(&mut buffer)?;
        let id = hex::encode(buffer);

        let mut user = User::new();
        user.insert("id", id);
        user.set();
    }

    unsafe {
        *ptr::null_mut() = true;
    }

    Ok(())
}
