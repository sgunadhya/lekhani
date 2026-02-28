use crate::adapters::tauri::state::AppState;

use super::{McpToolCall, McpToolResult};

pub fn execute(state: &AppState, call: McpToolCall) -> Result<McpToolResult, String> {
    match call {
        McpToolCall::GetNarrativeSnapshot => state.get_snapshot().map(McpToolResult::Snapshot),
        McpToolCall::UpsertCharacter { character } => state
            .narrative_repository
            .save_character(character)
            .map(|_| McpToolResult::Empty)
            .map_err(|err| err.to_string()),
        McpToolCall::UpsertEvent { event } => state
            .narrative_repository
            .save_event(event)
            .map(|_| McpToolResult::Empty)
            .map_err(|err| err.to_string()),
        McpToolCall::UpsertRelationship { relationship } => state
            .narrative_repository
            .save_relationship(relationship)
            .map(|_| McpToolResult::Empty)
            .map_err(|err| err.to_string()),
        _ => Err("unsupported ontology MCP tool call".to_string()),
    }
}
