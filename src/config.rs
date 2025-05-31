use serde::Deserialize;
use std::fs;
use std::path::Path;
use crate::{Result};

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    #[serde(rename = "ServerType")]
    pub server_type: String,
    #[serde(rename = "DSN")]
    pub dsn: String,
    #[serde(rename = "connection_max")]
    pub conn_max: Option<i32>,
    #[serde(rename = "Schema")]
    pub schema: String,
}

#[derive(Debug, Deserialize)]
pub struct GitlabConfig {
    pub token: String,
    pub project: String,
    pub url: String,
    pub active: bool,
}

#[derive(Debug, Deserialize)]
pub struct S3Config {
    pub endpoint: String,
    #[serde(rename = "accessKeyId")]
    pub access_key_id: String,
    #[serde(rename = "secretAccessKey")]
    pub secret_access_key: String,
    #[serde(rename = "useSSL")]
    pub use_ssl: bool,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(rename = "Service")]
    pub service: Option<String>,
    #[serde(rename = "Synconly")]
    pub synconly: Option<Vec<i64>>,
    #[serde(rename = "ClearBeforeSync")]
    pub clear_before_sync: Option<Vec<i64>>,
    #[serde(rename = "Endpoint")]
    pub endpoint: String,
    #[serde(rename = "Apikey")]
    pub apikey: String,
    #[serde(rename = "Logfile")]
    pub logfile: Option<String>,
    #[serde(rename = "Loglevel")]
    pub loglevel: Option<String>,
    #[serde(rename = "AccessLog")]
    pub access_log: Option<String>,
    #[serde(rename = "newgroupactive")]
    pub new_group_active: Option<bool>,
    #[serde(rename = "database")]
    pub db: DatabaseConfig,
    #[serde(rename = "groupcacheexpiration")]
    pub group_cache_expiration: Option<String>,
    #[serde(rename = "gitlab")]
    pub gitlab: Option<GitlabConfig>,
    #[serde(rename = "s3")]
    pub s3: S3Config,
}

impl Config {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn synconly(&self) -> Vec<i64> {
        self.synconly.clone().unwrap_or_default()
    }

    pub fn clear_before_sync(&self) -> Vec<i64> {
        self.clear_before_sync.clone().unwrap_or_default()
    }

    pub fn new_group_active(&self) -> bool {
        self.new_group_active.unwrap_or(false)
    }

    pub fn loglevel(&self) -> &str {
        self.loglevel.as_deref().unwrap_or("info")
    }
} 