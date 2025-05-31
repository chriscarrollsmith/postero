use serde::{Deserialize, Serialize};
use super::UserData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub data: UserData,
} 