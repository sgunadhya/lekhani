use crate::domain::{AppError, WorkingMemory};

pub trait WorkingMemoryRepository: Send + Sync {
    fn load_working_memory(
        &self,
        project_id: &str,
        session_id: &str,
    ) -> Result<WorkingMemory, AppError>;

    fn save_working_memory(&self, memory: WorkingMemory) -> Result<WorkingMemory, AppError>;
}
