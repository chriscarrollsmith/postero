use serde::{Deserialize, Serialize};
use sqlx::{Row, PgPool};
use chrono::{DateTime, Utc};
use crate::{Result, Error};
use super::{SyncDirection, LibraryType, GroupData, UserData, ZoteroClient};
use crate::filesystem::FileSystem;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    // Fields from libraries table
    pub id: i64,
    pub library_type: LibraryType,
    pub version: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>, // JSONB column - can be GroupData or UserData
    pub deleted: bool,
    pub item_version: i64,
    pub collection_version: i64,
    pub tag_version: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gitlab: Option<DateTime<Utc>>,

    // Fields from sync_libraries table
    pub active: bool,
    pub sync_direction: SyncDirection,
    pub sync_tags: bool,

    // Computed/helper fields
    pub is_modified: bool,

    // Add reference to client for sync operations
    #[serde(skip)]
    pub client: Option<std::sync::Arc<ZoteroClient>>,
    #[serde(skip)]
    pub db: Option<PgPool>,
    #[serde(skip)]
    pub db_schema: Option<String>,
    #[serde(skip)]
    pub filesystem: Option<std::sync::Arc<dyn FileSystem>>,
}

impl Library {
    pub fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self> {
        // Parse the data JSONB column
        let data_json = row.try_get::<Option<serde_json::Value>, &str>("data")?;

        Ok(Self {
            id: row.try_get("id")?,
            library_type: row.try_get("library_type")?,
            version: row.try_get("version")?,
            created: row.try_get("created")?,
            modified: row.try_get("modified")?,
            data: data_json,
            deleted: row.try_get("deleted")?,
            item_version: row.try_get("item_version")?,
            collection_version: row.try_get("collection_version")?,
            tag_version: row.try_get("tag_version")?,
            gitlab: row.try_get("gitlab")?,
            active: row.try_get("active")?,
            sync_direction: row.try_get("direction")?,
            sync_tags: row.try_get("tags")?,
            is_modified: false,
            client: None,
            db: None,
            db_schema: None,
            filesystem: None,
        })
    }

    pub fn from_group_data(data: &GroupData) -> Self {
        Self {
            id: data.id,
            library_type: LibraryType::Group,
            version: data.version,
            created: None,
            modified: None,
            data: Some(serde_json::to_value(data).unwrap()),
            deleted: false,
            item_version: 0,
            collection_version: 0,
            tag_version: 0,
            gitlab: None,
            active: true,
            sync_direction: SyncDirection::ToLocal,
            sync_tags: false,
            is_modified: false,
            client: None,
            db: None,
            db_schema: None,
            filesystem: None,
        }
    }

    pub fn from_user_data(data: &UserData) -> Self {
        Self {
            id: data.id,
            library_type: LibraryType::User,
            version: 0, // User libraries don't have versions in the same way
            created: None,
            modified: None,
            data: Some(serde_json::to_value(data).unwrap()),
            deleted: false,
            item_version: 0,
            collection_version: 0,
            tag_version: 0,
            gitlab: None,
            active: true,
            sync_direction: SyncDirection::ToLocal,
            sync_tags: false,
            is_modified: false,
            client: None,
            db: None,
            db_schema: None,
            filesystem: None,
        }
    }

