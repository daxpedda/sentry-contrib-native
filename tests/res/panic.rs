use anyhow::Result;
use sentry::{Options, User};
use sentry_contrib_native as sentry;
use std::io::{self, Read};

fn main() -> Result<()> {
    sentry::set_hook(None, None);
    let _shutdown = Options::new().init()?;

    let mut buffer = [0; 16];
    io::stdin().read_exact(&mut buffer)?;
    let id = hex::encode(buffer);

    let mut user = User::new();
    user.insert("id", id);
    user.set();

    panic!("test panic")
}
