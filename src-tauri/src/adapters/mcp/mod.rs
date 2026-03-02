mod narrative;
mod ontology;
mod sync;

use uuid::Uuid;

use crate::adapters::tauri::state::AppState;
use crate::domain::{OntologyEntity, OntologyRelationship};
pub use narrative::{apply_suggestion_action, preview_turn, submit_turn};
pub use sync::{get_debug_state, SyncDebugState};

#[derive(Debug)]
pub enum McpToolResult {
    Empty,
}

pub fn execute_tool(state: &AppState, call: McpToolCall) -> Result<McpToolResult, String> {
    match call {
        McpToolCall::ProposeOntologyCommit { .. } => ontology::execute(state, call),
    }
}

#[derive(Debug, Clone)]
pub enum McpToolCall {
    ProposeOntologyCommit {
        sync_run_id: Option<Uuid>,
        title: String,
        summary: String,
        entity: Option<OntologyEntity>,
        relationship: Option<OntologyRelationship>,
    },
}
