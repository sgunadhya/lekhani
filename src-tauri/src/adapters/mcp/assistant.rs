use crate::adapters::tauri::state::AppState;
use crate::domain::{AppError, TaskStatus};
use crate::ports::WorkingMemoryRepository;

use super::{McpToolCall, McpToolResult};

pub fn execute(state: &AppState, call: McpToolCall) -> Result<McpToolResult, String> {
    match call {
        McpToolCall::GetWorkingMemory => state.get_working_memory().map(McpToolResult::WorkingMemory),
        McpToolCall::AddStoryTask { task } => {
            let mut memory = state.get_working_memory()?;
            memory.story_backlog.push(task);
            state
                .sqlite_repository
                .as_ref()
                .ok_or_else(|| "sqlite repository not available".to_string())?
                .save_working_memory(memory)
                .map(|_| McpToolResult::Empty)
                .map_err(|err: AppError| err.to_string())
        }
        McpToolCall::ResolveStoryTask { task_id } => {
            let mut memory = state.get_working_memory()?;
            if let Some(task) = memory
                .story_backlog
                .iter_mut()
                .find(|task| task.id == task_id)
            {
                task.status = TaskStatus::Resolved;
            }
            state
                .sqlite_repository
                .as_ref()
                .ok_or_else(|| "sqlite repository not available".to_string())?
                .save_working_memory(memory)
                .map(|_| McpToolResult::Empty)
                .map_err(|err: AppError| err.to_string())
        }
        _ => Err("unsupported assistant MCP tool call".to_string()),
    }
}
