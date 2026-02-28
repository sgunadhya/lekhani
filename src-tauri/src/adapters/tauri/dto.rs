use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    ChangeType, NarrativeCharacter, NarrativeCommitTarget, NarrativeEvent,
    NarrativeMessagePreview, NarrativeNudge, NarrativeSnapshot,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenplayChangeDto {
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
pub struct ScreenplayDto {
    pub id: Uuid,
    pub title: String,
    pub fountain_text: String,
    pub version: u64,
    pub changes: Vec<ScreenplayChangeDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveScreenplayRequest {
    pub screenplay: ScreenplayDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentFileDto {
    pub screenplay: ScreenplayDto,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveDocumentRequest {
    pub screenplay: ScreenplayDto,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseDescriptionRequest {
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitNarrativeInputRequest {
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStatusDto {
    pub backend: String,
    pub detail: String,
}

pub type NarrativeCharacterDto = NarrativeCharacter;
pub type NarrativeCommitTargetDto = NarrativeCommitTarget;
pub type NarrativeEventDto = NarrativeEvent;
pub type PreviewNarrativeInputDto = NarrativeMessagePreview;
pub type NarrativeNudgeDto = NarrativeNudge;
pub type NarrativeSnapshotDto = NarrativeSnapshot;
