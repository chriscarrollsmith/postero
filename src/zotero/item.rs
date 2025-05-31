use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use crate::{Result, Error};
use super::{ItemData, SyncStatus};
use crate::filesystem::{FileSystem, FileGetOptions, FilePutOptions};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub key: String,
    pub version: i64,
    pub library: i64,
    pub data: ItemData,
    pub meta: Option<ItemMeta>,
    pub trashed: bool,
    pub deleted: bool,
    pub sync_status: SyncStatus,
    pub md5: Option<String>,
    
    #[serde(skip)]
    pub db: Option<PgPool>,
    #[serde(skip)]
    pub db_schema: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemMeta {
    #[serde(rename = "createdByUser")]
    pub created_by_user: Option<super::UserData>,
    #[serde(rename = "creatorSummary")]
    pub creator_summary: Option<String>,
    #[serde(rename = "parsedDate")]
    pub parsed_date: Option<String>,
    #[serde(rename = "numChildren")]
    pub num_children: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemType {
    Book,
    Article,
    Chapter,
    Note,
    Attachment,
    Document,
    #[serde(other)]
    Other,
}

impl Item {
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
            .bind(&self.key)
            .bind(self.version)
            .bind(self.library)
            .bind(&data_json)
            .bind(&meta_json)
            .bind(self.trashed)
            .bind(self.deleted)
            .bind(sync_status_str)
            .bind(&self.md5)
            .execute(db)
            .await?;

        Ok(())
    }

    pub async fn update_cloud(&mut self, client: &super::ZoteroClient, library_version: &mut i64) -> Result<()> {
        // Check if item is marked for deletion
        if self.deleted {
            // Delete item from Zotero API
            let new_version = client.delete_item(self.library, &self.key, *library_version).await?;
            *library_version = new_version;
            
            // Remove from local database
            if let (Some(db), Some(schema)) = (&self.db, &self.db_schema) {
                let query = format!(
                    "DELETE FROM {}.items WHERE key = $1 AND library = $2",
                    schema
                );
                sqlx::query(&query)
                    .bind(&self.key)
                    .bind(self.library)
                    .execute(db)
                    .await?;
            }
            return Ok(());
        }

        match self.sync_status {
            SyncStatus::New | SyncStatus::Modified => {
                // Upload item to Zotero API
                let new_version = client.upload_item(self.library, self, *library_version).await?;
                
                // Update local status
                self.sync_status = SyncStatus::Synced;
                self.version = new_version;
                *library_version = new_version;
                
                // Handle file upload for imported_file attachments
                if self.data.item_type == "attachment" && 
                   self.data.extra_fields.get("linkMode").and_then(|v| v.as_str()) == Some("imported_file") {
                    // TODO: File upload requires filesystem and file_path parameters
                    // self.upload_file_cloud(client, filesystem, group_id, file_path).await?;
                    tracing::info!("Attachment upload skipped - requires filesystem context");
                }
                
                // Update local database
                if let (Some(db), Some(schema)) = (&self.db, &self.db_schema) {
                    let query = format!(
                        "UPDATE {}.items SET sync = 'synced', version = $1 WHERE key = $2 AND library = $3",
                        schema
                    );
                    sqlx::query(&query)
                        .bind(self.version)
                        .bind(&self.key)
                        .bind(self.library)
                        .execute(db)
                        .await?;
                }
            }
            
            SyncStatus::Synced => {
                // Already synchronized, nothing to do
                tracing::debug!("Item {} already synchronized", self.key);
            }

            SyncStatus::Incomplete => {
                // Handle incomplete sync - might need to retry
                tracing::warn!("Item {} has incomplete sync status", self.key);
            }
        }
        
        Ok(())
    }

    pub async fn download_attachment_cloud(
        &self,
        client: &super::ZoteroClient,
        filesystem: &dyn FileSystem,
        group_id: i64,
    ) -> Result<()> {
        // Only process attachment items with linked_file or imported_file link modes
        let link_mode = self.data.extra_fields.get("linkMode")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if link_mode != "linked_file" && link_mode != "imported_file" {
            tracing::debug!("Skipping non-file attachment: {}", self.key);
            return Ok(());
        }

        // Get filename from item data
        let filename = self.data.extra_fields.get("filename")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let s3_key = format!("attachments/{}/{}", self.key, filename);

        // Check if file already exists in S3 with correct MD5
        let cloud_md5 = self.data.extra_fields.get("md5")
            .and_then(|v| v.as_str());

        if let Some(expected_md5) = cloud_md5 {
            let folder = format!("attachments/{}", self.key);
            if let Ok(existing_data) = filesystem.file_get(&folder, filename, FileGetOptions::default()).await {
                let actual_md5 = format!("{:x}", md5::compute(&existing_data));
                if actual_md5 == expected_md5 {
                    tracing::debug!("File already exists with correct MD5: {}/{}", folder, filename);
                    return Ok(());
                }
            }
        }

        // Get download URL from Zotero API
        let download_url = match client.get_attachment_download_url(group_id, &self.key).await {
            Ok(url) => url,
            Err(Error::Api { code: 404, .. }) => {
                tracing::warn!("Attachment file not found in Zotero: {}", self.key);
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        // Download file data
        tracing::info!("Downloading attachment: {} -> {}", self.key, s3_key);
        let file_data = client.download_file(&download_url).await?;

        // Verify MD5 if provided
        if let Some(expected_md5) = cloud_md5 {
            let actual_md5 = format!("{:x}", md5::compute(&file_data));
            if actual_md5 != expected_md5 {
                return Err(Error::Validation(format!(
                    "MD5 mismatch for {}: expected {}, got {}",
                    self.key, expected_md5, actual_md5
                )));
            }
        }

        // Upload to S3
        let folder = format!("attachments/{}", self.key);
        filesystem.file_put(&folder, filename, &file_data, FilePutOptions::default()).await?;

        tracing::info!("Successfully downloaded attachment: {} ({} bytes)", self.key, file_data.len());
        Ok(())
    }

    pub async fn upload_file_cloud(
        &self,
        client: &super::ZoteroClient,
        filesystem: &dyn FileSystem,
        group_id: i64,
        file_path: &str,
    ) -> Result<()> {
        // Only process attachment items
        if self.data.item_type != "attachment" {
            return Err(Error::Validation("Item is not an attachment".to_string()));
        }

        // Parse file path to extract folder and filename
        let path_parts: Vec<&str> = file_path.split('/').collect();
        if path_parts.len() < 2 {
            return Err(Error::Validation("Invalid file path format".to_string()));
        }
        
        let folder = path_parts[..path_parts.len()-1].join("/");
        let filename = path_parts[path_parts.len()-1];

        // Read file from S3
        let file_data = filesystem.file_get(&folder, filename, FileGetOptions::default()).await?;
        
        // Calculate MD5
        let md5_hash = format!("{:x}", md5::compute(&file_data));
        
        // Get file modification time (use current time as fallback)
        let mtime = chrono::Utc::now().timestamp();

        // Get upload authorization from Zotero
        let auth = client.get_upload_authorization(
            group_id,
            &self.key,
            filename,
            file_data.len(),
            &Some(md5_hash.clone()),
            Some(mtime),
        ).await?;

        // If file already exists on Zotero, no upload needed
        if auth.exists {
            tracing::info!("File already exists in Zotero: {}", self.key);
            return Ok(());
        }

        // Upload file to Zotero's storage
        if let (Some(upload_url), Some(upload_key), Some(params)) = 
            (auth.upload_url, auth.upload_key, auth.params) {
            
            tracing::info!("Uploading file to Zotero: {} ({} bytes)", self.key, file_data.len());
            
            // Upload file data
            client.upload_file_to_url(&upload_url, &file_data, &params).await?;
            
            // Register upload completion
            client.register_upload_completion(group_id, &self.key, &upload_key).await?;
            
            tracing::info!("Successfully uploaded file: {}", self.key);
        } else {
            return Err(Error::Api {
                code: 500,
                message: "Invalid upload authorization response".to_string(),
            });
        }

        Ok(())
    }
} 