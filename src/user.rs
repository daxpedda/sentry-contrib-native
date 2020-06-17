//! Sentry user implementation.

use crate::{Object, Sealed, SentryString, GLOBAL_LOCK};
use std::net::SocketAddr;

/// A Sentry user.
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
    /// # use sentry_contrib_native::{User, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut user = User::new();
    /// user.set_id("1");
    /// user.set();
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self(Some(unsafe { sys::value_new_object() }))
    }

    /// Sets the id of the user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{User, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut user = User::new();
    /// user.set_id("1");
    /// user.set();
    /// # Ok(()) }
    /// ```
    pub fn set_id<S: Into<SentryString>>(&mut self, id: S) {
        self.insert("id", id.into())
    }

    /// Sets the username of the user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{User, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut user = User::new();
    /// user.set_id("1");
    /// user.set_username("test");
    /// user.set();
    /// # Ok(()) }
    /// ```
    pub fn set_username<S: Into<SentryString>>(&mut self, username: S) {
        self.insert("username", username.into())
    }

    /// Sets the email of the user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{User, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut user = User::new();
    /// user.set_id("1");
    /// user.set_email("example@test.org");
    /// user.set();
    /// # Ok(()) }
    /// ```
    pub fn set_email<S: Into<SentryString>>(&mut self, email: S) {
        self.insert("email", email.into())
    }

    /// Sets the IP address of the user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{User, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut user = User::new();
    /// user.set_id("1");
    /// user.set_ip(([1, 1, 1, 1], 443));
    /// user.set();
    /// # Ok(()) }
    /// ```
    pub fn set_ip<IP: Into<SocketAddr>>(&mut self, ip: IP) {
        self.insert("ip", ip.into().to_string())
    }

    /// Sets the specified user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{User, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut user = User::new();
    /// user.set_id("1");
    /// user.set();
    /// # Ok(()) }
    /// ```
    pub fn set(self) {
        let user = self.take();

        {
            let _lock = GLOBAL_LOCK.write().expect("global lock poisoned");
            unsafe { sys::set_user(user) };
        }
    }
}
