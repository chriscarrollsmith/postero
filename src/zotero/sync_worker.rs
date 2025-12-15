//! Sync worker for event-driven outgoing sync.
//!
//! This module implements a worker process that polls the sync queue
//! and synchronizes pending changes to Zotero.

use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use sqlx::PgPool;
use tracing::{info, warn, error, debug};

use crate::{Result, Error};
use crate::filesystem::FileSystem;
use super::{
    ZoteroClient, LibraryType, SyncStatus,
    Item, Collection, ItemData, CollectionData,
    sync_queue::{SyncQueue, SyncQueueEntry},
};

/// Configuration for the sync worker
#[derive(Debug, Clone)]
pub struct SyncWorkerConfig {
    /// Interval between queue polls
    pub poll_interval: Duration,
    /// Maximum number of entries to process per poll
    pub batch_size: i32,
    /// Maximum number of concurrent libraries to process
    pub max_concurrent_libraries: usize,
    /// Days to keep processed entries before cleanup
    pub cleanup_days: i32,
}

impl Default for SyncWorkerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(5),
            batch_size: 50, // Zotero API limit
            max_concurrent_libraries: 4,
            cleanup_days: 7,
        }
    }
}

/// Worker process for event-driven sync
pub struct SyncWorker {
    client: Arc<ZoteroClient>,
    db: PgPool,
    schema: String,
    queue: SyncQueue,
    #[allow(dead_code)] // Reserved for future attachment upload support
    filesystem: Arc<dyn FileSystem>,
    config: SyncWorkerConfig,
}

impl SyncWorker {
    /// Create a new SyncWorker instance
    pub fn new(
        client: Arc<ZoteroClient>,
        db: PgPool,
        schema: String,
        filesystem: Arc<dyn FileSystem>,
        config: SyncWorkerConfig,
    ) -> Self {
        let queue = SyncQueue::new(db.clone(), schema.clone());
        Self {
            client,
            db,
            schema,
            queue,
            filesystem,
            config,
        }
    }

    /// Main worker loop - runs indefinitely
    pub async fn run(&self) -> Result<()> {
        info!(
            "Starting sync worker with poll interval {:?}, batch size {}",
            self.config.poll_interval, self.config.batch_size
        );

        let mut ticker = interval(self.config.poll_interval);
        let mut cleanup_counter = 0u64;

        loop {
            ticker.tick().await;

            // Process pending entries
            if let Err(e) = self.process_pending().await {
                error!("Error processing sync queue: {}", e);
            }

            // Periodic cleanup (every 100 iterations)
            cleanup_counter += 1;
            if cleanup_counter >= 100 {
                cleanup_counter = 0;
                if let Err(e) = self.cleanup().await {
                    warn!("Error during cleanup: {}", e);
                }
            }
        }
    }

    /// Run a single iteration (useful for testing)
    pub async fn run_once(&self) -> Result<()> {
        self.process_pending().await
    }

    /// Process all pending entries from the queue
    async fn process_pending(&self) -> Result<()> {
        // Get libraries with pending entries
        let libraries = self.queue.get_libraries_with_pending().await?;

        if libraries.is_empty() {
            debug!("No pending sync entries");
            return Ok(());
        }

        info!("Found {} libraries with pending sync entries", libraries.len());

        for (library_id, library_type) in libraries {
            if let Err(e) = self.process_library(library_id, library_type).await {
                error!(
                    "Error processing library {} ({}): {}",
                    library_id, library_type, e
                );
            }
        }

        Ok(())
    }

    /// Process pending entries for a single library
    async fn process_library(&self, library_id: i64, library_type: LibraryType) -> Result<()> {
        // Fetch pending entries
        let entries = self.queue
            .fetch_pending(library_id, library_type, self.config.batch_size)
            .await?;

        if entries.is_empty() {
            return Ok(());
        }

        info!(
            "Processing {} entries for library {} ({})",
            entries.len(),
            library_id,
            library_type
        );

        // Get current library version for API calls
        let mut library_version = self.get_library_version(library_id, library_type).await?;

        // Process entries by type (collections first, then items)
        let (collection_entries, item_entries): (Vec<_>, Vec<_>) = entries
            .into_iter()
            .partition(|e| e.entity_type == "collection");

        // Process collections first (items may reference them)
        for entry in collection_entries {
            self.process_entry(&entry, &mut library_version).await;
        }

        // Process items
        for entry in item_entries {
            self.process_entry(&entry, &mut library_version).await;
        }

        // Update library version in database
        self.update_library_version(library_id, library_type, library_version).await?;

        Ok(())
    }

