use anyhow::Result;
use sentry::{BeforeSend, Event, Options, Value};
use sentry_contrib_native as sentry;
use std::{thread, time::Duration};

fn main() -> Result<()> {
    struct Filter {
        counter: usize,
    }

    impl BeforeSend for Filter {
        fn before_send(&mut self, value: Value) -> Value {
            self.counter += 1;
            value
        }
    }

    impl Drop for Filter {
        fn drop(&mut self) {
            println!("BLUBB: {}", self.counter)
        }
    }

    let mut options = Options::new();
    options.set_before_send(Filter { counter: 0 });
    let _shutdown = options.init()?;

    Event::new().capture();
    Event::new().capture();
    Event::new().capture();

    thread::sleep(Duration::from_secs(5));

    Ok(())
}
