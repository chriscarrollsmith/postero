use serde::{Deserialize, Serialize};
use super::TagData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub data: TagData,
} 