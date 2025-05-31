use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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