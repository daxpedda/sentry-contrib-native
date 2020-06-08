use crate::{Object, Sealed, SentryString, GLOBAL_LOCK};
use std::net::SocketAddr;

/// A sentry user.
pub struct User(Option<sys::Value>);

object_drop!(User);

impl Default for User {
    fn default() -> Self {
        Self::new()
    }
}

object_sealed!(User);
object_debug!(User);
object_clone!(User);
object_partial_eq!(User);
object_from_iterator!(User);
object_extend!(User);

impl User {
    /// Creates a new user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{User, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let user = User::new();
    /// user.set_id(1);
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
    /// # use sentry_native::{User, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let user = User::new();
    /// user.set_id(1);
    /// user.set();
    /// # Ok(()) }
    /// ```
    pub fn set_id(&self, id: i32) {
        self.insert("id", id)
    }

    /// Sets the username of the user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{User, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let user = User::new();
    /// user.set_id(1);
    /// user.set_username("test");
    /// user.set();
    /// # Ok(()) }
    /// ```
    pub fn set_username<S: Into<SentryString>>(&self, username: S) {
        self.insert("username", username.into())
    }

    /// Sets the email of the user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{User, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let user = User::new();
    /// user.set_id(1);
    /// user.set_email("example@test.org");
    /// user.set();
    /// # Ok(()) }
    /// ```
    pub fn set_email<S: Into<SentryString>>(&self, email: S) {
        self.insert("email", email.into())
    }

    /// Sets the email of the user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{User, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let user = User::new();
    /// user.set_id(1);
    /// user.set_ip(([1, 1, 1, 1], 443));
    /// user.set();
    /// # Ok(()) }
    /// ```
    pub fn set_ip<IP: Into<SocketAddr>>(&self, ip: IP) {
        self.insert("ip", &ip.into().to_string())
    }

    /// Sets the specified user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{User, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let user = User::new();
    /// user.set_id(1);
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
