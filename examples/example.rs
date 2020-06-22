use anyhow::Result;
use sentry::Options;
use sentry_contrib_native as sentry;
use std::{thread, time::Duration};

fn main() -> Result<()> {
    let mut options = Options::new();
    options.set_debug(true);
    options.set_logger(|level, message| {
        println!("{:<9} {}", format!("[{}]", level), message);
    });
    let _shutdown = options.init()?;

    thread::sleep(Duration::from_secs(2));

    Ok(())
}
