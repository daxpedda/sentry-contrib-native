use crate::{Error, Value};
use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    iter::FromIterator,
};

/// A sentry list value.
pub struct List(Option<sys::Value>);

impl Drop for List {
    fn drop(&mut self) {
        if let Some(value) = self.0.take() {
            unsafe { sys::value_decref(value) };
        }
    }
}

impl Debug for List {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_list()
            .entries(self.to_vec().iter())
            .finish()
    }
}

impl Clone for List {
    fn clone(&self) -> Self {
        let self_raw = self.unwrap();
        let list = Self::new();
        let list_raw = list.unwrap();

        for index in 0..self.get_length() {
            match unsafe {
                sys::value_append(list_raw, sys::value_get_by_index_owned(self_raw, index))
            } {
                0 => (),
                _ => panic!("sentry failed to allocate memory"),
            }
        }

        list
    }
}

impl PartialEq for List {
    fn eq(&self, other: &Self) -> bool {
        self.to_vec() == other.to_vec()
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}

impl<V: Into<Value>> FromIterator<V> for List {
    fn from_iter<I: IntoIterator<Item = V>>(iter: I) -> Self {
        let list = Self::new();

        for item in iter {
            list.push(item);
        }

        list
    }
}

impl Extend<Value> for List {
    fn extend<T: IntoIterator<Item = Value>>(&mut self, iter: T) {
        for value in iter {
            self.push(value);
        }
    }
}

impl List {
    /// Creates a new, empty list value.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{Event, List, Object};
    /// # fn main() -> anyhow::Result<()> {
    /// let event = Event::new();
    /// let list = List::new();
    /// list.push(true);
    /// event.insert("test", list);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self(Some(unsafe { sys::value_new_list() }))
    }

    pub(crate) unsafe fn from_raw(value: sys::Value) -> Self {
        Self(Some(value))
    }

    fn unwrap(&self) -> sys::Value {
        self.0.expect("use after free")
    }

    pub(crate) fn take(mut self) -> sys::Value {
        self.0.take().expect("use after free")
    }

    /// Converts a [`List`] to a [`Vec`].
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{Event, List, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let list = List::new();
    /// list.push(true);
    /// list.push(false);
    /// assert_eq!(vec![Value::Bool(true), Value::Bool(false)], list.to_vec());
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn to_vec(&self) -> Vec<Value> {
        let mut list = Vec::new();

        for index in 0..self.get_length() {
            if let Some(value) = self.get(index) {
                list.push(value)
            }
        }

        list
    }

    /// Appends a value to a list.
    ///
    /// # Panics
    /// Panics if sentry failed to allocate memory.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{Event, List, Object};
    /// # fn main() -> anyhow::Result<()> {
    /// let event = Event::new();
    /// let list = List::new();
    /// list.push(true);
    /// event.insert("test", list);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    pub fn push<V: Into<Value>>(&self, value: V) {
        let list = self.unwrap();

        let value = value.into();

        match unsafe { sys::value_append(list, value.take()) } {
            0 => (),
            _ => panic!("sentry failed to allocate memory"),
        }
    }

    /// Returns the length of the list.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{Event, List, Object};
    /// # fn main() -> anyhow::Result<()> {
    /// let event = Event::new();
    /// let list = List::new();
    /// list.push(true);
    /// assert_eq!(1, list.get_length());
    /// event.insert("test", list);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn get_length(&self) -> usize {
        let list = self.unwrap();

        unsafe { sys::value_get_length(list) }
    }

    /// Looks up a value in a list by index.
    ///
    /// # Errors
    /// Fails with [`Error::StrUtf8`] if value conversion fails.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{Event, List, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let event = Event::new();
    /// let list = List::new();
    /// list.push(true);
    /// assert_eq!(Value::Bool(true), list.get(0).unwrap());
    /// event.insert("test", list);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Value> {
        let list = self.unwrap();

        match unsafe { sys::value_get_by_index_owned(list, index) }.into() {
            Value::Null => None,
            value => Some(value),
        }
    }

    /// Inserts a value into the list at a certain position.
    ///
    /// # Panics
    /// Panics if sentry failed to allocate memory.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{Event, List, Object, Value};
    /// # fn main() -> anyhow::Result<()> {
    /// let event = Event::new();
    /// let list = List::new();
    /// list.set_by_index(0, true);
    /// assert_eq!(Value::Bool(true), list.get(0).unwrap());
    /// event.insert("test", list);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    pub fn set_by_index<V: Into<Value>>(&self, index: usize, value: V) {
        let list = self.unwrap();

        let value = value.into();

        match unsafe { sys::value_set_by_index(list, index, value.take()) } {
            0 => (),
            _ => panic!("sentry failed to allocate memory"),
        }
    }

    /// Removes a value from the list by index.
    ///
    /// # Errors
    /// Fails with [`Error::ListRemove`] if index wasn't found.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{Event, List, Object};
    /// # fn main() -> anyhow::Result<()> {
    /// let event = Event::new();
    /// let list = List::new();
    /// list.push(true);
    /// list.remove_by_index(0)?;
    /// assert_eq!(None, list.get(0));
    /// event.insert("test", list);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    pub fn remove_by_index(&self, index: usize) -> Result<(), Error> {
        let list = self.unwrap();

        match unsafe { sys::value_remove_by_index(list, index) } {
            0 => Ok(()),
            _ => Err(Error::ListRemove),
        }
    }
}

/*
#[cfg(test)]
mod test {
    use crate::{List, Map};
    use anyhow::Result;
    use rusty_fork::test_fork;
    use std::convert::TryFrom;
    use std::ffi::CString;

    #[test_fork]
    fn test() -> Result<()> {
        let list = List::new();

        list.push(());
        assert_eq!(list.get(0), None);

        list.push("test1");
        assert_eq!(list.get(1), Some("test1".into()));
        list.push(&String::from("test2"));
        assert_eq!(list.get(2), Some((&String::from("test2")).into()));
        list.push(CString::new("test3")?);
        assert_eq!(list.get(3), Some(CString::new("test3")?));

        list.push(true);
        assert_eq!(list.get(4), true);

        list.push(5);
        assert_eq!(list.get(5), Some(5.into()));

        list.push(6.6);
        assert_eq!(list.get(6), Some(6.6.into()));

        list.push(List::new());
        assert_eq!(list.get(7), Some(List::new().into()));

        list.push(Map::new());
        assert_eq!(list.get(8), Some(Map::new().into()));

        list.push("test9", List::new());
        assert_eq!(list.get(CString::new("test9")?), Some(List::new().into()));

        list.insert("test10", Map::new());
        assert_eq!(list.get(&String::from("test10")), Some(Map::new().into()));

        list.remove("test3")?;
        assert_eq!(list.get("test3"), None);
        list.remove(CString::new("test4")?)?;
        assert_eq!(list.get("test4"), None);
        list.remove("test5")?;
        assert_eq!(list.get(&String::from("test5")), None);

        assert_eq!(list.get_length(), 8);

        assert_eq!(Map::try_from(list.get("test10").unwrap())?.to_vec(), vec!());

        Ok(())
    }
}
*/
