use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyncSourceKind {
    NarrativeChat,
    ScreenplayExtraction,
    OntologySuggestion,
    LintResolution,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyncRunStatus {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRun {
    pub id: Uuid,
    pub source_kind: SyncSourceKind,
    pub source_ref: Option<String>,
    pub document_version: Option<u64>,
    pub ontology_version: Option<u64>,
    pub status: SyncRunStatus,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyncTargetLayer {
    Document,
    Ontology,
    Link,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyncTargetKind {
    Character,
    Event,
    Relationship,
    Setting,
    WorldContext,
    ScreenplayPatch,
    DocumentMetadata,
    Link,
    LintFinding,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SyncActionKind {
    Create,
    Update,
    Merge,
    Relink,
    Patch,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CandidateStatus {
    Draft,
    Ready,
    Applied,
    Rejected,
    Superseded,
    Expired,
    Conflicted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCandidate {
    pub id: Uuid,
    pub sync_run_id: Uuid,
    pub source_kind: SyncSourceKind,
    pub source_ref: Option<String>,
    pub target_layer: SyncTargetLayer,
    pub target_kind: SyncTargetKind,
    pub action_kind: SyncActionKind,
    pub status: CandidateStatus,
    pub confidence: Option<f32>,
    pub title: String,
    pub summary: String,
    pub payload_json: String,
    pub evidence_json: Option<String>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceRecord {
    pub id: Uuid,
    pub sync_run_id: Uuid,
    pub source_kind: SyncSourceKind,
    pub source_ref: Option<String>,
    pub derived_kind: String,
    pub derived_ref: String,
    pub confidence: Option<f32>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}
