use anyhow::Result;
use sentry::{Event, Map, Options};
use sentry_contrib_native as sentry;
use std::ptr;

fn main() -> Result<()> {
    let mut options = Options::new();
    options.set_before_send(|value| std::fs::write("blubb.txt", format!("{:?}", value)));
    let _shutdown = Options::new().init()?;

    Event::new().capture();

    std::thread::sleep(std::time::Duration::from_secs(5));

    Ok(())
}