    // Helper methods to access data fields easily
    pub fn name(&self) -> &str {
        match self.library_type {
            LibraryType::Group => {
                self.data.as_ref()
                    .and_then(|d| d.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
            }
            LibraryType::User => {
                self.data.as_ref()
                    .and_then(|d| d.get("displayName"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
            }
        }
    }

    pub fn description(&self) -> Option<&str> {
        match self.library_type {
            LibraryType::Group => {
                self.data.as_ref()
                    .and_then(|d| d.get("description"))
                    .and_then(|desc| desc.as_str())
            }
            LibraryType::User => None, // Users don't have descriptions
        }
    }

    pub fn owner(&self) -> i64 {
        match self.library_type {
            LibraryType::Group => {
                self.data.as_ref()
                    .and_then(|d| d.get("owner"))
                    .and_then(|o| o.as_i64())
                    .unwrap_or(0)
            }
            LibraryType::User => self.id, // User owns their own library
        }
    }

    pub fn group_type(&self) -> &str {
        match self.library_type {
            LibraryType::Group => {
                self.data.as_ref()
                    .and_then(|d| d.get("type"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
            }
            LibraryType::User => "user", // Not really applicable for users
        }
    }

    pub fn library_reading(&self) -> &str {
        match self.library_type {
            LibraryType::Group => {
                self.data.as_ref()
                    .and_then(|d| d.get("libraryReading"))
                    .and_then(|lr| lr.as_str())
                    .unwrap_or("public")
            }
            LibraryType::User => "private", // User libraries are private by default
        }
    }

    pub fn library_editing(&self) -> &str {
        match self.library_type {
            LibraryType::Group => {
                self.data.as_ref()
                    .and_then(|d| d.get("libraryEditing"))
                    .and_then(|le| le.as_str())
                    .unwrap_or("")
            }
            LibraryType::User => "owner", // User can edit their own library
        }
    }

    pub fn file_editing(&self) -> &str {
        match self.library_type {
            LibraryType::Group => {
                self.data.as_ref()
                    .and_then(|d| d.get("fileEditing"))
                    .and_then(|fe| fe.as_str())
                    .unwrap_or("")
            }
            LibraryType::User => "owner", // User can edit files in their own library
        }
    }

    pub fn set_client(&mut self, client: std::sync::Arc<ZoteroClient>, db: PgPool, db_schema: String, filesystem: std::sync::Arc<dyn FileSystem>) {
        self.client = Some(client);
        self.db = Some(db);
        self.db_schema = Some(db_schema);
        self.filesystem = Some(filesystem);
    }

    pub fn can_upload(&self) -> bool {
        matches!(self.sync_direction, 
            SyncDirection::ToCloud | SyncDirection::BothCloud | SyncDirection::BothLocal | SyncDirection::BothManual)
    }

    pub fn can_download(&self) -> bool {
        matches!(self.sync_direction, 
            SyncDirection::ToLocal | SyncDirection::BothCloud | SyncDirection::BothLocal | SyncDirection::BothManual)
    }

    pub fn build_items_url(&self) -> String {
        match self.library_type {
            LibraryType::User => format!("users/{}/items", self.id),
            LibraryType::Group => format!("groups/{}/items", self.id),
        }
    }

    pub fn build_collections_url(&self) -> String {
        match self.library_type {
            LibraryType::User => format!("users/{}/collections", self.id),
            LibraryType::Group => format!("groups/{}/collections", self.id),
        }
    }

    pub fn build_tags_url(&self) -> String {
        match self.library_type {
            LibraryType::User => format!("users/{}/tags", self.id),
            LibraryType::Group => format!("groups/{}/tags", self.id),
        }
    }

    pub fn build_deleted_url(&self) -> String {
        match self.library_type {
            LibraryType::User => format!("users/{}/deleted", self.id),
            LibraryType::Group => format!("groups/{}/deleted", self.id),
        }
    }

    pub async fn clear_local(&mut self) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        // Clear library versions
        let query = format!(
            "UPDATE {}.libraries SET version=0, item_version=0, collection_version=0, tag_version=0 WHERE id=$1 AND library_type=$2",
            schema
        );
        sqlx::query(&query)
            .bind(self.id)
            .bind(self.library_type)
            .execute(db).await?;

        // Clear items
        let query = format!("DELETE FROM {}.items WHERE library_id=$1 AND library_type=$2", schema);
        sqlx::query(&query)
            .bind(self.id)
            .bind(self.library_type)
            .execute(db).await?;

        // Clear collections
        let query = format!("DELETE FROM {}.collections WHERE library_id=$1 AND library_type=$2", schema);
        sqlx::query(&query)
            .bind(self.id)
            .bind(self.library_type)
            .execute(db).await?;

        // Clear tags
        let query = format!("DELETE FROM {}.tags WHERE library_id=$1 AND library_type=$2", schema);
        sqlx::query(&query)
            .bind(self.id)
            .bind(self.library_type)
            .execute(db).await?;

        // Reset local versions
        self.version = 0;
        self.item_version = 0;
        self.collection_version = 0;
        self.tag_version = 0;

        Ok(())
    }

    pub async fn sync(&mut self) -> Result<()> {
        if self.sync_direction == SyncDirection::None {
            return Ok(());
        }

        tracing::info!("Starting sync for {} library {}", self.library_type, self.id);

        // Sync collections
        tracing::info!("Starting sync_collections for {} library {}", self.library_type, self.id);
        let (_, collection_version) = self.sync_collections().await?;
        tracing::info!("Completed sync_collections for {} library {}", self.library_type, self.id);
        
        // Upload modified items
        tracing::info!("Starting upload_items for {} library {}", self.library_type, self.id);
        let (_, _item_version) = self.upload_items().await?;
        tracing::info!("Completed upload_items for {} library {}", self.library_type, self.id);
        
        // Download items from cloud
        tracing::info!("Starting download_items for {} library {}", self.library_type, self.id);
        let (_, item_version) = self.download_items().await?;
        tracing::info!("Completed download_items for {} library {}", self.library_type, self.id);
        
        // Sync tags
        if self.sync_tags {
            tracing::info!("Starting sync_tags for {} library {}", self.library_type, self.id);
            let (_, tag_version) = self.sync_tags().await?;
            self.tag_version = tag_version;
            tracing::info!("Completed sync_tags for {} library {}", self.library_type, self.id);
        }

        // Sync deleted items
        tracing::info!("Starting sync_deleted for {} library {}", self.library_type, self.id);
        let _deleted_version = self.sync_deleted().await?;
        tracing::info!("Completed sync_deleted for {} library {}", self.library_type, self.id);

        // Update local versions
        self.item_version = item_version;
        self.collection_version = collection_version;

        // Update library in database
        self.update_local().await?;

        tracing::info!("Completed sync for {} library {}", self.library_type, self.id);

        Ok(())
    }

    // The rest of the sync methods adapted from Group implementation
    // but using the new library_id and library_type fields

    async fn sync_collections(&self) -> Result<(i64, i64)> {
        let client = self.client.as_ref().ok_or_else(|| Error::InvalidData("Client not set".to_string()))?;
        
        let mut counter = 0i64;
        let mut last_modified_version = self.collection_version;

        // Upload modified collections if we can upload
        if self.can_upload() {
            counter += self.sync_modified_collections().await?;
        }

        // Download collections from cloud if we can download
        if self.can_download() {
            let (versions, cloud_version) = client.get_collections_version_cloud_unified(self.id, self.library_type, self.collection_version).await?;
            
            if cloud_version > last_modified_version {
                last_modified_version = cloud_version;
            }

            let mut collections_to_update = Vec::new();
            for (collection_key, version) in versions {
                let local_version = self.get_collection_version_local(&collection_key).await?;
                if local_version < version {
                    collections_to_update.push(collection_key);
                }
            }

            // Fetch collections in batches of 50
            for chunk in collections_to_update.chunks(50) {
                let (collections, _) = client.get_collections_cloud_unified(self.id, self.library_type, chunk).await?;
                for collection in collections {
                    self.update_collection_local(&collection).await?;
                    counter += 1;
                }
            }
        }

        Ok((counter, last_modified_version))
    }

    async fn upload_items(&self) -> Result<(i64, i64)> {
        if !self.can_upload() {
            return Ok((0, self.item_version));
        }

        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;
        let client = self.client.as_ref().ok_or_else(|| Error::InvalidData("Client not set".to_string()))?;

        // Get the current cloud version first
        let (_, mut last_modified_version) = client.get_items_version_cloud_unified(self.id, self.library_type, 0, false).await?;

        // Query for items that need to be uploaded
        let query = format!(
            r#"
            SELECT key, version, data, meta, trashed, deleted, sync, md5
            FROM {}.items
            WHERE library_id = $1 AND library_type = $2 AND (sync = 'new' OR sync = 'modified')
            ORDER BY key
            "#,
            schema
        );

        let rows = sqlx::query(&query)
            .bind(self.id)
            .bind(self.library_type)
            .fetch_all(db)
            .await?;

        let mut counter = 0i64;

        for row in rows {
            let key: String = row.get("key");
            let version: i64 = row.get("version");
            let data_json: String = row.get("data");
            let meta_json: Option<String> = row.get("meta");
            let trashed: bool = row.get("trashed");
            let deleted: bool = row.get("deleted");
            let sync_status: String = row.get("sync");
            let md5: Option<String> = row.get("md5");

            // Parse the item data
            let item_data: super::ItemData = serde_json::from_str(&data_json)?;
            let item_meta: Option<super::item::ItemMeta> = meta_json
                .map(|json| serde_json::from_str(&json))
                .transpose()?;

            let mut item = super::Item {
                key: key.clone(),
                version,
                library_id: self.id,
                library_type: self.library_type,
                data: item_data,
                meta: item_meta,
                trashed,
                deleted,
                sync_status: match sync_status.as_str() {
                    "new" => super::SyncStatus::New,
                    "modified" => super::SyncStatus::Modified,
                    _ => super::SyncStatus::Synced,
                },
                md5,
                db: Some(db.clone()),
                db_schema: Some(schema.clone()),
            };

            // Upload to cloud
            if let Err(e) = item.update_cloud(client, &mut last_modified_version).await {
                tracing::error!("Failed to upload item {}: {}", key, e);
                continue;
            }

            counter += 1;
        }

        Ok((counter, last_modified_version))
    }

    async fn download_items(&self) -> Result<(i64, i64)> {
        if !self.can_download() {
            return Ok((0, self.item_version));
        }

        let client = self.client.as_ref().ok_or_else(|| Error::InvalidData("Client not set".to_string()))?;
        
        let mut counter = 0i64;
        let mut last_modified_version = self.item_version;

        // Download both trashed and non-trashed items
        for trashed in [true, false] {
            let (versions, cloud_version) = client.get_items_version_cloud_unified(self.id, self.library_type, self.item_version, trashed).await?;
            
            if cloud_version > last_modified_version {
                last_modified_version = cloud_version;
            }

            let mut items_to_update = Vec::new();
            for (item_key, version) in versions {
                let local_version = self.get_item_version_local(&item_key).await?;
                if local_version < version {
                    items_to_update.push(item_key);
                }
            }

            // Fetch items in batches of 50
            for chunk in items_to_update.chunks(50) {
                let items = client.get_items_cloud_unified(self.id, self.library_type, chunk).await?;
                for item in &items {
                    self.update_item_local(item).await?;
                    counter += 1;
                }
                
                // Download attachment files for imported_file attachments
                for item in &items {
                    if item.data.item_type == "attachment" {
                        if let Some(filesystem) = &self.filesystem {
                            if let Err(e) = item.download_attachment_cloud(client, filesystem.as_ref()).await {
                                tracing::error!("Failed to download attachment for item {}: {}", item.key, e);
                                // Continue with other items even if one attachment fails
                            }
                        } else {
                            tracing::warn!("Filesystem not configured, skipping attachment download for item {}", item.key);
                        }
                    }
                }
            }
        }

        Ok((counter, last_modified_version))
    }

    async fn sync_tags(&self) -> Result<(i64, i64)> {
        if !self.can_download() {
            return Ok((0, self.tag_version));
        }

        let client = self.client.as_ref().ok_or_else(|| Error::InvalidData("Client not set".to_string()))?;
        
        let (tags, last_modified_version) = client.get_tags_cloud_unified(self.id, self.library_type, self.tag_version).await?;
        
        let mut counter = 0i64;
        for tag in tags {
            self.create_tag_local(&tag).await?;
            counter += 1;
        }

        Ok((counter, last_modified_version))
    }

    async fn sync_deleted(&self) -> Result<i64> {
        if !self.can_download() {
            return Ok(0);
        }

        let client = self.client.as_ref().ok_or_else(|| Error::InvalidData("Client not set".to_string()))?;
        
        tracing::info!("Starting sync_deleted for {} library {}", self.library_type, self.id);
        
        let (deletions, last_modified_version) = client.get_deletions_cloud_unified(self.id, self.library_type, self.version).await?;
        
        let mut counter = 0i64;
        
        // Delete items
        for item_key in deletions.items {
            self.try_delete_item_local(&item_key, last_modified_version).await?;
            counter += 1;
        }
        
        // Delete collections
        for collection_key in deletions.collections {
            self.try_delete_collection_local(&collection_key, last_modified_version).await?;
            counter += 1;
        }
        
        // Delete tags
        for tag_name in deletions.tags {
            self.delete_tag_local(&tag_name).await?;
            counter += 1;
        }

        tracing::info!("Completed sync_deleted for {} library {}, processed {} deletions", 
            self.library_type, self.id, counter);

        Ok(counter)
    }

    // Helper methods for database operations using new schema

    async fn get_collection_version_local(&self, collection_key: &str) -> Result<i64> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        let query = format!("SELECT version FROM {}.collections WHERE key = $1 AND library_id = $2 AND library_type = $3", schema);
        let result = sqlx::query(&query)
            .bind(collection_key)
            .bind(self.id)
            .bind(self.library_type)
            .fetch_optional(db)
            .await?;

        Ok(result.map(|row| row.get::<i64, _>("version")).unwrap_or(0))
    }

    async fn get_item_version_local(&self, item_key: &str) -> Result<i64> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        let query = format!("SELECT version FROM {}.items WHERE key = $1 AND library_id = $2 AND library_type = $3", schema);
        let result = sqlx::query(&query)
            .bind(item_key)
            .bind(self.id)
            .bind(self.library_type)
            .fetch_optional(db)
            .await?;

        Ok(result.map(|row| row.get::<i64, _>("version")).unwrap_or(0))
    }

    async fn sync_modified_collections(&self) -> Result<i64> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;
        let client = self.client.as_ref().ok_or_else(|| Error::InvalidData("Client not set".to_string()))?;

        // Get current library version
        let mut library_version = self.collection_version;

        // Query for collections that need to be uploaded
        let query = format!(
            r#"
            SELECT key, version, data, meta, deleted, sync
            FROM {}.collections
            WHERE library_id = $1 AND library_type = $2 AND (sync = 'new' OR sync = 'modified')
            "#,
            schema
        );

        let rows = sqlx::query(&query)
            .bind(self.id)
            .bind(self.library_type)
            .fetch_all(db)
            .await?;

        let mut counter = 0i64;

        for row in rows {
            let key: String = row.get("key");
            let version: i64 = row.get("version");
            let data_json: String = row.get("data");
            let meta_json: Option<String> = row.get("meta");
            let deleted: bool = row.get("deleted");
            let sync_status: String = row.get("sync");

            // Parse the collection data
            let collection_data: super::CollectionData = serde_json::from_str(&data_json)?;
            let collection_meta: Option<super::collection::CollectionMeta> = meta_json
                .map(|json| serde_json::from_str(&json))
                .transpose()?;

            let mut collection = super::Collection {
                key: key.clone(),
                version,
                library_id: self.id,
                library_type: self.library_type,
                data: collection_data,
                meta: collection_meta,
                deleted,
                sync_status: match sync_status.as_str() {
                    "new" => super::SyncStatus::New,
                    "modified" => super::SyncStatus::Modified,
                    _ => super::SyncStatus::Synced,
                },
                db: Some(db.clone()),
                db_schema: Some(schema.clone()),
            };

            // Upload to cloud using the unified client methods
            match collection.update_cloud(client, library_version).await {
                Ok(new_version) => {
                    library_version = new_version;
                    counter += 1;
                }
                Err(e) => {
                    tracing::error!("Failed to upload collection {}: {}", key, e);
                    continue;
                }
            }
        }

        Ok(counter)
    }

    async fn update_collection_local(&self, collection: &super::Collection) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        let data_value = serde_json::to_value(&collection.data)?;
        let meta_value = collection.meta.as_ref()
            .map(|m| serde_json::to_value(m))
            .transpose()?;

        let query = format!(
            r#"
            INSERT INTO {}.collections (key, version, library_id, library_type, data, meta, deleted, sync)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (key, library_id, library_type) DO UPDATE SET
                version = EXCLUDED.version,
                data = EXCLUDED.data,
                meta = EXCLUDED.meta,
                deleted = EXCLUDED.deleted,
                sync = EXCLUDED.sync
            "#,
            schema
        );

        sqlx::query(&query)
            .bind(&collection.key)
            .bind(collection.version)
            .bind(self.id)
            .bind(self.library_type)
            .bind(&data_value)
            .bind(&meta_value)
            .bind(collection.deleted)
            .bind(super::SyncStatus::Synced)
            .execute(db)
            .await?;

        Ok(())
    }

