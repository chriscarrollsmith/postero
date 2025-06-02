use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// Custom deserializer for parent_collection that handles boolean false
fn deserialize_parent_collection<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
    
    match value {
        serde_json::Value::Bool(false) => Ok(None),
        serde_json::Value::String(s) => Ok(Some(s)),
        serde_json::Value::Null => Ok(None),
        _ => Ok(None),
    }
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write: Option<bool>,
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
    #[serde(rename = "parentCollection", skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_parent_collection")]
    pub parent_collection: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relations: Option<serde_json::Value>,
}

// API response structure for collections from Zotero
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionApiResponse {
    pub key: String,
    pub version: i64,
    pub library: LibraryInfo,
    pub links: serde_json::Value,
    pub meta: CollectionMeta,
    pub data: CollectionData,
}

// API response structure for items from Zotero
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemApiResponse {
    pub key: String,
    pub version: i64,
    pub library: LibraryInfo,
    pub links: serde_json::Value,
    pub meta: ItemMeta,
    pub data: ItemData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemMeta {
    #[serde(rename = "createdByUser")]
    pub created_by_user: Option<UserData>,
    #[serde(rename = "creatorSummary")]
    pub creator_summary: Option<String>,
    #[serde(rename = "parsedDate")]
    pub parsed_date: Option<String>,
    #[serde(rename = "numChildren")]
    pub num_children: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryInfo {
    #[serde(rename = "type")]
    pub library_type: String,
    pub id: i64,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionMeta {
    #[serde(rename = "numCollections")]
    pub num_collections: i32,
    #[serde(rename = "numItems")]
    pub num_items: i32,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<i64>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub group_type: Option<String>,
    #[serde(rename = "libraryReading", skip_serializing_if = "Option::is_none")]
    pub library_reading: Option<String>,
    #[serde(rename = "libraryEditing", skip_serializing_if = "Option::is_none")]
    pub library_editing: Option<String>,
    #[serde(rename = "fileEditing", skip_serializing_if = "Option::is_none")]
    pub file_editing: Option<String>,
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