use anyhow::Result;
use sentry::{BeforeSend, Event, Options, Value};
use sentry_contrib_native as sentry;
use std::{thread, time::Duration};

static mut BLUBB: usize = 0;

fn main() -> Result<()> {
    struct Filter;

    impl BeforeSend for Filter {
        fn before_send(&mut self, value: Value) -> Value {
            unsafe { BLUBB += 1 };
            value
        }
    }

    let mut options = Options::new();
    options.set_before_send(|value| {
        unsafe { BLUBB += 1 };
        value
    });
    let _shutdown = options.init()?;

    Event::new().capture();

    thread::sleep(Duration::from_secs(5));

    println!("BLUBB: {}", unsafe { BLUBB });

    Ok(())
}
