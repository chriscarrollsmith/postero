use reqwest::{Client, header::{HeaderMap, HeaderValue}};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use url::Url;
use crate::{Error, Result};
use crate::filesystem::FileSystem;
use super::{Group, ApiKey, UploadAuthorization, UploadAuthorizationResponse};
use serde_json;

#[derive(Debug, Clone)]
pub struct ZoteroClient {
    client: Client,
    base_url: Url,
    db: PgPool,
    db_schema: String,
    fs: Arc<dyn FileSystem>,
    new_group_active: bool,
    current_key: Option<ApiKey>,
}

impl ZoteroClient {
    pub async fn new(
        base_url: &str,
        api_key: &str,
        db: PgPool,
        fs: Arc<dyn FileSystem>,
        db_schema: &str,
        new_group_active: bool,
    ) -> Result<Self> {
        let base_url = Url::parse(base_url)?;
        
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization", 
            HeaderValue::from_str(&format!("Bearer {}", api_key))
                .map_err(|e| Error::InvalidData(format!("Invalid API key: {}", e)))?
        );
        headers.insert(
            "Zotero-API-Version", 
            HeaderValue::from_static("3")
        );

        let client = Client::builder()
            .default_headers(headers)
            .build()?;

        let mut zotero = Self {
            client,
            base_url,
            db,
            db_schema: db_schema.to_string(),
            fs,
            new_group_active,
            current_key: None,
        };

