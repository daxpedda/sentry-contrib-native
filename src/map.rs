/// A sentry map value.
pub struct Map(Option<sys::Value>);

object_drop!(Map);

impl Default for Map {
    fn default() -> Self {
        Self::new()
    }
}

object_sealed!(Map);
object_debug!(Map);
object_clone!(Map);
object_partial_eq!(Map);
object_from_iterator!(Map);
object_extend!(Map);

impl Map {
    /// Creates a new object.
    ///
    /// # Examples
    /// ```
    /// # use sentry_native::{Event, Map, Object};
    /// # fn main() -> anyhow::Result<()> {
    /// let event = Event::new();
    /// let object = Map::new();
    /// object.insert("test", true);
    /// event.insert("test", object);
    /// event.capture();
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self(Some(unsafe { sys::value_new_object() }))
    }

    pub(crate) unsafe fn from_raw(value: sys::Value) -> Self {
        Self(Some(value))
    }
}

#[cfg(test)]
mod test {
    use crate::{List, Map, Object};
    use anyhow::Result;
    use rusty_fork::test_fork;
    use std::{convert::TryFrom, ffi::CString};

    #[test_fork]
    fn test() -> Result<()> {
        let object = Map::new();

        object.insert(CString::new("test0")?, ());
        assert_eq!(object.get(CString::new("test0")?), None);
        object.insert("test1", ());
        assert_eq!(object.get("test1"), None);

        object.insert(&String::from("test2"), ());
        assert_eq!(object.get(&String::from("test2")), None);

        object.insert("test3", true);
        assert_eq!(object.get("test3"), Some(true.into()));
        object.insert("test4", 4);
        assert_eq!(object.get("test4"), Some(4.into()));
        object.insert("test5", 5.5);
        assert_eq!(object.get("test5"), Some(5.5.into()));
        object.insert("test6", CString::new("6")?);
        assert_eq!(object.get("test6"), Some(CString::new("6")?.into()));
        object.insert("test7", "7");
        assert_eq!(object.get("test7"), Some("7".into()));
        object.insert("test8", &String::from("8"));
        assert_eq!(object.get("test8"), Some((&String::from("8")).into()));

        object.insert("test9", List::new());
        assert_eq!(object.get(CString::new("test9")?), Some(List::new().into()));

        object.insert("test10", Map::new());
        assert_eq!(object.get(&String::from("test10")), Some(Map::new().into()));

        object.remove("test3")?;
        assert_eq!(object.get("test3"), None);
        object.remove(CString::new("test4")?)?;
        assert_eq!(object.get("test4"), None);
        object.remove("test5")?;
        assert_eq!(object.get(&String::from("test5")), None);

        assert_eq!(object.get_length(), 8);

        assert_eq!(
            Map::try_from(object.get("test10").unwrap())?.to_vec(),
            vec!()
        );

        Ok(())
    }
}
