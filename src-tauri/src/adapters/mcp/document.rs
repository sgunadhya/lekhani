use crate::adapters::tauri::state::AppState;
use crate::domain::AppError;

use super::{McpToolCall, McpToolResult};

pub fn execute(state: &AppState, call: McpToolCall) -> Result<McpToolResult, String> {
    match call {
        McpToolCall::GetActiveScreenplay => state
            .screenplay_service
            .get_active_screenplay()
            .map(McpToolResult::Screenplay)
            .map_err(|err: AppError| err.to_string()),
        _ => Err("unsupported document MCP tool call".to_string()),
    }
}
