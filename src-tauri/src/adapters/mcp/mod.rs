mod document;
mod narrative;
mod ontology;
mod sync;

use uuid::Uuid;

use crate::domain::{
    NarrativeCharacter, NarrativeEvent, NarrativeSnapshot, OntologyRelationship, Screenplay,
    SyncActionKind, SyncRun, SyncTargetKind,
};

pub use narrative::{preview_turn, submit_turn, NarrativeTurnOutcome};
pub use sync::{get_debug_state, SyncDebugState};

#[derive(Clone)]
pub enum McpToolCall {
    GetNarrativeSnapshot,
    GetActiveScreenplay,
    UpsertCharacter {
        character: NarrativeCharacter,
    },
    UpsertEvent {
        event: NarrativeEvent,
    },
    UpsertRelationship {
        relationship: OntologyRelationship,
    },
    StartNarrativeSyncRun {
        prompt: String,
        document_version: u64,
    },
    RecordAppliedOntologyCandidate {
        sync_run_id: Uuid,
        prompt: String,
        target_kind: SyncTargetKind,
        action_kind: SyncActionKind,
        title: String,
        summary: String,
        payload_json: String,
        derived_kind: String,
        derived_ref: Option<String>,
    },
    FinishSyncRun {
        sync_run: SyncRun,
    },
}

pub enum McpToolResult {
    Snapshot(NarrativeSnapshot),
    Screenplay(Screenplay),
    SyncRun(SyncRun),
    Empty,
}
