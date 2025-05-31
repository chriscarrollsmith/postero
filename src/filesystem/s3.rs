use async_trait::async_trait;
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::{Client, primitives::ByteStream};
use crate::{Error, Result};
use super::{FileSystem, FilePutOptions, FileGetOptions, FileStatOptions, FolderCreateOptions, FileInfo};

#[derive(Debug)]
pub struct S3FileSystem {
    client: Client,
    endpoint: String,
    use_ssl: bool,
}

impl S3FileSystem {
    pub async fn new(endpoint: &str, access_key_id: &str, secret_access_key: &str, use_ssl: bool) -> Result<Self> {
        let region_provider = RegionProviderChain::default_provider().or_else("us-east-1");
        
        let mut config_builder = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(region_provider)
            .credentials_provider(aws_sdk_s3::config::Credentials::new(
                access_key_id,
                secret_access_key,
                None,
                None,
                "static",
            ));

        if !endpoint.is_empty() {
            config_builder = config_builder.endpoint_url(endpoint);
        }

        let config = config_builder.load().await;
        let client = Client::new(&config);

        Ok(Self {
            client,
            endpoint: endpoint.to_string(),
            use_ssl,
        })
    }
}

#[async_trait]
impl FileSystem for S3FileSystem {
    async fn folder_exists(&self, folder: &str) -> Result<bool> {
        match self.client.head_bucket().bucket(folder).send().await {
            Ok(_) => Ok(true),
            Err(err) => {
                if let Some(service_err) = err.as_service_error() {
                    if service_err.is_not_found() {
                        return Ok(false);
                    }
                }
                Err(Error::S3(err.into()))
            }
        }
    }

    async fn folder_create(&self, folder: &str, _opts: FolderCreateOptions) -> Result<()> {
        self.client
            .create_bucket()
            .bucket(folder)
            .send()
            .await
            .map_err(|e| Error::S3(e.into()))?;
        Ok(())
    }

    async fn file_exists(&self, folder: &str, name: &str) -> Result<bool> {
        match self.client
            .head_object()
            .bucket(folder)
            .key(name)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(err) => {
                if let Some(service_err) = err.as_service_error() {
                    if service_err.is_not_found() {
                        return Ok(false);
                    }
                }
                Err(Error::S3(err.into()))
            }
        }
    }

    async fn file_get(&self, folder: &str, name: &str, opts: FileGetOptions) -> Result<Vec<u8>> {
        let mut request = self.client.get_object().bucket(folder).key(name);
        
        if let Some(version_id) = opts.version_id {
            request = request.version_id(version_id);
        }

        let response = request.send().await.map_err(|e| Error::S3(e.into()))?;
        let data = response.body.collect().await.map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        Ok(data.into_bytes().to_vec())
    }

    async fn file_put(&self, folder: &str, name: &str, data: &[u8], opts: FilePutOptions) -> Result<()> {
        let mut request = self.client
            .put_object()
            .bucket(folder)
            .key(name)
            .body(ByteStream::from(data.to_vec()));

        if let Some(content_type) = opts.content_type {
            request = request.content_type(content_type);
        }

        request.send().await.map_err(|e| Error::S3(e.into()))?;
        Ok(())
    }

    async fn file_write_bytes(&self, folder: &str, name: &str, data: Vec<u8>, opts: FilePutOptions) -> Result<()> {
        self.file_put(folder, name, &data, opts).await
    }

    async fn file_read_bytes(&self, folder: &str, name: &str, opts: FileGetOptions) -> Result<Vec<u8>> {
        self.file_get(folder, name, opts).await
    }

    async fn file_stat(&self, folder: &str, name: &str, _opts: FileStatOptions) -> Result<FileInfo> {
        let response = self.client
            .head_object()
            .bucket(folder)
            .key(name)
            .send()
            .await
            .map_err(|e| Error::S3(e.into()))?;

        let size = response.content_length().unwrap_or(0) as u64;
        let modified = response.last_modified()
            .map(|dt| chrono::DateTime::from_timestamp(dt.secs(), dt.subsec_nanos()).unwrap_or_default())
            .unwrap_or_default();

        Ok(FileInfo {
            name: name.to_string(),
            size,
            modified,
            is_dir: false,
        })
    }

    fn protocol(&self) -> &str {
        if self.use_ssl {
            "https"
        } else {
            "http"
        }
    }
}

impl std::fmt::Display for S3FileSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "S3FileSystem(endpoint: {}, ssl: {})", self.endpoint, self.use_ssl)
    }
} 