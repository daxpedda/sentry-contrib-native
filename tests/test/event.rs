use serde::{Deserialize, Deserializer};
use serde_derive::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(untagged)]
pub enum Response {
    Event(Event),
    NotFound { detail: String },
}

#[derive(Clone, Debug, Deserialize)]
pub struct Event {
    pub title: String,
    pub message: String,
    #[serde(deserialize_with = "tags")]
    pub tags: HashMap<String, String>,
    #[serde(deserialize_with = "entries")]
    pub entries: HashMap<String, HashMap<String, Value>>,
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
