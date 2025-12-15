//! Sync queue management for event-driven outgoing sync.
//!
//! This module provides functionality to manage the sync_queue table which
//! stores pending sync operations created by PostgreSQL triggers when items
//! or collections are modified.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use crate::{Result, Error};
use super::LibraryType;

/// Represents a single entry in the sync queue
#[derive(Debug, Clone)]
pub struct SyncQueueEntry {
    pub id: i64,
    pub entity_type: String,
    pub entity_key: String,
    pub library_id: i64,
    pub library_type: LibraryType,
    pub operation: String,
    pub priority: i32,
    pub retry_count: i32,
    pub max_retries: i32,
    pub next_retry_at: DateTime<Utc>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

/// Manages the sync queue for event-driven sync operations
pub struct SyncQueue {
    db: PgPool,
    schema: String,
}

impl SyncQueue {
    /// Create a new SyncQueue instance
    pub fn new(db: PgPool, schema: String) -> Self {
        Self { db, schema }
    }

    /// Fetch pending entries for a specific library
    ///
    /// Returns entries that:
    /// - Have not been processed (processed_at IS NULL)
    /// - Have not exceeded max retries
    /// - Are ready for retry (next_retry_at <= NOW())
    ///
    /// Uses `FOR UPDATE SKIP LOCKED` to allow concurrent workers
    pub async fn fetch_pending(
        &self,
        library_id: i64,
        library_type: LibraryType,
        batch_size: i32,
    ) -> Result<Vec<SyncQueueEntry>> {
        let query = format!(
            r#"
            SELECT
                id, entity_type, entity_key, library_id, library_type,
                operation, priority, retry_count, max_retries,
                next_retry_at, last_error, created_at, processed_at
            FROM {}.sync_queue
            WHERE library_id = $1
              AND library_type = $2
              AND processed_at IS NULL
              AND retry_count < max_retries
              AND next_retry_at <= NOW()
            ORDER BY priority DESC, created_at ASC
            LIMIT $3
            FOR UPDATE SKIP LOCKED
            "#,
            self.schema
        );

        let rows = sqlx::query_as::<_, (
            i64, String, String, i64, LibraryType,
            String, i32, i32, i32,
            DateTime<Utc>, Option<String>, DateTime<Utc>, Option<DateTime<Utc>>
        )>(&query)
            .bind(library_id)
            .bind(library_type)
            .bind(batch_size)
            .fetch_all(&self.db)
            .await
            .map_err(Error::from_sqlx_error)?;

        Ok(rows.into_iter().map(|row| SyncQueueEntry {
            id: row.0,
            entity_type: row.1,
            entity_key: row.2,
            library_id: row.3,
            library_type: row.4,
            operation: row.5,
            priority: row.6,
            retry_count: row.7,
            max_retries: row.8,
            next_retry_at: row.9,
            last_error: row.10,
            created_at: row.11,
            processed_at: row.12,
        }).collect())
    }

    /// Mark an entry as successfully processed
    pub async fn mark_completed(&self, entry_id: i64) -> Result<()> {
        let query = format!(
            "UPDATE {}.sync_queue SET processed_at = NOW() WHERE id = $1",
            self.schema
        );

        sqlx::query(&query)
            .bind(entry_id)
            .execute(&self.db)
            .await
            .map_err(Error::from_sqlx_error)?;

        Ok(())
    }

    /// Mark an entry as failed with exponential backoff
    ///
    /// The next_retry_at is calculated as: NOW() + (2^retry_count) minutes
    /// This provides exponential backoff: 1min, 2min, 4min, 8min, 16min, etc.
    pub async fn mark_failed(&self, entry_id: i64, error: &str) -> Result<()> {
        let query = format!(
            r#"
            UPDATE {}.sync_queue
            SET retry_count = retry_count + 1,
                last_error = $2,
                next_retry_at = NOW() + (INTERVAL '1 minute' * POWER(2, retry_count))
            WHERE id = $1
            "#,
            self.schema
        );

        sqlx::query(&query)
            .bind(entry_id)
            .bind(error)
            .execute(&self.db)
            .await
            .map_err(Error::from_sqlx_error)?;

        Ok(())
    }

    /// Get all libraries that have pending sync entries
    ///
    /// Only returns libraries where outgoing_sync = 'event_driven'
    pub async fn get_libraries_with_pending(&self) -> Result<Vec<(i64, LibraryType)>> {
        let query = format!(
            r#"
            SELECT DISTINCT sq.library_id, sq.library_type
            FROM {schema}.sync_queue sq
            JOIN {schema}.sync_libraries sl ON sq.library_id = sl.library_id
                AND sq.library_type = sl.library_type
            WHERE sq.processed_at IS NULL
              AND sq.retry_count < sq.max_retries
              AND sq.next_retry_at <= NOW()
              AND sl.outgoing_sync = 'event_driven'
            "#,
            schema = self.schema
        );

        let results = sqlx::query_as::<_, (i64, LibraryType)>(&query)
            .fetch_all(&self.db)
            .await
            .map_err(Error::from_sqlx_error)?;

        Ok(results)
    }

    /// Cleanup old processed entries
    ///
    /// Deletes entries that were processed more than `days_to_keep` days ago
    /// Returns the number of entries deleted
    pub async fn cleanup_old_entries(&self, days_to_keep: i32) -> Result<u64> {
        let query = format!(
            r#"
            DELETE FROM {}.sync_queue
            WHERE processed_at IS NOT NULL
              AND processed_at < NOW() - INTERVAL '1 day' * $1
            "#,
            self.schema
        );

        let result = sqlx::query(&query)
            .bind(days_to_keep)
            .execute(&self.db)
            .await
            .map_err(Error::from_sqlx_error)?;

        Ok(result.rows_affected())
    }

    /// Get queue statistics for monitoring
    pub async fn get_stats(&self) -> Result<QueueStats> {
        let query = format!(
            r#"
            SELECT
                COUNT(*) FILTER (WHERE processed_at IS NULL AND retry_count < max_retries) as pending,
                COUNT(*) FILTER (WHERE processed_at IS NOT NULL) as processed,
                COUNT(*) FILTER (WHERE retry_count >= max_retries AND processed_at IS NULL) as failed
            FROM {}.sync_queue
            "#,
            self.schema
        );

        let row = sqlx::query_as::<_, (i64, i64, i64)>(&query)
            .fetch_one(&self.db)
            .await
            .map_err(Error::from_sqlx_error)?;

        Ok(QueueStats {
            pending: row.0,
            processed: row.1,
            failed: row.2,
        })
    }

    /// Delete a specific entry from the queue (used for permanent deletions)
    pub async fn delete_entry(&self, entry_id: i64) -> Result<()> {
        let query = format!(
            "DELETE FROM {}.sync_queue WHERE id = $1",
            self.schema
        );

        sqlx::query(&query)
            .bind(entry_id)
            .execute(&self.db)
            .await
            .map_err(Error::from_sqlx_error)?;

        Ok(())
    }
}

/// Statistics about the sync queue
#[derive(Debug, Clone)]
pub struct QueueStats {
    pub pending: i64,
    pub processed: i64,
    pub failed: i64,
}
