use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use regex::Regex;
use lazy_static::lazy_static;

pub mod client;
pub mod types;
pub mod library;
pub mod item;
pub mod collection;
pub mod tag;
pub mod user;
pub mod sync;

pub use client::ZoteroClient;
pub use types::*;
pub use library::Library;
pub use item::{Item, ItemType};
pub use collection::Collection;
pub use tag::Tag;
pub use user::User;
pub use sync::{SyncDirection, SyncMode, SyncStatus, LibraryType};

lazy_static! {
    static ref TEXT_VARIABLES_REGEX: Regex = Regex::new(r#"([a-zA-Z0-9_]+:([^ \n<"]+|"[^"]+"))"#).unwrap();
    static ref REMOVE_EMPTY_REGEX: Regex = Regex::new(r"(?m)^\s*$[\r\n]*|[\r\n]+\s+\z").unwrap();
}

/// Extract metadata from text using the format "key:value"
pub fn text_to_metadata(text: &str) -> HashMap<String, Vec<String>> {
    let mut meta = HashMap::new();
    
    for cap in TEXT_VARIABLES_REGEX.captures_iter(text) {
        if let Some(key_value) = cap.get(0) {
            let parts: Vec<&str> = key_value.as_str().splitn(2, ':').collect();
            if parts.len() == 2 {
                let key = parts[0].to_string();
                let value = parts[1].trim().trim_matches('"').to_string();
                
                meta.entry(key).or_insert_with(Vec::new).push(value);
            }
        }
    }
    
    meta
}

/// Remove metadata tags from text and clean up empty lines
pub fn text_no_meta(text: &str) -> String {
    let result = TEXT_VARIABLES_REGEX.replace_all(text, " ");
    REMOVE_EMPTY_REGEX.replace_all(&result, "").to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCollectionCreateResultFailed {
    pub key: String,
    pub code: i64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCollectionCreateResult {
    pub success: HashMap<String, String>,
    pub unchanged: HashMap<String, String>,
    pub failed: HashMap<String, ItemCollectionCreateResultFailed>,
    pub successful: HashMap<String, Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deletions {
    pub collections: Vec<String>,
    pub searches: Vec<String>,
    pub items: Vec<String>,
    pub tags: Vec<String>,
    pub settings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RelationList(pub HashMap<String, String>);

impl<'de> Deserialize<'de> for RelationList {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        
        let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
        
        match value {
            serde_json::Value::Object(obj) => {
                let mut map = HashMap::new();
                for (k, v) in obj {
                    if let serde_json::Value::String(s) = v {
                        map.insert(k, s);
                    }
                }
                Ok(RelationList(map))
            }
            serde_json::Value::Array(arr) => {
                if arr.is_empty() {
                    Ok(RelationList(HashMap::new()))
                } else {
                    Err(D::Error::custom("non-empty array not supported for RelationList"))
                }
            }
            _ => Err(D::Error::custom("invalid type for RelationList")),
        }
    }
}

impl Serialize for RelationList {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;
        if self.0.is_empty() {
            let seq = serializer.serialize_seq(Some(0))?;
            seq.end()
        } else {
            self.0.serialize(serializer)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZoteroStringList(pub Vec<String>);

impl<'de> Deserialize<'de> for ZoteroStringList {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        
        let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
        
        match value {
            serde_json::Value::String(s) => Ok(ZoteroStringList(vec![s])),
            serde_json::Value::Array(arr) => {
                let mut strings = Vec::new();
                for item in arr {
                    if let serde_json::Value::String(s) = item {
                        strings.push(s);
                    } else {
                        return Err(D::Error::custom("array must contain only strings"));
                    }
                }
                Ok(ZoteroStringList(strings))
            }
            _ => Err(D::Error::custom("expected string or array of strings")),
        }
    }
}

impl Serialize for ZoteroStringList {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.0.len() == 1 {
            self.0[0].serialize(serializer)
        } else {
            self.0.serialize(serializer)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Parent(pub String);

impl<'de> Deserialize<'de> for Parent {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
        
        match value {
            serde_json::Value::Bool(_) => Ok(Parent(String::new())),
            serde_json::Value::String(s) => Ok(Parent(s)),
            _ => Ok(Parent(String::new())),
        }
    }
}

impl Serialize for Parent {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.0.is_empty() {
            false.serialize(serializer)
        } else {
            self.0.serialize(serializer)
        }
    }
} 