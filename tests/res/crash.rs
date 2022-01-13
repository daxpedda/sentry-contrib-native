#[cfg(feature = "transport-custom")]
#[path = "../util/custom_transport.rs"]
#[rustfmt::skip]
mod custom_transport;

use anyhow::Result;
#[cfg(feature = "transport-custom")]
use custom_transport::Transport;
use libloading::{Library, Symbol};
use sentry::{Breadcrumb, Options, User};
use sentry_contrib_native as sentry;
use std::{
    io::{self, Read},
    ptr,
};

#[tokio::main]
async fn main() -> Result<()> {
    sentry::set_hook(None, None);

    let mut options = Options::new();
    options.set_debug(true);
    options.set_release("1.0");
    options.set_environment("production");
    options.set_distribution("release-pgo");
    options.add_attachment("tests/res/attachment.txt");
    #[cfg(feature = "transport-custom")]
    options.set_transport(Transport::new);
    let _shutdown = options.init()?;

    // breadcrumb
    Breadcrumb::new(Some("test type".into()), Some("test message".into())).add();

    // dylib
    let lib_location = dylib::location();
    let lib = unsafe { Library::new(&lib_location) }.unwrap();
    sentry::clear_modulecache();
    assert!(sentry::modules_list().contains(&lib_location.to_str().unwrap().to_string()));
    let func: Symbol<extern "C" fn() -> bool> = unsafe { lib.get(b"test\0") }.unwrap();
    assert!(func());

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

    #[allow(deref_nullptr)]
    unsafe {
        *ptr::null_mut() = true;
    }

    Ok(())
}
