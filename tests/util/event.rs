use serde::{Deserialize, Deserializer};
use serde_derive::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, ops::Deref};

#[derive(Clone, Debug, Deserialize)]
pub struct Event {
    pub title: String,
    pub message: String,
    pub context: HashMap<String, Value>,
    pub contexts: HashMap<String, Contexts>,
    #[serde(deserialize_with = "tags")]
    pub tags: HashMap<String, String>,
    #[serde(deserialize_with = "entries")]
    pub entries: HashMap<String, HashMap<String, Value>>,
    pub user: Option<User>,
    pub release: Option<Release>,
    pub dist: Option<String>,
    #[serde(default)]
    pub attachments: HashMap<String, Attachment>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Contexts {
    pub r#type: String,
    #[serde(flatten)]
    pub data: HashMap<String, Value>,
}

impl Deref for Contexts {
    type Target = HashMap<String, Value>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[derive(Deserialize)]
struct Tag {
    pub key: String,
    pub value: String,
}

fn tags<'de, D>(deserializer: D) -> Result<HashMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut map = HashMap::new();

    for tag in Vec::<Tag>::deserialize(deserializer)? {
        map.insert(tag.key, tag.value);
    }

    Ok(map)
}

#[derive(Deserialize)]
struct Entry {
    pub r#type: String,
    pub data: HashMap<String, Value>,
}

fn entries<'de, D>(deserializer: D) -> Result<HashMap<String, HashMap<String, Value>>, D::Error>
where
    D: Deserializer<'de>,
{
    let mut map = HashMap::new();

    for tag in Vec::<Entry>::deserialize(deserializer)? {
        map.insert(tag.r#type, tag.data);
    }

    Ok(map)
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct User {
    pub data: Option<HashMap<String, Value>>,
    pub email: Option<String>,
    pub id: Option<String>,
    pub ip_address: Option<String>,
    pub name: Option<String>,
    pub username: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub short_version: Option<String>,
    pub version: Option<String>,
    pub version_info: Option<VersionInfo>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct VersionInfo {
    pub description: String,
    pub version: HashMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub sha1: String,
    pub size: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(clippy::module_name_repetitions)]
pub struct MinEvent {
    #[serde(rename = "eventID")]
    pub event_id: String,
    pub user: Option<User>,
}
