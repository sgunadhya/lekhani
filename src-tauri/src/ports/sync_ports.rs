use crate::domain::{ConflictKind, SyncActionKind, SyncTargetKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCandidatePayload {
    pub target_kind: SyncTargetKind,
    pub action_kind: SyncActionKind,
    pub payload_json: String,
    pub evidence_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateResolutionPolicy {
    pub auto_apply_confidence_threshold: f32,
    pub editor_requires_confirmation: bool,
    pub document_patch_requires_confirmation: bool,
    pub conflict_kinds: Vec<ConflictKind>,
}
