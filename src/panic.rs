use crate::{Event, Level, Map, Object};
use std::{convert::TryFrom, panic};

/// Our Panichandler
pub fn set_hook() {
    panic::set_hook(Box::new(|panic_info| {
        let mut event = Event::new_message(
            Level::Error,
            Some("rust panic".into()),
            panic_info.to_string(),
        );

        if let Some(location) = panic_info.location() {
            let mut extra = Map::new();
            extra.insert("file", location.file());

            if let Ok(line) = i32::try_from(location.line()) {
                extra.insert("line", line);
            }

            if let Ok(column) = i32::try_from(location.column()) {
                extra.insert("column", column);
            }

            event.insert("extra", extra);
        }

        event.value_add_stacktrace(0);
        event.capture();
    }));
}

#[cfg(test)]
mod test {
    use crate::{panic::set_hook, Options};
    use rusty_fork::test_fork;

    #[test_fork]
    #[should_panic]
    fn hook() {
        set_hook();

        let _shutdown = Options::new().init().unwrap();
        panic!("this panic is a test");
    }
}
