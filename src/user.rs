//! Sentry user implementation.

use crate::{global_write, Sealed};

/// A Sentry user.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Object, User};
/// let mut user = User::new();
/// user.insert("id", 1);
/// user.set();
/// ```
pub struct User(Option<sys::Value>);

impl Default for User {
    fn default() -> Self {
        Self::new()
    }
}

derive_object!(User);

impl User {
    /// Creates a new user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::User;
    /// let mut user = User::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self(Some(unsafe { sys::value_new_object() }))
    }

    /// Sets the specified user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Object, User};
    /// let mut user = User::new();
    /// user.insert("id", 1);
    /// user.set();
    /// ```
    pub fn set(self) {
        let user = self.take();

        {
            let _lock = global_write();
            unsafe { sys::set_user(user) };
        }
    }
}

#[test]
fn threaded_stress() {
    use crate::Object;
    use std::thread;

    let mut spawns = Vec::new();

    spawns.push(thread::spawn(|| {
        let mut handles = Vec::new();

        for index in 0..100 {
            handles.push(thread::spawn(move || {
                let mut user = User::new();
                user.insert("id", index);
                user.set();
            }))
        }

        handles
    }));

    spawns.push(thread::spawn(|| {
        let mut handles = Vec::new();

        for _ in 0..100 {
            handles.push(thread::spawn(move || {
                crate::remove_user();
            }))
        }

        handles
    }));

    for handles in spawns {
        for handle in handles.join().unwrap() {
            handle.join().unwrap();
        }
    }
}
