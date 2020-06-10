//! Sentry map implementation.

use crate::{Object, SentryString, Value};

/// A Sentry map value.
pub struct Map(Option<sys::Value>);

impl Default for Map {
    fn default() -> Self {
        Self::new()
    }
}

derive_object!(Map);

impl<S, V> PartialEq<[(S, V)]> for Map
where
    SentryString: PartialEq<S>,
    Value: PartialEq<V>,
{
    fn eq(&self, other: &[(S, V)]) -> bool {
        if self.len() != other.len() {
            return false;
        }

        let map = self.to_vec();

        map.iter()
            .zip(other.iter())
            .all(|(x, y)| x.0 == y.0 && x.1 == y.1)
    }
}

impl Map {
    /// Creates a new object.
    ///
    /// # Examples
    /// ```
    /// # use sentry_contrib_native::{Event, Map, Object};
    /// # fn main() -> anyhow::Result<()> {
    /// let mut event = Event::new();
    /// let mut object = Map::new();
    /// object.insert("test", true);
    /// event.insert("test", object);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self(Some(unsafe { sys::value_new_object() }))
    }

    pub(crate) const unsafe fn from_raw(value: sys::Value) -> Self {
        Self(Some(value))
    }
}

#[cfg(test)]
mod test {
    use crate::{List, Map, Object};
    use anyhow::Result;
    use rusty_fork::test_fork;

    #[test_fork]
    fn test() -> Result<()> {
        let mut object = Map::new();

        object.insert("test1", ());
        assert_eq!(object.get("test1"), None);

        object.insert(String::from("test2").as_ref(), ());
        assert_eq!(object.get(String::from("test2").as_ref()), None);

        object.insert("test3", true);
        assert_eq!(object.get("test3"), Some(true.into()));
        object.insert("test4", 4);
        assert_eq!(object.get("test4"), Some(4.into()));
        object.insert("test5", 5.5);
        assert_eq!(object.get("test5"), Some(5.5.into()));
        object.insert("test7", "7");
        assert_eq!(object.get("test7"), Some("7".into()));
        object.insert("test8", String::from("8"));
        assert_eq!(object.get("test8"), Some((String::from("8")).into()));

        object.insert("test9", List::new());
        assert_eq!(object.get("test9"), Some(List::new().into()));

        object.insert("test10", Map::new());
        assert_eq!(
            object.get(String::from("test10").as_ref()),
            Some(Map::new().into())
        );

        object.remove("test3")?;
        assert_eq!(object.get("test3"), None);
        object.remove("test4")?;
        assert_eq!(object.get("test4"), None);
        object.remove("test5")?;
        assert_eq!(object.get(String::from("test5").as_ref()), None);

        assert_eq!(object.len(), 6);

        assert_eq!(
            object.get("test10").unwrap().as_map().unwrap().to_vec(),
            vec!()
        );

        Ok(())
    }
}
