use async_trait::async_trait;
use crate::{Result};

pub mod s3;

pub use s3::S3FileSystem;

#[derive(Debug, Default)]
pub struct FilePutOptions {
    pub content_type: Option<String>,
}

#[derive(Debug, Default)]
pub struct FileGetOptions {
    pub version_id: Option<String>,
}

#[derive(Debug, Default)]
pub struct FileStatOptions {}

#[derive(Debug, Default)]
pub struct FolderCreateOptions {
    pub object_locking: bool,
}

#[async_trait]
pub trait FileSystem: Send + Sync + std::fmt::Debug {
    async fn folder_exists(&self, folder: &str) -> Result<bool>;
    async fn folder_create(&self, folder: &str, opts: FolderCreateOptions) -> Result<()>;
    async fn file_exists(&self, folder: &str, name: &str) -> Result<bool>;
    async fn file_get(&self, folder: &str, name: &str, opts: FileGetOptions) -> Result<Vec<u8>>;
    async fn file_put(&self, folder: &str, name: &str, data: &[u8], opts: FilePutOptions) -> Result<()>;
    async fn file_write_bytes(&self, folder: &str, name: &str, data: Vec<u8>, opts: FilePutOptions) -> Result<()>;
    async fn file_read_bytes(&self, folder: &str, name: &str, opts: FileGetOptions) -> Result<Vec<u8>>;
    async fn file_stat(&self, folder: &str, name: &str, opts: FileStatOptions) -> Result<FileInfo>;
    fn protocol(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub modified: chrono::DateTime<chrono::Utc>,
    pub is_dir: bool,
}

impl std::fmt::Display for FileInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({} bytes, modified: {})", self.name, self.size, self.modified)
    }
} 