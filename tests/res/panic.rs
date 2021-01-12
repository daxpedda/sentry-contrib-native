#[cfg(feature = "transport-custom")]
#[path = "../util/custom_transport.rs"]
#[rustfmt::skip]
mod custom_transport;

use anyhow::Result;
#[cfg(feature = "transport-custom")]
use custom_transport::Transport;
use sentry::{Options, User};
use sentry_contrib_native as sentry;
use std::io::{self, Read};

#[tokio::main]
async fn main() -> Result<()> {
    sentry::set_hook(None, None);

    let mut options = Options::new();
    options.set_debug(true);
    #[cfg(feature = "transport-custom")]
    options.set_transport(Transport::new);
    let _shutdown = options.init()?;

    let mut buffer = [0; 16];
    io::stdin().read_exact(&mut buffer)?;
    let id = hex::encode(buffer);

    let mut user = User::new();
    user.insert("id", id);
    user.set();

    panic!("test panic")
}
