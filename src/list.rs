//! Sentry list implementation.

use crate::{Error, Value};
use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    iter::FromIterator,
};

/// A Sentry list value.
///
/// # Examples
/// ```
/// # use sentry_contrib_native::{Event, List, Map, Object};
/// # use std::iter::FromIterator;
/// let mut event = Event::new();
///
/// let mut list = List::new();
/// list.push(true);
///
/// event.insert("extra", Map::from_iter(Some(("some extra data", list))));
/// event.capture();
/// ```
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
        let self_raw = self.as_ref();
        let list = Self::new();
        let list_raw = list.as_ref();

        for index in 0..self.len() {
            match unsafe {
                sys::value_set_by_index(
                    list_raw,
                    index,
                    sys::value_get_by_index_owned(self_raw, index),
                )
            } {
                0 => (),
                _ => panic!("Sentry failed to allocate memory"),
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
        let mut list = Self::new();

        for item in iter {
            list.push(item);
        }

        list
    }
}

impl<V: Into<Value>> Extend<V> for List {
    fn extend<T: IntoIterator<Item = V>>(&mut self, iter: T) {
        for value in iter {
            self.push(value);
        }
    }
}

impl List {
    /// Creates a new Sentry list.
    #[must_use]
    pub fn new() -> Self {
        Self(Some(unsafe { sys::value_new_list() }))
    }

    /// Creates a [`List`] from [`sys::Value`].
    ///
    /// # Safety
    /// This doesn't check if [`sys::Value`] really is a [`List`].
    pub(crate) const unsafe fn from_raw(value: sys::Value) -> Self {
        Self(Some(value))
    }

    /// Yields [`sys::Value`], ownership is retained.
    fn as_ref(&self) -> sys::Value {
        self.0.expect("use after free")
    }

    /// Yields [`sys::Value`], [`List`] is consumed and caller is responsible
    /// for deallocating [`sys::Value`].
    pub(crate) fn take(mut self) -> sys::Value {
        self.0.take().expect("use after free")
    }

    /// Converts the [`List`] to a [`Vec`].
    #[must_use]
    pub fn to_vec(&self) -> Vec<Value> {
        let mut list = Vec::new();

        for index in 0..self.len() {
            if let Some(value) = self.get(index) {
                list.push(value)
            } else {
                list.push(Value::Null)
            }
        }

        list
    }

    /// Appends a [`Value`] to the [`List`].
    ///
    /// # Panics
    /// Panics if Sentry failed to allocate memory.
    pub fn push<V: Into<Value>>(&mut self, value: V) {
        let list = self.as_ref();

        let value = value.into();

        match unsafe { sys::value_append(list, value.take()) } {
            0 => (),
            _ => panic!("Sentry failed to allocate memory"),
        }
    }

    /// Returns the length of the [`List`].
    #[must_use]
    pub fn len(&self) -> usize {
        let list = self.as_ref();

        unsafe { sys::value_get_length(list) }
    }

    /// Returns `true` if the [`List`] has a length of 0.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Looks up a value in the [`List`] at position `index`.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{List, Value};
    /// let mut list = List::new();
    /// list.push(true);
    /// assert_eq!(Some(Value::Bool(true)), list.get(0));
    /// ```
    #[must_use]
    pub fn get(&self, index: usize) -> Option<Value> {
        let list = self.as_ref();

        match Value::from_raw(unsafe { sys::value_get_by_index_owned(list, index) }) {
            Value::Null => None,
            value => Some(value),
        }
    }

    /// Inserts a [`Value`] into the [`List`] at position `index`.
    ///
    /// # Panics
    /// Panics if Sentry failed to allocate memory.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{List, Value};
    /// let mut list = List::new();
    /// list.insert(5, true);
    /// assert_eq!(Some(Value::Bool(true)), list.get(5));
    /// ```
    pub fn insert<V: Into<Value>>(&mut self, index: usize, value: V) {
        let list = self.as_ref();

        let value = value.into();

        match unsafe { sys::value_set_by_index(list, index, value.take()) } {
            0 => (),
            _ => panic!("Sentry failed to allocate memory"),
        }
    }

