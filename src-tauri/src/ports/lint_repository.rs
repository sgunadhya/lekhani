use crate::domain::{AppError, LintFinding};

pub trait LintRepository: Send + Sync {
    fn upsert_finding(&self, finding: LintFinding) -> Result<LintFinding, AppError>;
    fn list_open_findings(&self) -> Result<Vec<LintFinding>, AppError>;
}
