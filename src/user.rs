//! Sentry user implementation.

use crate::{Object, Value};
use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};

/// A Sentry user.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::User;
/// let mut user = User::new();
/// user.insert("id", 1);
/// user.set();
/// ```
#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct User(BTreeMap<String, Value>);

impl Default for User {
    fn default() -> Self {
        Self::new()
    }
}

impl Object for User {
    fn into_parts(self) -> (sys::Value, BTreeMap<String, Value>) {
        (unsafe { sys::value_new_object() }, self.0)
    }
}

impl Deref for User {
    type Target = BTreeMap<String, Value>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for User {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl User {
    /// Creates a new user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::User;
    /// let mut user = User::new();
    /// ```
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Inserts a key-value pair into the [`User`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::User;
    /// let mut user = User::new();
    /// user.insert("id", 1);
    /// ```
    pub fn insert<S: Into<String>, V: Into<Value>>(&mut self, key: S, value: V) {
        self.deref_mut().insert(key.into(), value.into());
    }

    /// Sets the specified user.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::User;
    /// let mut user = User::new();
    /// user.insert("id", 1);
    /// user.set();
    /// ```
    pub fn set(self) {
        let user = self.into_raw();
        unsafe { sys::set_user(user) }
    }
}

#[test]
fn user() {
    User::new().set();

    let mut user = User::new();
    user.insert("test", "test");
    user.set();
}
