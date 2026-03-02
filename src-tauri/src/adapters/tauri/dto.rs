use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{
    AssistantCapability, AssistantIntent, BeatId, ChangeType, NarrativeCharacter,
    NarrativeEvent, NarrativeMessagePreview, NarrativeMode, NarrativeSnapshot,
    NarrativeSuggestedAction, NarrativeSuggestionAction, ProvenanceRecord, SyncCandidate,
    SyncRun, ThreadStatus, WorkingMemory, WritePolicy,
};
use crate::application::{InterpretationTarget, TurnRoute};

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
pub struct NarrativeSuggestionActionRequest {
    pub action: NarrativeSuggestionAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeTurnDto {
    pub reply_title: String,
    pub reply_body: String,
    pub committed: NarrativeMessagePreview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantTurnDto {
    pub intent: AssistantIntent,
    pub capabilities: Vec<AssistantCapability>,
    pub write_policy: WritePolicy,
    pub interpretation_target: InterpretationTarget,
    pub interpretation_route: TurnRoute,
    pub interpretation_confidence: f32,
    pub reply_title: String,
    pub reply_body: String,
    pub narrative_mode: NarrativeMode,
    pub thread_status: ThreadStatus,
    pub active_beat: Option<BeatId>,
    pub evaluation_nudge: Option<String>,
    pub committed: NarrativeMessagePreview,
    pub working_memory: WorkingMemory,
    pub suggested_actions: Vec<NarrativeSuggestedAction>,
}

pub type WorkingMemoryDto = WorkingMemory;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDebugDto {
    pub runs: Vec<SyncRun>,
    pub pending_candidates: Vec<SyncCandidate>,
    pub recent_provenance: Vec<ProvenanceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStatusDto {
    pub backend: String,
    pub detail: String,
}

pub type NarrativeCharacterDto = NarrativeCharacter;
pub type NarrativeEventDto = NarrativeEvent;
pub type PreviewNarrativeInputDto = NarrativeMessagePreview;
pub type NarrativeSnapshotDto = NarrativeSnapshot;
