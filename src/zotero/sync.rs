use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "library_type")]
#[sqlx(rename_all = "lowercase")]
pub enum LibraryType {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "group")]
    Group,
}

impl Default for LibraryType {
    fn default() -> Self {
        LibraryType::Group
    }
}

impl std::fmt::Display for LibraryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibraryType::User => write!(f, "user"),
            LibraryType::Group => write!(f, "group"),
        }
    }
}

impl std::str::FromStr for LibraryType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(LibraryType::User),
            "group" => Ok(LibraryType::Group),
            _ => Err(format!("Invalid library type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "syncdirection")]
#[sqlx(rename_all = "lowercase")]
pub enum SyncDirection {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "tocloud")]
    ToCloud,
    #[serde(rename = "tolocal")]
    ToLocal,
    #[serde(rename = "bothcloud")]
    BothCloud,
    #[serde(rename = "bothlocal")]
    BothLocal,
    #[serde(rename = "bothmanual")]
    BothManual,
}

impl Default for SyncDirection {
    fn default() -> Self {
        SyncDirection::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    #[serde(rename = "new")]
    New,
    #[serde(rename = "synced")]
    Synced,
    #[serde(rename = "modified")]
    Modified,
    #[serde(rename = "incomplete")]
    Incomplete,
}

impl Default for SyncStatus {
    fn default() -> Self {
        SyncStatus::New
    }
} 