    /// Process a single queue entry
    async fn process_entry(&self, entry: &SyncQueueEntry, library_version: &mut i64) {
        debug!(
            "Processing {} {} ({}) for library {}",
            entry.operation, entry.entity_type, entry.entity_key, entry.library_id
        );

        let result = match entry.entity_type.as_str() {
            "item" => self.sync_item(entry, library_version).await,
            "collection" => self.sync_collection(entry, library_version).await,
            _ => Err(Error::InvalidData(format!(
                "Unknown entity type: {}",
                entry.entity_type
            ))),
        };

        match result {
            Ok(()) => {
                debug!("Successfully synced {} {}", entry.entity_type, entry.entity_key);
                if let Err(e) = self.queue.mark_completed(entry.id).await {
                    error!("Failed to mark entry {} as completed: {}", entry.id, e);
                }
            }
            Err(e) => {
                warn!(
                    "Failed to sync {} {}: {}",
                    entry.entity_type, entry.entity_key, e
                );
                if let Err(e2) = self.queue.mark_failed(entry.id, &e.to_string()).await {
                    error!("Failed to mark entry {} as failed: {}", entry.id, e2);
                }
            }
        }
    }

    /// Sync a single item to Zotero
    async fn sync_item(&self, entry: &SyncQueueEntry, library_version: &mut i64) -> Result<()> {
        // Handle delete operations specially
        if entry.operation == "delete" {
            let new_version = self.client
                .delete_item_unified(entry.library_id, entry.library_type, &entry.entity_key, *library_version)
                .await?;
            *library_version = new_version;

            // Remove from local database
            let query = format!(
                "DELETE FROM {}.items WHERE key = $1 AND library_id = $2 AND library_type = $3",
                self.schema
            );
            sqlx::query(&query)
                .bind(&entry.entity_key)
                .bind(entry.library_id)
                .bind(entry.library_type)
                .execute(&self.db)
                .await?;

            return Ok(());
        }

        // Load item from database
        let mut item = self.load_item(&entry.entity_key, entry.library_id, entry.library_type).await?;

        // Upload to Zotero
        item.update_cloud(&self.client, library_version).await?;

        Ok(())
    }

    /// Sync a single collection to Zotero
    async fn sync_collection(&self, entry: &SyncQueueEntry, library_version: &mut i64) -> Result<()> {
        // Handle delete operations specially
        if entry.operation == "delete" {
            let new_version = self.client
                .delete_collection_unified(entry.library_id, entry.library_type, &entry.entity_key, *library_version)
                .await?;
            *library_version = new_version;

            // Remove from local database
            let query = format!(
                "DELETE FROM {}.collections WHERE key = $1 AND library_id = $2 AND library_type = $3",
                self.schema
            );
            sqlx::query(&query)
                .bind(&entry.entity_key)
                .bind(entry.library_id)
                .bind(entry.library_type)
                .execute(&self.db)
                .await?;

            return Ok(());
        }

        // Load collection from database
        let mut collection = self.load_collection(&entry.entity_key, entry.library_id, entry.library_type).await?;

        // Upload to Zotero
        let new_version = collection.update_cloud(&self.client, *library_version).await?;
        *library_version = new_version;

        Ok(())
    }

