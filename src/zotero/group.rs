use serde::{Deserialize, Serialize};
use sqlx::{Row, PgPool};
use chrono::{DateTime, Utc};
use crate::{Result, Error};
use super::{SyncDirection, GroupData, ZoteroClient};
use crate::filesystem::FileSystem;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    // Fields from groups table
    pub id: i64,
    pub version: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<GroupData>, // JSONB column
    pub deleted: bool,
    pub item_version: i64,
    pub collection_version: i64,
    pub tag_version: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gitlab: Option<DateTime<Utc>>,
    
    // Fields from syncgroups table
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

impl Group {
    pub fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self> {
        // Parse the data JSONB column
        let data_json = row.try_get::<Option<serde_json::Value>, &str>("data")?;
        let data = match data_json {
            Some(json) => Some(serde_json::from_value(json)?),
            None => None,
        };

        Ok(Self {
            id: row.try_get("id")?,
            version: row.try_get("version")?,
            created: row.try_get("created")?,
            modified: row.try_get("modified")?,
            data,
            deleted: row.try_get("deleted")?,
            item_version: row.try_get("itemversion")?,
            collection_version: row.try_get("collectionversion")?,
            tag_version: row.try_get("tagversion")?,
            gitlab: row.try_get("gitlab")?,
            active: row.try_get("active")?,
            sync_direction: match row.try_get::<&str, &str>("direction")? {
                "tocloud" => SyncDirection::ToCloud,
                "tolocal" => SyncDirection::ToLocal,
                "bothcloud" => SyncDirection::BothCloud,
                "bothlocal" => SyncDirection::BothLocal,
                "bothmanual" => SyncDirection::BothManual,
                _ => SyncDirection::None,
            },
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
            version: data.version,
            created: None,
            modified: None,
            data: Some(data.clone()),
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
        self.data.as_ref().map(|d| d.name.as_str()).unwrap_or("")
    }

    pub fn description(&self) -> Option<&str> {
        self.data.as_ref().and_then(|d| d.description.as_deref())
    }

    pub fn owner(&self) -> i64 {
        self.data.as_ref().map(|d| d.owner).unwrap_or(0)
    }

    pub fn group_type(&self) -> &str {
        self.data.as_ref().map(|d| d.group_type.as_str()).unwrap_or("")
    }

    pub fn library_reading(&self) -> &str {
        self.data.as_ref().map(|d| d.library_reading.as_str()).unwrap_or("")
    }

    pub fn library_editing(&self) -> &str {
        self.data.as_ref().map(|d| d.library_editing.as_str()).unwrap_or("")
    }

    pub fn file_editing(&self) -> &str {
        self.data.as_ref().map(|d| d.file_editing.as_str()).unwrap_or("")
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

    pub async fn clear_local(&mut self) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        // Clear group versions
        let query = format!(
            "UPDATE {}.groups SET version=0, itemversion=0, collectionversion=0, tagversion=0 WHERE id=$1",
            schema
        );
        sqlx::query(&query).bind(self.id).execute(db).await?;

        // Clear items
        let query = format!("DELETE FROM {}.items WHERE library=$1", schema);
        sqlx::query(&query).bind(self.id).execute(db).await?;

        // Clear collections
        let query = format!("DELETE FROM {}.collections WHERE library=$1", schema);
        sqlx::query(&query).bind(self.id).execute(db).await?;

        // Clear tags
        let query = format!("DELETE FROM {}.tags WHERE library=$1", schema);
        sqlx::query(&query).bind(self.id).execute(db).await?;

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

        tracing::info!("Starting sync for group {}", self.id);

        // Sync collections
        let (_, collection_version) = self.sync_collections().await?;
        
        // Upload modified items
        let (_, _item_version) = self.upload_items().await?;
        
        // Download items from cloud
        let (_, item_version) = self.download_items().await?;
        
        // Sync tags
        let (_, tag_version) = self.sync_tags().await?;
        
        // Sync deletions
        let _ = self.sync_deleted().await?;

        // Update versions if successful
        if collection_version > self.collection_version {
            self.collection_version = collection_version;
            self.is_modified = true;
        }
        if item_version > self.item_version {
            self.item_version = item_version;
            self.is_modified = true;
        }
        if tag_version > self.tag_version {
            self.tag_version = tag_version;
            self.is_modified = true;
        }

        // Update local group record
        if self.is_modified {
            self.update_local().await?;
        }

        tracing::info!("Sync completed for group {}", self.id);
        Ok(())
    }

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
            let (versions, cloud_version) = client.get_collections_version_cloud(self.id, self.collection_version).await?;
            
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
                let (collections, _) = client.get_collections_cloud(self.id, chunk).await?;
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
        let (_, mut last_modified_version) = client.get_items_version_cloud(self.id, 0, false).await?;

        // Query for items that need to be uploaded
        let query = format!(
            r#"
            SELECT key, version, data, meta, trashed, deleted, sync, md5
            FROM {}.items
            WHERE library = $1 AND (sync = 'new' OR sync = 'modified')
            ORDER BY key
            "#,
            schema
        );

        let rows = sqlx::query(&query)
            .bind(self.id)
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
                library: self.id,
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

            // The update_cloud method now handles database updates internally
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
            let (versions, cloud_version) = client.get_items_version_cloud(self.id, self.item_version, trashed).await?;
            
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
                let items = client.get_items_cloud(self.id, chunk).await?;
                for item in &items {
                    self.update_item_local(item).await?;
                    counter += 1;
                }
                
                // Download attachment files for imported_file attachments
                for item in &items {
                    if item.data.item_type == "attachment" {
                        if let Some(filesystem) = &self.filesystem {
                            if let Err(e) = item.download_attachment_cloud(client, filesystem.as_ref(), self.id).await {
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
        
        let (tags, last_modified_version) = client.get_tags_cloud(self.id, self.tag_version).await?;
        
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
        
        let (deletions, last_modified_version) = client.get_deletions_cloud(self.id, self.version).await?;
        
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

        Ok(counter)
    }

    // Helper methods for database operations
    async fn get_collection_version_local(&self, collection_key: &str) -> Result<i64> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        let query = format!("SELECT version FROM {}.collections WHERE key = $1 AND library = $2", schema);
        let result = sqlx::query(&query)
            .bind(collection_key)
            .bind(self.id)
            .fetch_optional(db)
            .await?;

        Ok(result.map(|row| row.get::<i64, _>("version")).unwrap_or(0))
    }

    async fn get_item_version_local(&self, item_key: &str) -> Result<i64> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        let query = format!("SELECT version FROM {}.items WHERE key = $1 AND library = $2", schema);
        let result = sqlx::query(&query)
            .bind(item_key)
            .bind(self.id)
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
            WHERE library = $1 AND (sync = 'new' OR sync = 'modified')
            "#,
            schema
        );

        let rows = sqlx::query(&query)
            .bind(self.id)
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
                library: self.id,
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

            // Upload to cloud using the new client-based approach
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

        let data_json = serde_json::to_string(&collection.data)?;
        let meta_json = collection.meta.as_ref()
            .map(|m| serde_json::to_string(m))
            .transpose()?;

        let query = format!(
            r#"
            INSERT INTO {}.collections (key, version, library, data, meta, deleted, sync)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (key, library) DO UPDATE SET
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
            .bind(&data_json)
            .bind(&meta_json)
            .bind(collection.deleted)
            .bind("synced")
            .execute(db)
            .await?;

        Ok(())
    }

    async fn update_item_local(&self, item: &super::Item) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        let data_json = serde_json::to_string(&item.data)?;
        let meta_json = item.meta.as_ref()
            .map(|m| serde_json::to_string(m))
            .transpose()?;

        let query = format!(
            r#"
            INSERT INTO {}.items (key, version, library, data, meta, trashed, deleted, sync, md5)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (key, library) DO UPDATE SET
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
            .bind(&data_json)
            .bind(&meta_json)
            .bind(item.trashed)
            .bind(item.deleted)
            .bind("synced")
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
        let meta_json = serde_json::to_string(&tag_meta)?;

        let query = format!(
            r#"
            INSERT INTO {}.tags (tag, meta, library)
            VALUES ($1, $2, $3)
            ON CONFLICT (tag, library) DO UPDATE SET
                meta = EXCLUDED.meta
            "#,
            schema
        );

        sqlx::query(&query)
            .bind(&tag.data.tag)
            .bind(&meta_json)
            .bind(self.id)
            .execute(db)
            .await?;

        Ok(())
    }

    async fn try_delete_item_local(&self, item_key: &str, last_modified_version: i64) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        // Check if item exists and get its current sync status
        let query = format!("SELECT sync, deleted FROM {}.items WHERE key = $1 AND library = $2", schema);
        let result = sqlx::query(&query)
            .bind(item_key)
            .bind(self.id)
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
                        "UPDATE {}.items SET version = $1, sync = 'synced' WHERE key = $2 AND library = $3",
                        schema
                    );
                    sqlx::query(&query)
                        .bind(last_modified_version)
                        .bind(item_key)
                        .bind(self.id)
                        .execute(db)
                        .await?;
                    false
                }
            };

