use chrono::Utc;
use uuid::Uuid;

use crate::adapters::db::SqliteScreenplayRepository;
use crate::adapters::tauri::state::AppState;
use crate::domain::{
    CandidateStatus, ProvenanceRecord, SyncCandidate, SyncRun, SyncRunStatus, SyncSourceKind,
    SyncTargetLayer,
};
use crate::ports::{CandidateRepository, ProvenanceRepository, SyncRunRepository};

use super::{McpToolCall, McpToolResult};

#[derive(Debug, Clone)]
pub struct SyncDebugState {
    pub runs: Vec<SyncRun>,
    pub pending_candidates: Vec<SyncCandidate>,
    pub recent_provenance: Vec<ProvenanceRecord>,
}

pub fn execute(state: &AppState, call: McpToolCall) -> Result<McpToolResult, String> {
    match call {
        McpToolCall::StartNarrativeSyncRun {
            prompt,
            document_version,
        } => {
            let run = SyncRun {
                id: Uuid::new_v4(),
                source_kind: SyncSourceKind::NarrativeChat,
                source_ref: Some(prompt),
                document_version: Some(document_version),
                ontology_version: None,
                status: SyncRunStatus::Running,
                created_at: Utc::now(),
                completed_at: None,
            };

            match state.sqlite_repository.as_ref() {
                Some(repository) => repository
                    .create_run(run)
                    .map(McpToolResult::SyncRun)
                    .map_err(|err| err.to_string()),
                None => Ok(McpToolResult::SyncRun(run)),
            }
        }
        McpToolCall::RecordAppliedOntologyCandidate {
            sync_run_id,
            prompt,
            target_kind,
            action_kind,
            title,
            summary,
            payload_json,
            derived_kind,
            derived_ref,
        } => {
            let Some(repository) = state.sqlite_repository.as_ref() else {
                return Ok(McpToolResult::Empty);
            };

            let candidate = repository
                .create_candidate(SyncCandidate {
                    id: Uuid::new_v4(),
                    sync_run_id,
                    source_kind: SyncSourceKind::NarrativeChat,
                    source_ref: Some(prompt.clone()),
                    target_layer: SyncTargetLayer::Ontology,
                    target_kind,
                    action_kind,
                    status: CandidateStatus::Applied,
                    confidence: Some(1.0),
                    title,
                    summary: summary.clone(),
                    payload_json,
                    evidence_json: Some(serde_json::json!({ "prompt": prompt }).to_string()),
                    created_at: Utc::now(),
                    resolved_at: Some(Utc::now()),
                })
                .map_err(|err| err.to_string())?;

            if let Some(derived_ref) = derived_ref {
                repository
                    .create_record(ProvenanceRecord {
                        id: Uuid::new_v4(),
                        sync_run_id,
                        source_kind: SyncSourceKind::NarrativeChat,
                        source_ref: Some(prompt),
                        derived_kind,
                        derived_ref,
                        confidence: candidate.confidence,
                        notes: Some(summary),
                        created_at: Utc::now(),
                    })
                    .map_err(|err| err.to_string())?;
            }

            Ok(McpToolResult::Empty)
        }
        McpToolCall::FinishSyncRun { mut sync_run } => {
            let Some(repository) = state.sqlite_repository.as_ref() else {
                return Ok(McpToolResult::Empty);
            };

            sync_run.status = SyncRunStatus::Completed;
            sync_run.completed_at = Some(Utc::now());
            repository
                .update_run(sync_run)
                .map(|_| McpToolResult::Empty)
                .map_err(|err| err.to_string())
        }
        _ => Err("unsupported sync MCP tool call".to_string()),
    }
}

pub fn get_debug_state(state: &AppState) -> Result<SyncDebugState, String> {
    let Some(repository) = state.sqlite_repository.as_ref() else {
        return Ok(SyncDebugState {
            runs: Vec::new(),
            pending_candidates: Vec::new(),
            recent_provenance: Vec::new(),
        });
    };

    Ok(SyncDebugState {
        runs: repository
            .list_recent_sync_runs(10)
            .map_err(|err| err.to_string())?,
        pending_candidates: repository
            .list_pending_candidates()
            .map_err(|err| err.to_string())?,
        recent_provenance: repository
            .list_recent_provenance(25)
            .map_err(|err| err.to_string())?,
    })
}

pub fn record_applied_candidate_payload<T: serde::Serialize>(
    payload: &T,
) -> Result<String, String> {
    serde_json::to_string(payload).map_err(|err| format!("failed to serialize sync payload: {err}"))
}

pub fn list_recent_sync_runs(
    repository: &SqliteScreenplayRepository,
    limit: usize,
) -> Result<Vec<SyncRun>, String> {
    repository
        .list_recent_sync_runs(limit)
        .map_err(|err| err.to_string())
}
