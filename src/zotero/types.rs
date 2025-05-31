use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub key: String,
    #[serde(rename = "userID")]
    pub user_id: i64,
    pub username: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub access: ApiKeyAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyAccess {
    pub user: ApiKeyUserAccess,
    pub groups: ApiKeyGroupAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyUserAccess {
    pub library: bool,
    pub files: bool,
    pub notes: bool,
    pub write: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyGroupAccess {
    pub all: ApiKeyUserAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Creator {
    #[serde(rename = "creatorType")]
    pub creator_type: String,
    #[serde(rename = "firstName", skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    #[serde(rename = "lastName")]
    pub last_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemData {
    pub key: String,
    pub version: i64,
    #[serde(rename = "itemType")]
    pub item_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creators: Option<Vec<Creator>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(rename = "dateAdded")]
    pub date_added: DateTime<Utc>,
    #[serde(rename = "dateModified")]
    pub date_modified: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<TagData>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collections: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations: Option<serde_json::Value>,
    #[serde(flatten)]
    pub extra_fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagData {
    pub tag: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub tag_type: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagMeta {
    #[serde(rename = "type")]
    pub tag_type: i64,
    #[serde(rename = "numItems")]
    pub num_items: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionData {
    pub key: String,
    pub version: i64,
    pub name: String,
    #[serde(rename = "parentCollection", skip_serializing_if = "Option::is_none")]
    pub parent_collection: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMeta {
    pub created: DateTime<Utc>,
    #[serde(rename = "lastModified")]
    pub last_modified: DateTime<Utc>,
    #[serde(rename = "numItems")]
    pub num_items: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupData {
    pub id: i64,
    pub version: i64,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub owner: i64,
    #[serde(rename = "type")]
    pub group_type: String,
    #[serde(rename = "libraryReading")]
    pub library_reading: String,
    #[serde(rename = "libraryEditing")]
    pub library_editing: String,
    #[serde(rename = "fileEditing")]
    pub file_editing: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(rename = "hasImage", skip_serializing_if = "Option::is_none")]
    pub has_image: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admins: Option<Vec<i64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserData {
    pub id: i64,
    pub username: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub data: TagData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deletions {
    pub collections: Vec<String>,
    pub items: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UploadAuthorization {
    pub exists: bool,
    pub upload_url: Option<String>,
    pub upload_key: Option<String>,
    pub params: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadAuthorizationResponse {
    pub url: String,
    #[serde(rename = "uploadKey")]
    pub upload_key: String,
    pub params: std::collections::HashMap<String, String>,
} 