            if should_delete {
                let query = format!(
                    "UPDATE {}.items SET deleted = true WHERE key = $1 AND library = $2",
                    schema
                );
                sqlx::query(&query)
                    .bind(item_key)
                    .bind(self.id)
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
        let query = format!("SELECT sync, deleted FROM {}.collections WHERE key = $1 AND library = $2", schema);
        let result = sqlx::query(&query)
            .bind(collection_key)
            .bind(self.id)
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
                        "UPDATE {}.collections SET version = $1, sync = 'synced' WHERE key = $2 AND library = $3",
                        schema
                    );
                    sqlx::query(&query)
                        .bind(last_modified_version)
                        .bind(collection_key)
                        .bind(self.id)
                        .execute(db)
                        .await?;
                    false
                }
            };

            if should_delete {
                let query = format!(
                    "UPDATE {}.collections SET deleted = true WHERE key = $1 AND library = $2",
                    schema
                );
                sqlx::query(&query)
                    .bind(collection_key)
                    .bind(self.id)
                    .execute(db)
                    .await?;
            }
        }

        Ok(())
    }

    async fn delete_tag_local(&self, tag_name: &str) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        let query = format!("DELETE FROM {}.tags WHERE tag = $1 AND library = $2", schema);
        sqlx::query(&query)
            .bind(tag_name)
            .bind(self.id)
            .execute(db)
            .await?;

        Ok(())
    }

    pub async fn update_local(&self) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        // Serialize the data to JSON for the JSONB column
        let data_json = match &self.data {
            Some(data) => Some(serde_json::to_string(data)?),
            None => None,
        };

        // Update the groups table
        let groups_query = format!(
            r#"
            UPDATE {}.groups 
            SET version = $2, data = $3, itemversion = $4, collectionversion = $5, 
                tagversion = $6, deleted = $7, modified = COALESCE($8, modified)
            WHERE id = $1
            "#,
            schema
        );

        sqlx::query(&groups_query)
            .bind(self.id)
            .bind(self.version)
            .bind(&data_json)
            .bind(self.item_version)
            .bind(self.collection_version)
            .bind(self.tag_version)
            .bind(self.deleted)
            .bind(self.modified)
            .execute(db)
            .await?;

        // Update the syncgroups table
        let sync_direction_str = match self.sync_direction {
            SyncDirection::None => "none",
            SyncDirection::ToCloud => "tocloud",
            SyncDirection::ToLocal => "tolocal",
            SyncDirection::BothCloud => "bothcloud",
            SyncDirection::BothLocal => "bothlocal",
            SyncDirection::BothManual => "bothmanual",
        };

        let syncgroups_query = format!(
            r#"
            INSERT INTO {}.syncgroups (id, active, direction, tags)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (id) DO UPDATE SET
                active = EXCLUDED.active,
                direction = EXCLUDED.direction,
                tags = EXCLUDED.tags
            "#,
            schema
        );

        sqlx::query(&syncgroups_query)
            .bind(self.id)
            .bind(self.active)
            .bind(sync_direction_str)
            .bind(self.sync_tags)
            .execute(db)
            .await?;

        Ok(())
    }
} 