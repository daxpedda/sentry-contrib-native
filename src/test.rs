//! Test helper implementations.

use std::{
    panic,
    sync::atomic::{AtomicBool, Ordering},
};

/// This global stores if a panic already happened.
static PANICKED: AtomicBool = AtomicBool::new(false);

/// Call this at the start of a test to record.
pub fn set_hook() {
    let hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        PANICKED.store(true, Ordering::SeqCst);
        hook(panic_info);
    }));
}

/// Call this at the end of a test to check if a thread panicked, if it did,
/// panic!
///
/// # Panics
/// Panics in the main thread if a panic occured in another thread.
pub fn verify_panics() {
    assert!(!PANICKED.load(Ordering::SeqCst), "panicked in thread");
}
