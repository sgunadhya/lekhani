use crate::adapters::tauri::state::AppState;
use crate::domain::{AppError, ProvenanceRecord, SyncCandidate, SyncRun};
use crate::ports::CandidateRepository;

#[derive(Debug, Clone)]
pub struct SyncDebugState {
    pub runs: Vec<SyncRun>,
    pub pending_candidates: Vec<SyncCandidate>,
    pub recent_provenance: Vec<ProvenanceRecord>,
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
            .map_err(|err: AppError| err.to_string())?,
        pending_candidates: repository
            .list_pending_candidates()
            .map_err(|err: AppError| err.to_string())?,
        recent_provenance: repository
            .list_recent_provenance(25)
            .map_err(|err: AppError| err.to_string())?,
    })
}
