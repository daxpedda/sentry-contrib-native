use anyhow::Result;
use sentry::{Event, Map};
use sentry_contrib_native as sentry;

fn main() -> Result<()> {
    let mut event = Event::new();
    event.add_exception(Map::new(), 0);
    println!("{:#?}", event);

    Ok(())
}
