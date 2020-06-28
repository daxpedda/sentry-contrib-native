//! Sentry user implementation.

use crate::{global_write, Object, Value};
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
/// user.insert("id".into(), 1.into());
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

    /// Sets the specified user.
    ///
    /// # Panics
    /// Panics if any [`String`] contains a null byte.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::User;
    /// let mut user = User::new();
    /// user.insert("id".into(), 1.into());
    /// user.set();
    /// ```
    pub fn set(self) {
        let user = self.into_raw();

        {
            let _lock = global_write();
            unsafe { sys::set_user(user) };
        }
    }
}

#[test]
fn user() {
    User::new().set()
}