    /// Removes a [`Value`] from the [`List`] at position `index`.
    ///
    /// # Errors
    /// Fails with [`Error::ListRemove`] if position at `index` wasn't found.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::List;
    /// # fn main() -> anyhow::Result<()> {
    /// let mut list = List::new();
    /// list.push(true);
    /// list.remove(0)?;
    /// assert_eq!(None, list.get(0));
    /// # Ok(()) }
    /// ```
    pub fn remove(&mut self, index: usize) -> Result<(), Error> {
        let list = self.as_ref();

        match unsafe { sys::value_remove_by_index(list, index) } {
            0 => Ok(()),
            _ => Err(Error::ListRemove),
        }
    }
}

#[test]
#[allow(clippy::cognitive_complexity)]
fn list() -> anyhow::Result<()> {
    use crate::Map;

    let mut list = List::new();
    list.push(true);

    let mut list2 = List::new();
    list2.push(true);

    #[allow(clippy::redundant_clone)]
    {
        assert_eq!(list, list.clone());
        assert_eq!(list, list2);
        assert_eq!(list, list2.clone());
        assert_ne!(list, List::new());
        assert_ne!(list.clone(), List::new());
        assert_ne!(list, List::new().clone());
    }

    let mut list = List::new();

    list.push(());
    assert_eq!(list.get(0), None);

    list.push(true);
    assert_eq!(list.get(1), Some(true.into()));

    list.push(5);
    assert_eq!(list.get(2), Some(5.into()));

    list.push(6.6);
    assert_eq!(list.get(3), Some(6.6.into()));

    list.push("test1");
    assert_eq!(list.get(4), Some("test1".into()));
    list.push(String::from("test2"));
    assert_eq!(list.get(5), Some("test2".into()));

    list.push(List::new());
    assert_eq!(list.get(6), Some(List::new().into()));

    list.push(Map::new());
    assert_eq!(list.get(7), Some(Map::new().into()));

    list.extend(&["some", "test", "data"]);
    assert_eq!(list.get(8), Some("some".into()));
    assert_eq!(list.get(9), Some("test".into()));
    assert_eq!(list.get(10), Some("data".into()));
    list.extend(vec!["some", "test", "data"]);
    list.extend(&vec!["some", "test", "data"]);

    assert_eq!(list.len(), 17);

    assert_eq!(list.to_vec(), list.to_vec());
    assert_eq!(list, list.clone());
    assert_ne!(list.to_vec(), Vec::<Value>::new());
    assert_ne!(list, List::new());

    let new_list: Vec<Value> = vec![
        ().into(),
        true.into(),
        5.into(),
        6.6.into(),
        "test1".into(),
        "test2".into(),
        List::new().into(),
        Map::new().into(),
        "some".into(),
        "test".into(),
        "data".into(),
        "some".into(),
        "test".into(),
        "data".into(),
        "some".into(),
        "test".into(),
        "data".into(),
    ];
    assert_eq!(list.to_vec(), new_list);
    assert_eq!(list, List::from_iter(new_list.clone()));
    #[allow(clippy::redundant_clone)]
    {
        assert_eq!(list.clone(), List::from_iter(new_list));
    }

    list.remove(3)?;
    assert_eq!(list.len(), 16);

    let list = List::from_iter(&[(), (), ()]);
    assert_eq!(list.len(), 3);

    Ok(())
}

#[test]
fn sync() -> anyhow::Result<()> {
    use std::{
        convert::{TryFrom, TryInto},
        sync::{Arc, Mutex},
        thread,
    };

    let list = List::new();

    let list = {
        let mut handles = vec![];
        let list = Arc::new(Mutex::new(list));

        for index in 0..100 {
            let list = Arc::clone(&list);

            handles.push(thread::spawn(move || {
                list.lock()
                    .unwrap()
                    .insert(index, i32::try_from(index).unwrap());
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        Arc::try_unwrap(list).unwrap().into_inner()?
    };

    {
        let mut handles = vec![];
        let list = Arc::new(list);

        for index in 0..100 {
            let list = Arc::clone(&list);

            handles.push(thread::spawn(move || {
                assert_eq!(list.get(index), Some(Value::Int(index.try_into().unwrap())));
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }
    }

    Ok(())
}

#[test]
fn send() {
    use std::thread;

    let mut list = List::new();
    list.push("test");

    thread::spawn(move || {
        assert_eq!(list.get(0), Some(Value::String("test".into())));
    })
    .join()
    .unwrap();
}
