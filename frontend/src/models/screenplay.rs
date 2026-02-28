use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenplayListItem {
    pub id: Uuid,
    pub title: String,
}