    /// Load an item from the database
    async fn load_item(&self, key: &str, library_id: i64, library_type: LibraryType) -> Result<Item> {
        let query = format!(
            r#"
            SELECT key, version, data, meta, trashed, deleted, sync::TEXT as sync, md5
            FROM {}.items
            WHERE key = $1 AND library_id = $2 AND library_type = $3
            "#,
            self.schema
        );

        let row = sqlx::query(&query)
            .bind(key)
            .bind(library_id)
            .bind(library_type)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Item {} not found", key)))?;

        use sqlx::Row;
        let key: String = row.get("key");
        let version: i64 = row.get("version");
        let data_value: serde_json::Value = row.get("data");
        let meta_value: Option<serde_json::Value> = row.get("meta");
        let trashed: bool = row.get("trashed");
        let deleted: bool = row.get("deleted");
        let sync_status: String = row.get("sync");
        let md5: Option<String> = row.get("md5");

        let item_data: ItemData = serde_json::from_value(data_value)?;
        let item_meta: Option<super::item::ItemMeta> = meta_value
            .map(|v| serde_json::from_value(v))
            .transpose()?;

        Ok(Item {
            key,
            version,
            library_id,
            library_type,
            data: item_data,
            meta: item_meta,
            trashed,
            deleted,
            sync_status: match sync_status.as_str() {
                "new" => SyncStatus::New,
                "modified" => SyncStatus::Modified,
                "incomplete" => SyncStatus::Incomplete,
                _ => SyncStatus::Synced,
            },
            md5,
            db: Some(self.db.clone()),
            db_schema: Some(self.schema.clone()),
        })
    }

    /// Load a collection from the database
    async fn load_collection(&self, key: &str, library_id: i64, library_type: LibraryType) -> Result<Collection> {
        let query = format!(
            r#"
            SELECT key, version, data, meta, deleted, sync::TEXT as sync
            FROM {}.collections
            WHERE key = $1 AND library_id = $2 AND library_type = $3
            "#,
            self.schema
        );

        let row = sqlx::query(&query)
            .bind(key)
            .bind(library_id)
            .bind(library_type)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Collection {} not found", key)))?;

        use sqlx::Row;
        let key: String = row.get("key");
        let version: i64 = row.get("version");
        let data_value: serde_json::Value = row.get("data");
        let meta_value: Option<serde_json::Value> = row.get("meta");
        let deleted: bool = row.get("deleted");
        let sync_status: String = row.get("sync");

        let collection_data: CollectionData = serde_json::from_value(data_value)?;
        let collection_meta: Option<super::collection::CollectionMeta> = meta_value
            .map(|v| serde_json::from_value(v))
            .transpose()?;

        Ok(Collection {
            key,
            version,
            library_id,
            library_type,
            data: collection_data,
            meta: collection_meta,
            deleted,
            sync_status: match sync_status.as_str() {
                "new" => SyncStatus::New,
                "modified" => SyncStatus::Modified,
                "incomplete" => SyncStatus::Incomplete,
                _ => SyncStatus::Synced,
            },
            db: Some(self.db.clone()),
            db_schema: Some(self.schema.clone()),
        })
    }

    /// Get the current item version for a library
    async fn get_library_version(&self, library_id: i64, library_type: LibraryType) -> Result<i64> {
        let query = format!(
            "SELECT item_version FROM {}.libraries WHERE id = $1 AND library_type = $2",
            self.schema
        );

        let row = sqlx::query(&query)
            .bind(library_id)
            .bind(library_type)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Library {} not found", library_id)))?;

        use sqlx::Row;
        let version: i64 = row.get("item_version");
        Ok(version)
    }

    /// Update the library version in the database
    async fn update_library_version(
        &self,
        library_id: i64,
        library_type: LibraryType,
        version: i64,
    ) -> Result<()> {
        let query = format!(
            "UPDATE {}.libraries SET item_version = $1 WHERE id = $2 AND library_type = $3",
            self.schema
        );

        sqlx::query(&query)
            .bind(version)
            .bind(library_id)
            .bind(library_type)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    /// Cleanup old processed entries
    async fn cleanup(&self) -> Result<()> {
        let deleted = self.queue.cleanup_old_entries(self.config.cleanup_days).await?;
        if deleted > 0 {
            info!("Cleaned up {} old sync queue entries", deleted);
        }
        Ok(())
    }

    /// Get queue statistics
    pub async fn get_stats(&self) -> Result<super::sync_queue::QueueStats> {
        self.queue.get_stats().await
    }
}
