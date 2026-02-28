use chrono::Utc;
use uuid::Uuid;

use crate::adapters::tauri::state::AppState;
use crate::domain::{
    AppError, CandidateStatus, SyncActionKind, SyncCandidate, SyncSourceKind, SyncTargetKind,
    SyncTargetLayer,
};
use crate::ports::{CandidateRepository, SyncRunRepository};

use super::{McpToolCall, McpToolResult};

pub fn execute(state: &AppState, call: McpToolCall) -> Result<McpToolResult, String> {
    match call {
        McpToolCall::GetNarrativeSnapshot => state.get_snapshot().map(McpToolResult::Snapshot),
        McpToolCall::ProposeOntologyCommit {
            sync_run_id,
            title,
            summary,
            entity,
            relationship,
        } => {
            let Some(repository) = state.sqlite_repository.as_ref() else {
                return Err("Persistence unavailable for proposals".to_string());
            };

            let (target_kind, payload_json) = if let Some(ent) = entity {
                let kind = match ent.kind {
                    crate::domain::OntologyEntityKind::Character => SyncTargetKind::Character,
                    crate::domain::OntologyEntityKind::Event => SyncTargetKind::Event,
                    crate::domain::OntologyEntityKind::Setting => SyncTargetKind::Setting,
                    crate::domain::OntologyEntityKind::WorldContext => SyncTargetKind::WorldContext,
                };
                (kind, serde_json::to_string(&ent).map_err(|e| e.to_string())?)
            } else if let Some(rel) = relationship {
                (SyncTargetKind::Relationship, serde_json::to_string(&rel).map_err(|e| e.to_string())?)
            } else {
                (SyncTargetKind::Character, "{}".to_string())
            };

            let run_id = if let Some(run_id) = sync_run_id {
                run_id
            } else {
                repository
                    .create_run(crate::domain::SyncRun {
                        id: Uuid::new_v4(),
                        source_kind: SyncSourceKind::NarrativeChat,
                        source_ref: Some("Proposed via Narrative Agent".to_string()),
                        document_version: None,
                        ontology_version: None,
                        status: crate::domain::SyncRunStatus::Completed,
                        created_at: Utc::now(),
                        completed_at: Some(Utc::now()),
                    })
                    .map_err(|err: AppError| err.to_string())?
                    .id
            };

            repository
                .create_candidate(SyncCandidate {
                    id: Uuid::new_v4(),
                    sync_run_id: run_id,
                    source_kind: SyncSourceKind::NarrativeChat,
                    source_ref: None,
                    target_layer: SyncTargetLayer::Ontology,
                    target_kind,
                    action_kind: SyncActionKind::Create,
                    status: CandidateStatus::Draft,
                    confidence: Some(0.8),
                    title,
                    summary,
                    payload_json,
                    evidence_json: None,
                    created_at: Utc::now(),
                    resolved_at: None,
                })
                .map(|_| McpToolResult::Empty)
                .map_err(|err: AppError| err.to_string())
        }
        _ => Err("unsupported ontology tool call or direct mutation bypass attempted".to_string()),
    }
}