    async fn update_item_local(&self, item: &super::Item) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        let data_value = serde_json::to_value(&item.data)?;
        let meta_value = item.meta.as_ref()
            .map(|m| serde_json::to_value(m))
            .transpose()?;

        let query = format!(
            r#"
            INSERT INTO {}.items (key, version, library_id, library_type, data, meta, trashed, deleted, sync, md5)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (key, library_id, library_type) DO UPDATE SET
                version = EXCLUDED.version,
                data = EXCLUDED.data,
                meta = EXCLUDED.meta,
                trashed = EXCLUDED.trashed,
                deleted = EXCLUDED.deleted,
                sync = EXCLUDED.sync,
                md5 = EXCLUDED.md5
            "#,
            schema
        );

        sqlx::query(&query)
            .bind(&item.key)
            .bind(item.version)
            .bind(self.id)
            .bind(self.library_type)
            .bind(&data_value)
            .bind(&meta_value)
            .bind(item.trashed)
            .bind(item.deleted)
            .bind(super::SyncStatus::Synced)
            .bind(&item.md5)
            .execute(db)
            .await?;

        Ok(())
    }

    async fn create_tag_local(&self, tag: &super::Tag) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        // Create tag metadata
        let tag_meta = super::TagMeta {
            tag_type: tag.data.tag_type.unwrap_or(0) as i64,
            num_items: 0, // This would be updated elsewhere
        };
        let meta_value = serde_json::to_value(&tag_meta)?;

        let query = format!(
            r#"
            INSERT INTO {}.tags (tag, meta, library_id, library_type)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (tag, library_id, library_type) DO UPDATE SET
                meta = EXCLUDED.meta
            "#,
            schema
        );

        sqlx::query(&query)
            .bind(&tag.data.tag)
            .bind(&meta_value)
            .bind(self.id)
            .bind(self.library_type)
            .execute(db)
            .await?;

        Ok(())
    }

    async fn try_delete_item_local(&self, item_key: &str, last_modified_version: i64) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        // Check if item exists and get its current sync status
        let query = format!("SELECT sync, deleted FROM {}.items WHERE key = $1 AND library_id = $2 AND library_type = $3", schema);
        let result = sqlx::query(&query)
            .bind(item_key)
            .bind(self.id)
            .bind(self.library_type)
            .fetch_optional(db)
            .await?;

        if let Some(row) = result {
            let sync_status: String = row.get("sync");
            let already_deleted: bool = row.get("deleted");

            if already_deleted {
                return Ok(()); // Already deleted
            }

            // Determine action based on sync status and direction
            let should_delete = match sync_status.as_str() {
                "synced" => true, // Safe to delete
                _ if self.can_download() => true, // Cloud leads, delete locally
                _ => {
                    // Local leads, mark as synced with cloud version
                    let query = format!(
                        "UPDATE {}.items SET version = $1, sync = 'synced' WHERE key = $2 AND library_id = $3 AND library_type = $4",
                        schema
                    );
                    sqlx::query(&query)
                        .bind(last_modified_version)
                        .bind(item_key)
                        .bind(self.id)
                        .bind(self.library_type)
                        .execute(db)
                        .await?;
                    false
                }
            };

            if should_delete {
                let query = format!(
                    "UPDATE {}.items SET deleted = true WHERE key = $1 AND library_id = $2 AND library_type = $3",
                    schema
                );
                sqlx::query(&query)
                    .bind(item_key)
                    .bind(self.id)
                    .bind(self.library_type)
                    .execute(db)
                    .await?;
            }
        }

        Ok(())
    }

    async fn try_delete_collection_local(&self, collection_key: &str, last_modified_version: i64) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        // Check if collection exists and get its current sync status
        let query = format!("SELECT sync, deleted FROM {}.collections WHERE key = $1 AND library_id = $2 AND library_type = $3", schema);
        let result = sqlx::query(&query)
            .bind(collection_key)
            .bind(self.id)
            .bind(self.library_type)
            .fetch_optional(db)
            .await?;

        if let Some(row) = result {
            let sync_status: String = row.get("sync");
            let already_deleted: bool = row.get("deleted");

            if already_deleted {
                return Ok(()); // Already deleted
            }

            // Determine action based on sync status and direction
            let should_delete = match sync_status.as_str() {
                "synced" => true, // Safe to delete
                _ if self.can_download() => true, // Cloud leads, delete locally
                _ => {
                    // Local leads, mark as synced with cloud version
                    let query = format!(
                        "UPDATE {}.collections SET version = $1, sync = 'synced' WHERE key = $2 AND library_id = $3 AND library_type = $4",
                        schema
                    );
                    sqlx::query(&query)
                        .bind(last_modified_version)
                        .bind(collection_key)
                        .bind(self.id)
                        .bind(self.library_type)
                        .execute(db)
                        .await?;
                    false
                }
            };

            if should_delete {
                let query = format!(
                    "UPDATE {}.collections SET deleted = true WHERE key = $1 AND library_id = $2 AND library_type = $3",
                    schema
                );
                sqlx::query(&query)
                    .bind(collection_key)
                    .bind(self.id)
                    .bind(self.library_type)
                    .execute(db)
                    .await?;
            }
        }

        Ok(())
    }

    async fn delete_tag_local(&self, tag_name: &str) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        let query = format!("DELETE FROM {}.tags WHERE tag = $1 AND library_id = $2 AND library_type = $3", schema);
        sqlx::query(&query)
            .bind(tag_name)
            .bind(self.id)
            .bind(self.library_type)
            .execute(db)
            .await?;

        Ok(())
    }

    pub async fn update_local(&self) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        let query = format!(
            "UPDATE {}.libraries SET version=$1, item_version=$2, collection_version=$3, tag_version=$4, modified=NOW() WHERE id=$5 AND library_type=$6",
            schema
        );
        
        sqlx::query(&query)
            .bind(self.version)
            .bind(self.item_version)
            .bind(self.collection_version)
            .bind(self.tag_version)
            .bind(self.id)
            .bind(self.library_type)
            .execute(db)
            .await?;

        Ok(())
    }
} 