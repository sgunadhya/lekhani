use crate::domain::{AppError, SyncCandidate, SyncRun};

pub trait SyncRunRepository: Send + Sync {
    fn create_run(&self, run: SyncRun) -> Result<SyncRun, AppError>;
}

pub trait CandidateRepository: Send + Sync {
    fn create_candidate(&self, candidate: SyncCandidate) -> Result<SyncCandidate, AppError>;
    fn list_pending_candidates(&self) -> Result<Vec<SyncCandidate>, AppError>;
}
