use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Configuration error: {0}")]
    Config(#[from] toml::de::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("S3 error: {0}")]
    S3(#[from] aws_sdk_s3::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Sync error: {0}")]
    Sync(String),

    #[error("Unique constraint violation: {constraint}")]
    UniqueViolation { constraint: String },

    #[error("Empty result")]
    EmptyResult,

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("API error: {code} - {message}")]
    Api { code: u16, message: String },

    #[error("Rate limit exceeded, retry after: {retry_after:?}")]
    RateLimit { retry_after: Option<u64> },
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn is_unique_violation(&self, constraint: &str) -> bool {
        match self {
            Error::UniqueViolation { constraint: c } => c == constraint,
            _ => false,
        }
    }

    pub fn is_empty_result(&self) -> bool {
        matches!(self, Error::EmptyResult)
    }

    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::NotFound(_))
    }
}

impl Error {
    pub fn from_sqlx_error(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::RowNotFound => Error::EmptyResult,
            sqlx::Error::Database(db_err) if db_err.code() == Some(std::borrow::Cow::Borrowed("23505")) => {
                Error::UniqueViolation {
                    constraint: db_err.constraint().unwrap_or("unknown").to_string(),
                }
            }
            _ => Error::Database(err),
        }
    }
} 