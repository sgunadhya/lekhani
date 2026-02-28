use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screenplay {
    pub id: Uuid,
    pub title: String,
    pub fountain_text: String,
    #[serde(skip)]
    pub parsed: Option<()>,
    pub version: u64,
    pub changes: Vec<ScreenplayChange>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenplayChange {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub author: String,
    pub change_type: ChangeType,
    pub range_start: usize,
    pub range_end: usize,
    pub new_text: String,
    pub old_text: String,
    pub provenance: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Insert,
    Delete,
    Replace,
}