        zotero.init().await?;
        Ok(zotero)
    }

    async fn init(&mut self) -> Result<()> {
        // Load current API key info
        self.current_key = Some(self.get_api_key_info().await?);
        Ok(())
    }

    pub fn current_key(&self) -> Option<&ApiKey> {
        self.current_key.as_ref()
    }

    pub fn filesystem(&self) -> &Arc<dyn FileSystem> {
        &self.fs
    }

    pub async fn get_api_key_info(&self) -> Result<ApiKey> {
        let url = self.base_url.join("keys/current")?;
        let response = self.client.get(url).send().await?;
        
        if !response.status().is_success() {
            return Err(Error::Api {
                code: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let api_key: ApiKey = response.json().await?;
        Ok(api_key)
    }

    pub async fn get_user_group_versions(&self, user_id: i64) -> Result<HashMap<i64, i64>> {
        let url = self.base_url.join(&format!("users/{}/groups", user_id))?;
        
        let response = self.client
            .get(url)
            .query(&[("format", "versions")])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Api {
                code: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let versions: HashMap<String, i64> = response.json().await?;
        let mut result = HashMap::new();
        
        for (key, version) in versions {
            if let Ok(group_id) = key.parse::<i64>() {
                result.insert(group_id, version);
            }
        }

        Ok(result)
    }

    pub async fn get_group_cloud(&self, group_id: i64) -> Result<Group> {
        let url = self.base_url.join(&format!("groups/{}", group_id))?;
        
        let response = self.client.get(url).send().await?;
        
        if !response.status().is_success() {
            return Err(Error::Api {
                code: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let group: Group = response.json().await?;
        Ok(group)
    }

    pub async fn load_group_local(&self, group_id: i64) -> Result<Group> {
        let query = format!(
            r#"
            SELECT g.id, g.version, g.created, g.modified, g.data, g.deleted, 
                   g.itemversion, g.collectionversion, g.tagversion, g.gitlab,
                   sg.active, sg.direction, sg.tags
            FROM {}.groups g, {}.syncgroups sg 
            WHERE g.id = sg.id AND g.id = $1
            "#,
            self.db_schema, self.db_schema
        );

        let row = sqlx::query(&query)
            .bind(group_id)
            .fetch_one(&self.db)
            .await?;

        Group::from_row(&row)
    }

    pub async fn load_groups_local(&self) -> Result<Vec<Group>> {
        let query = format!(
            r#"
            SELECT g.id, g.version, g.created, g.modified, g.data, g.deleted, 
                   g.itemversion, g.collectionversion, g.tagversion, g.gitlab,
                   sg.active, sg.direction, sg.tags
            FROM {}.groups g, {}.syncgroups sg 
            WHERE g.id = sg.id
            ORDER BY g.data->>'name'
            "#,
            self.db_schema, self.db_schema
        );

        let rows = sqlx::query(&query).fetch_all(&self.db).await?;
        
        let mut groups = Vec::new();
        for row in rows {
            groups.push(Group::from_row(&row)?);
        }

        Ok(groups)
    }

    pub async fn create_empty_group_local(&self, group_id: i64) -> Result<(bool, super::SyncDirection)> {
        // First, try to create the basic group record
        let groups_query = format!(
            r#"
            INSERT INTO {}.groups (id, version, created, modified)
            VALUES ($1, 0, NOW(), NOW())
            ON CONFLICT (id) DO NOTHING
            "#,
            self.db_schema
        );

        let result = sqlx::query(&groups_query)
            .bind(group_id)
            .execute(&self.db)
            .await?;

        let was_created = result.rows_affected() > 0;

        // Now create or update the syncgroups entry
        let direction = super::SyncDirection::ToLocal;
        let direction_str = match direction {
            super::SyncDirection::None => "none",
            super::SyncDirection::ToCloud => "tocloud",
            super::SyncDirection::ToLocal => "tolocal",
            super::SyncDirection::BothCloud => "bothcloud",
            super::SyncDirection::BothLocal => "bothlocal",
            super::SyncDirection::BothManual => "bothmanual",
        };

        let syncgroups_query = format!(
            r#"
            INSERT INTO {}.syncgroups (id, active, direction, tags)
            VALUES ($1, $2, $3, false)
            ON CONFLICT (id) DO UPDATE SET
                active = EXCLUDED.active,
                direction = EXCLUDED.direction
            "#,
            self.db_schema
        );

        sqlx::query(&syncgroups_query)
            .bind(group_id)
            .bind(self.new_group_active)
            .bind(direction_str)
            .execute(&self.db)
            .await?;

        Ok((was_created, direction))
    }

    pub async fn delete_unknown_groups_local(&self, known_groups: &[i64]) -> Result<()> {
        if known_groups.is_empty() {
            return Ok(());
        }

        let placeholders: String = (1..=known_groups.len())
            .map(|i| format!("${}", i))
            .collect::<Vec<_>>()
            .join(", ");

        let query = format!(
            "DELETE FROM {}.groups WHERE id NOT IN ({})",
            self.db_schema, placeholders
        );

        let mut query_builder = sqlx::query(&query);
        for &group_id in known_groups {
            query_builder = query_builder.bind(group_id);
        }

        query_builder.execute(&self.db).await?;
        Ok(())
    }

    pub fn check_retry(&self, headers: &HeaderMap) -> bool {
        headers.get("retry-after").is_some()
    }

    pub async fn handle_retry(&self, headers: &HeaderMap) -> Result<()> {
        if let Some(retry_after) = headers.get("retry-after") {
            if let Ok(retry_str) = retry_after.to_str() {
                if let Ok(retry_secs) = retry_str.parse::<u64>() {
                    tracing::warn!("Rate limited, waiting {} seconds before retry", retry_secs);
                    tokio::time::sleep(tokio::time::Duration::from_secs(retry_secs)).await;
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    pub fn check_backoff(&self, headers: &HeaderMap) -> bool {
        headers.get("backoff").is_some()
    }

    pub async fn handle_backoff(&self, headers: &HeaderMap) -> Result<()> {
        if let Some(backoff) = headers.get("backoff") {
            if let Ok(backoff_str) = backoff.to_str() {
                if let Ok(backoff_secs) = backoff_str.parse::<u64>() {
                    tracing::warn!("Backoff requested, waiting {} seconds", backoff_secs);
                    tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    // Helper method to handle rate limiting for any API call
    pub async fn handle_rate_limiting(&self, response: &reqwest::Response) -> Result<()> {
        let headers = response.headers();
        
        // Handle backoff first (more urgent)
        if self.check_backoff(headers) {
            self.handle_backoff(headers).await?;
        }
        
        // Handle retry-after
        if self.check_retry(headers) {
            self.handle_retry(headers).await?;
        }
        
        Ok(())
    }

    pub async fn delete_collection_db(&self, key: &str) -> Result<()> {
        let query = format!("DELETE FROM {}.collections WHERE key = $1", self.db_schema);
        sqlx::query(&query).bind(key).execute(&self.db).await?;
        Ok(())
    }

    // Collection API methods
    pub async fn get_collections_version_cloud(&self, group_id: i64, since_version: i64) -> Result<(std::collections::HashMap<String, i64>, i64)> {
        let url = self.base_url.join(&format!("groups/{}/collections", group_id))?;
        
        let response = self.client
            .get(url)
            .query(&[("format", "versions"), ("since", &since_version.to_string())])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Api {
                code: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let last_modified_version = response
            .headers()
            .get("Last-Modified-Version")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(since_version);

        let versions: std::collections::HashMap<String, i64> = response.json().await?;
        Ok((versions, last_modified_version))
    }

    pub async fn get_collections_cloud(&self, group_id: i64, collection_keys: &[String]) -> Result<(Vec<super::Collection>, i64)> {
        if collection_keys.is_empty() {
            return Ok((Vec::new(), 0));
        }

        let url = self.base_url.join(&format!("groups/{}/collections", group_id))?;
        let keys = collection_keys.join(",");
        
        let response = self.client
            .get(url)
            .query(&[("collectionKey", &keys)])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Api {
                code: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let last_modified_version = response
            .headers()
            .get("Last-Modified-Version")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let collections: Vec<super::Collection> = response.json().await?;
        Ok((collections, last_modified_version))
    }

    // Item API methods
    pub async fn get_items_version_cloud(&self, group_id: i64, since_version: i64, trashed: bool) -> Result<(std::collections::HashMap<String, i64>, i64)> {
        let url = self.base_url.join(&format!("groups/{}/items", group_id))?;
        
        let mut params = vec![
            ("format", "versions".to_string()),
            ("since", since_version.to_string()),
        ];
        
        if trashed {
            params.push(("trashed", "1".to_string()));
        }
        
        let response = self.client
            .get(url)
            .query(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Api {
                code: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let last_modified_version = response
            .headers()
            .get("Last-Modified-Version")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(since_version);

        let versions: std::collections::HashMap<String, i64> = response.json().await?;
        Ok((versions, last_modified_version))
    }

    pub async fn get_items_cloud(&self, group_id: i64, item_keys: &[String]) -> Result<Vec<super::Item>> {
        if item_keys.is_empty() {
            return Ok(Vec::new());
        }

        let url = self.base_url.join(&format!("groups/{}/items", group_id))?;
        let keys = item_keys.join(",");
        
        let response = self.client
            .get(url)
            .query(&[("itemKey", &keys)])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Api {
                code: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let items: Vec<super::Item> = response.json().await?;
        Ok(items)
    }

    // Tag API methods
    pub async fn get_tags_cloud(&self, group_id: i64, since_version: i64) -> Result<(Vec<super::Tag>, i64)> {
        let url = self.base_url.join(&format!("groups/{}/tags", group_id))?;
        
        let response = self.client
            .get(url)
            .query(&[("since", &since_version.to_string())])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Api {
                code: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let last_modified_version = response
            .headers()
            .get("Last-Modified-Version")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(since_version);

        let tags: Vec<super::Tag> = response.json().await?;
        Ok((tags, last_modified_version))
    }

    // Deletion API methods
    pub async fn get_deletions_cloud(&self, group_id: i64, since_version: i64) -> Result<(super::Deletions, i64)> {
        let url = self.base_url.join(&format!("groups/{}/deleted", group_id))?;
        
        let response = self.client
            .get(url)
            .query(&[("since", &since_version.to_string())])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Api {
                code: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        let last_modified_version = response
            .headers()
            .get("Last-Modified-Version")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse().ok())
            .unwrap_or(since_version);

        let deletions: super::Deletions = response.json().await?;
        Ok((deletions, last_modified_version))
    }

    // Upload API methods
    pub async fn upload_collection(&self, group_id: i64, collection: &super::Collection, library_version: i64) -> Result<i64> {
        let url = if collection.sync_status == super::SyncStatus::New {
            // POST for new collections
            self.base_url.join(&format!("groups/{}/collections", group_id))?
        } else {
            // PUT for existing collections
            self.base_url.join(&format!("groups/{}/collections/{}", group_id, collection.key))?
        };

        // Prepare collection data for API
        let api_data = serde_json::json!({
            "name": collection.data.name,
            "parentCollection": collection.data.parent_collection,
            "relations": collection.data.relations
        });

        let mut request = if collection.sync_status == super::SyncStatus::New {
            self.client.post(url)
        } else {
            self.client.put(url)
        };

        request = request
            .header("If-Unmodified-Since-Version", library_version.to_string())
            .json(&api_data);

        let response = request.send().await?;

        // Handle rate limiting
        self.handle_rate_limiting(&response).await?;

        match response.status().as_u16() {
            200 | 201 => {
                // Success - parse new version from response
                let new_version = response
                    .headers()
                    .get("Last-Modified-Version")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(library_version + 1);
                Ok(new_version)
            }
            304 => {
                // Not modified
                Ok(library_version)
            }
            412 => {
                // Precondition failed - conflict
                Err(Error::Api {
                    code: 412,
                    message: "Collection has been modified remotely. Sync required.".to_string(),
                })
            }
            429 | 503 => {
                // Rate limited or service unavailable - should have been handled by handle_rate_limiting
                Err(Error::Api {
                    code: response.status().as_u16(),
                    message: "Rate limited or service unavailable".to_string(),
                })
            }
            _ => {
                Err(Error::Api {
                    code: response.status().as_u16(),
                    message: response.text().await.unwrap_or_default(),
                })
            }
        }
    }

    pub async fn upload_item(&self, group_id: i64, item: &super::Item, library_version: i64) -> Result<i64> {
        let url = if item.sync_status == super::SyncStatus::New {
            // POST for new items
            self.base_url.join(&format!("groups/{}/items", group_id))?
        } else {
            // PUT for existing items
            self.base_url.join(&format!("groups/{}/items/{}", group_id, item.key))?
        };

        // Start with basic item data
        let mut api_data = serde_json::json!({
            "itemType": item.data.item_type,
            "tags": item.data.tags,
            "collections": item.data.collections,
            "relations": item.data.relations
        });

        // Add title if present
        if let Some(ref title) = item.data.title {
            api_data["title"] = serde_json::Value::String(title.clone());
        }

        // Add creators if present
        if let Some(ref creators) = item.data.creators {
            api_data["creators"] = serde_json::to_value(creators)?;
        }

        // Add date if present
        if let Some(ref date) = item.data.date {
            api_data["date"] = serde_json::Value::String(date.clone());
        }

        // Add all extra fields from the flattened map
        for (key, value) in &item.data.extra_fields {
            api_data[key] = value.clone();
        }

        // Add MD5 if this is an attachment and MD5 is available
        if item.data.item_type == "attachment" {
            if let Some(ref md5) = item.md5 {
                api_data["md5"] = serde_json::Value::String(md5.clone());
            }
        }

        let mut request = if item.sync_status == super::SyncStatus::New {
            self.client.post(url)
        } else {
            self.client.put(url)
        };

        request = request
            .header("If-Unmodified-Since-Version", library_version.to_string())
            .json(&api_data);

        let response = request.send().await?;

        // Handle rate limiting
        self.handle_rate_limiting(&response).await?;

        match response.status().as_u16() {
            200 | 201 => {
                // Success - parse new version from response
                let new_version = response
                    .headers()
                    .get("Last-Modified-Version")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(library_version + 1);
                Ok(new_version)
            }
            304 => {
                // Not modified
                Ok(library_version)
            }
            412 => {
                // Precondition failed - conflict
                Err(Error::Api {
                    code: 412,
                    message: "Item has been modified remotely. Sync required.".to_string(),
                })
            }
            413 => {
                // Request entity too large - file upload needed
                Err(Error::Api {
                    code: 413,
                    message: "Item too large. File upload required for attachments.".to_string(),
                })
            }
            429 | 503 => {
                // Rate limited or service unavailable
                Err(Error::Api {
                    code: response.status().as_u16(),
                    message: "Rate limited or service unavailable".to_string(),
                })
            }
            _ => {
                Err(Error::Api {
                    code: response.status().as_u16(),
                    message: response.text().await.unwrap_or_default(),
                })
            }
        }
    }

    pub async fn delete_collection(&self, group_id: i64, collection_key: &str, library_version: i64) -> Result<i64> {
        let url = self.base_url.join(&format!("groups/{}/collections/{}", group_id, collection_key))?;

        let response = self.client
            .delete(url)
            .header("If-Unmodified-Since-Version", library_version.to_string())
            .send()
            .await?;

        // Handle rate limiting
        self.handle_rate_limiting(&response).await?;

        match response.status().as_u16() {
            204 => {
                // Success - deleted
                let new_version = response
                    .headers()
                    .get("Last-Modified-Version")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(library_version + 1);
                Ok(new_version)
            }
            412 => {
                // Precondition failed - conflict
                Err(Error::Api {
                    code: 412,
                    message: "Collection has been modified remotely. Sync required.".to_string(),
                })
            }
            _ => {
                Err(Error::Api {
                    code: response.status().as_u16(),
                    message: response.text().await.unwrap_or_default(),
                })
            }
        }
    }

    pub async fn delete_item(&self, group_id: i64, item_key: &str, library_version: i64) -> Result<i64> {
        let url = self.base_url.join(&format!("groups/{}/items/{}", group_id, item_key))?;

        let response = self.client
            .delete(url)
            .header("If-Unmodified-Since-Version", library_version.to_string())
            .send()
            .await?;

        // Handle rate limiting
        self.handle_rate_limiting(&response).await?;

        match response.status().as_u16() {
            204 => {
                // Success - deleted
                let new_version = response
                    .headers()
                    .get("Last-Modified-Version")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(library_version + 1);
                Ok(new_version)
            }
            412 => {
                // Precondition failed - conflict
                Err(Error::Api {
                    code: 412,
                    message: "Item has been modified remotely. Sync required.".to_string(),
                })
            }
            _ => {
                Err(Error::Api {
                    code: response.status().as_u16(),
                    message: response.text().await.unwrap_or_default(),
                })
            }
        }
    }

    // File attachment API methods
    pub async fn get_attachment_download_url(&self, group_id: i64, item_key: &str) -> Result<String> {
        let url = self.base_url.join(&format!("groups/{}/items/{}/file", group_id, item_key))?;
        
        let response = self.client
            .get(url)
            .send()
            .await?;

        // Handle rate limiting
        self.handle_rate_limiting(&response).await?;

        match response.status().as_u16() {
            302 => {
                // Redirect to download URL
                if let Some(location) = response.headers().get("location") {
                    match location.to_str() {
                        Ok(url) => Ok(url.to_string()),
                        Err(_) => Err(Error::Api {
                            code: 302,
                            message: "Invalid location header encoding".to_string(),
                        })
                    }
                } else {
                    Err(Error::Api {
                        code: 302,
                        message: "Redirect response missing location header".to_string(),
                    })
                }
            }
            404 => {
                Err(Error::Api {
                    code: 404,
                    message: "Attachment file not found".to_string(),
                })
            }
            _ => {
                Err(Error::Api {
                    code: response.status().as_u16(),
                    message: response.text().await.unwrap_or_default(),
                })
            }
        }
    }

    pub async fn download_file(&self, download_url: &str) -> Result<Vec<u8>> {
        let response = self.client
            .get(download_url)
            .send()
            .await?;

        // Handle rate limiting
        self.handle_rate_limiting(&response).await?;

        if !response.status().is_success() {
            return Err(Error::Api {
                code: response.status().as_u16(),
                message: "Failed to download file".to_string(),
            });
        }

        let file_data = response.bytes().await?;
        Ok(file_data.to_vec())
    }

    pub async fn get_upload_authorization(
        &self,
        group_id: i64,
        item_key: &str,
        filename: &str,
        filesize: usize,
        md5: &Option<String>,
        mtime: Option<i64>,
    ) -> Result<UploadAuthorization> {
        let url = self.base_url.join(&format!("groups/{}/items/{}/file", group_id, item_key))?;
        
        let mut upload_data = serde_json::json!({
            "filename": filename,
            "filesize": filesize,
        });

        if let Some(ref md5_hash) = md5 {
            upload_data["md5"] = serde_json::Value::String(md5_hash.clone());
        }

        if let Some(mtime_val) = mtime {
            upload_data["mtime"] = serde_json::Value::Number(serde_json::Number::from(mtime_val));
        }

        let response = self.client
            .post(url)
            .json(&upload_data)
            .send()
            .await?;

        // Handle rate limiting
        self.handle_rate_limiting(&response).await?;

        match response.status().as_u16() {
            200 => {
                // File already exists, no upload needed
                Ok(UploadAuthorization {
                    exists: true,
                    upload_url: None,
                    upload_key: None,
                    params: None,
                })
            }
            201 => {
                // Upload required
                let auth: UploadAuthorizationResponse = response.json().await?;
                Ok(UploadAuthorization {
                    exists: false,
                    upload_url: Some(auth.url),
                    upload_key: Some(auth.upload_key),
                    params: Some(auth.params),
                })
            }
            412 => {
                Err(Error::Api {
                    code: 412,
                    message: "File upload precondition failed".to_string(),
                })
            }
            413 => {
                Err(Error::Api {
                    code: 413,
                    message: "File too large".to_string(),
                })
            }
            _ => {
                Err(Error::Api {
                    code: response.status().as_u16(),
                    message: response.text().await.unwrap_or_default(),
                })
            }
        }
    }

    pub async fn upload_file_to_url(
        &self,
        upload_url: &str,
        file_data: &[u8],
        params: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        // Create multipart form
        let mut form = reqwest::multipart::Form::new();
        
        // Add all the required parameters
        for (key, value) in params {
            form = form.text(key.clone(), value.clone());
        }
        
        // Add the file data
        let file_part = reqwest::multipart::Part::bytes(file_data.to_vec())
            .file_name("file");
        form = form.part("file", file_part);

        let response = self.client
            .post(upload_url)
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(Error::Api {
                code: response.status().as_u16(),
                message: format!("File upload failed: {}", response.text().await.unwrap_or_default()),
            });
        }

        Ok(())
    }

    pub async fn register_upload_completion(
        &self,
        group_id: i64,
        item_key: &str,
        upload_key: &str,
    ) -> Result<()> {
        let url = self.base_url.join(&format!("groups/{}/items/{}/file", group_id, item_key))?;
        
        let completion_data = serde_json::json!({
            "upload": upload_key
        });

        let response = self.client
            .post(url)
            .json(&completion_data)
            .send()
            .await?;

        // Handle rate limiting
        self.handle_rate_limiting(&response).await?;

        if !response.status().is_success() {
            return Err(Error::Api {
                code: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }

        Ok(())
    }
} 