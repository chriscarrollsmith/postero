use serde::Deserialize;
use std::fs;
use std::path::Path;
use crate::{Result};

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    #[serde(alias = "ServerType", alias = "servertype")]
    pub server_type: String,
    #[serde(alias = "DSN", alias = "dsn")]
    pub dsn: String,
    #[serde(alias = "connection_max")]
    pub conn_max: Option<i32>,
    #[serde(alias = "Schema", alias = "schema")]
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
    #[serde(alias = "accessKeyId")]
    pub access_key_id: String,
    #[serde(alias = "secretAccessKey")]
    pub secret_access_key: String,
    #[serde(alias = "useSSL", alias = "usessl")]
    pub use_ssl: bool,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(alias = "Service", alias = "service")]
    pub service: Option<String>,
    #[serde(alias = "Synconly", alias = "synconly")]
    pub synconly: Option<Vec<i64>>,
    #[serde(alias = "ClearBeforeSync", alias = "clear_before_sync")]
    pub clear_before_sync: Option<Vec<i64>>,
    #[serde(alias = "Endpoint", alias = "endpoint")]
    pub endpoint: String,
    #[serde(alias = "Apikey", alias = "apikey")]
    pub apikey: String,
    #[serde(alias = "Logfile", alias = "logfile")]
    pub logfile: Option<String>,
    #[serde(alias = "Loglevel", alias = "loglevel")]
    pub loglevel: Option<String>,
    #[serde(alias = "AccessLog", alias = "accesslog")]
    pub access_log: Option<String>,
    #[serde(alias = "newgroupactive")]
    pub new_group_active: Option<bool>,
    #[serde(alias = "database")]
    pub db: DatabaseConfig,
    #[serde(alias = "groupcacheexpiration")]
    pub group_cache_expiration: Option<String>,
    #[serde(alias = "gitlab")]
    pub gitlab: Option<GitlabConfig>,
    #[serde(alias = "s3")]
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