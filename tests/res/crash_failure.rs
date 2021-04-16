use anyhow::Result;
use sentry::{Consent, Options, User};
use sentry_contrib_native as sentry;
use std::{
    io::{self, Read},
    ptr,
};

#[cfg(feature = "transport-custom")]
#[path = "../util/custom_transport.rs"]
#[rustfmt::skip]
mod custom_transport;

#[cfg(feature = "transport-custom")]
use custom_transport::Transport;

#[tokio::main]
async fn main() -> Result<()> {
    sentry::set_hook(None, None);

    let mut options = Options::new();
    options.set_debug(true);
    options.set_require_user_consent(true);
    #[cfg(feature = "transport-custom")]
    options.set_transport(Transport::new);
    let _shutdown = options.init()?;

    sentry::set_user_consent(Consent::Revoked);

    let mut buffer = [0; 16];
    io::stdin().read_exact(&mut buffer)?;
    let id = hex::encode(buffer);

    let mut user = User::new();
    user.insert("id", id);
    user.set();

    #[allow(deref_nullptr)]
    unsafe {
        *ptr::null_mut() = true;
    }

    Ok(())
}
