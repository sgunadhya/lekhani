use crate::domain::{AppError, ProvenanceRecord, SyncCandidate, SyncConflict, SyncRun};

pub trait SyncRunRepository: Send + Sync {
    fn create_run(&self, run: SyncRun) -> Result<SyncRun, AppError>;
    fn update_run(&self, run: SyncRun) -> Result<SyncRun, AppError>;
    fn get_run(&self, run_id: &str) -> Result<Option<SyncRun>, AppError>;
}

pub trait CandidateRepository: Send + Sync {
    fn create_candidate(&self, candidate: SyncCandidate) -> Result<SyncCandidate, AppError>;
    fn update_candidate(&self, candidate: SyncCandidate) -> Result<SyncCandidate, AppError>;
    fn list_pending_candidates(&self) -> Result<Vec<SyncCandidate>, AppError>;
}

pub trait ConflictRepository: Send + Sync {
    fn create_conflict(&self, conflict: SyncConflict) -> Result<SyncConflict, AppError>;
    fn list_open_conflicts(&self) -> Result<Vec<SyncConflict>, AppError>;
}

pub trait ProvenanceRepository: Send + Sync {
    fn create_record(
        &self,
        record: ProvenanceRecord,
    ) -> Result<ProvenanceRecord, AppError>;
    fn list_for_run(&self, run_id: &str) -> Result<Vec<ProvenanceRecord>, AppError>;
}
