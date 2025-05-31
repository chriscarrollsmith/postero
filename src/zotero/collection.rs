use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use crate::{Result, Error};
use super::{CollectionData, SyncStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub key: String,
    pub version: i64,
    pub library: i64,
    pub data: CollectionData,
    pub meta: Option<CollectionMeta>,
    pub deleted: bool,
    pub sync_status: SyncStatus,
    
    #[serde(skip)]
    pub db: Option<PgPool>,
    #[serde(skip)]
    pub db_schema: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionMeta {
    #[serde(rename = "numCollections")]
    pub num_collections: i32,
    #[serde(rename = "numItems")]
    pub num_items: i32,
}

impl Collection {
    pub fn set_db(&mut self, db: PgPool, db_schema: String) {
        self.db = Some(db);
        self.db_schema = Some(db_schema);
    }

    pub async fn update_local(&self) -> Result<()> {
        let db = self.db.as_ref().ok_or_else(|| Error::InvalidData("Database not set".to_string()))?;
        let schema = self.db_schema.as_ref().ok_or_else(|| Error::InvalidData("Schema not set".to_string()))?;

        let data_json = serde_json::to_string(&self.data)?;
        let meta_json = self.meta.as_ref()
            .map(|m| serde_json::to_string(m))
            .transpose()?;

        let sync_status_str = match self.sync_status {
            SyncStatus::New => "new",
            SyncStatus::Modified => "modified", 
            SyncStatus::Synced => "synced",
            SyncStatus::Incomplete => "incomplete",
        };

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
            .bind(&self.key)
            .bind(self.version)
            .bind(self.library)
            .bind(&data_json)
            .bind(&meta_json)
            .bind(self.deleted)
            .bind(sync_status_str)
            .execute(db)
            .await?;

        Ok(())
    }

    pub async fn update_cloud(&mut self, client: &super::ZoteroClient, library_version: i64) -> Result<i64> {
        // Check if collection is marked for deletion
        if self.deleted {
            // Delete collection from Zotero API
            let new_version = client.delete_collection(self.library, &self.key, library_version).await?;
            
            // Remove from local database
            if let (Some(db), Some(schema)) = (&self.db, &self.db_schema) {
                let query = format!(
                    "DELETE FROM {}.collections WHERE key = $1 AND library = $2",
                    schema
                );
                sqlx::query(&query)
                    .bind(&self.key)
                    .bind(self.library)
                    .execute(db)
                    .await?;
            }
            
            return Ok(new_version);
        }

        match self.sync_status {
            SyncStatus::New | SyncStatus::Modified => {
                // Upload collection to Zotero API
                let new_version = client.upload_collection(self.library, self, library_version).await?;
                
                // Update local status
                self.sync_status = SyncStatus::Synced;
                self.version = new_version;
                
                // Update local database
                if let (Some(db), Some(schema)) = (&self.db, &self.db_schema) {
                    let query = format!(
                        "UPDATE {}.collections SET sync = 'synced', version = $1 WHERE key = $2 AND library = $3",
                        schema
                    );
                    sqlx::query(&query)
                        .bind(self.version)
                        .bind(&self.key)
                        .bind(self.library)
                        .execute(db)
                        .await?;
                }
                
                Ok(new_version)
            }
            
            SyncStatus::Synced => {
                // Already synchronized, nothing to do
                tracing::debug!("Collection {} already synchronized", self.key);
                Ok(library_version)
            }

            SyncStatus::Incomplete => {
                // Handle incomplete sync - might need to retry
                tracing::warn!("Collection {} has incomplete sync status", self.key);
                Ok(library_version)
            }
        }
    }
} 