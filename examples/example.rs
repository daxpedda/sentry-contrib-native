#![warn(
    clippy::all,
    clippy::missing_docs_in_private_items,
    clippy::nursery,
    clippy::pedantic,
    missing_docs
)]

//! Example on how to use [`sentry-contrib-native`].

use anyhow::{bail, Result};
use sentry::{Consent, Event, Level, Options};
use sentry_contrib_native as sentry;
use std::panic;

fn main() -> Result<()> {
    // set up panic hooks to send an event to the Sentry service
    sentry::set_hook(None, Some(panic::take_hook()));

    let mut options = Options::new();
    // TODO: fill out more options
    // if we want to see some logging
    options.set_logger(|level, message| {
        println!("{:<9} {}", format!("[{}]", level), message);
    });
    options.set_dsn("https://abcdef1234567890abcdef1234567890@o0.ingest.sentry.io/0");
    options.set_require_user_consent(true);
    let _shutdown = options.init()?;

    if ask_user_for_consent() {
        sentry::set_user_consent(Consent::Given);
    } else {
        sentry::set_user_consent(Consent::Revoked);
    }

    // TODO: use extra, context and so on

    if let Err(error) = function_that_can_go_wrong() {
        // something went wrong, let's upload it to Sentry
        let mut event = Event::new_message(Level::Error, None, error.to_string());
        // let's add a stacktrace
        event.add_stacktrace(0);
        // send that event!
        event.capture();
    }

    Ok(())
}

/// Potentially something in a settings menu that asks the user for consent to
/// upload crash or logging reports.
const fn ask_user_for_consent() -> bool {
    true
}

/// Let's say you have a function that can return an [`Err`].
fn function_that_can_go_wrong() -> Result<()> {
    // let's pretend something can go wrong here
    bail!("Oh no! Something (something) went wrong!")
}
