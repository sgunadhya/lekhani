mod assistant;
mod document;
mod narrative;
mod ontology;
mod sync;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::adapters::tauri::state::AppState;
use crate::domain::{
    NarrativeCharacter, NarrativeEvent, NarrativeSnapshot, OntologyEntity, OntologyRelationship,
    Screenplay, StoryTask, SyncActionKind, SyncRun, SyncTargetKind, WorkingMemory,
};

pub use crate::application::NarrativeTurnOutcome;
pub use narrative::{apply_suggestion_action, preview_turn, submit_turn};
pub use sync::{get_debug_state, SyncDebugState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolSchema {
    pub name: String,
    pub description: String,
    pub parameters_json_schema: serde_json::Value,
}

pub fn get_tool_schemas() -> Vec<McpToolSchema> {
    vec![
        McpToolSchema {
            name: "ontology.get_snapshot".to_string(),
            description: "Get the current narrative model state (characters, events, relationships).".to_string(),
            parameters_json_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        McpToolSchema {
            name: "document.get_active_screenplay".to_string(),
            description: "Read the current active screenplay draft text.".to_string(),
            parameters_json_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        McpToolSchema {
            name: "assistant.get_working_memory".to_string(),
            description: "Retrieve the current focus and story backlog (tasks).".to_string(),
            parameters_json_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        McpToolSchema {
            name: "ontology.propose_commit".to_string(),
            description: "Propose an ontology change (entity or relationship) for review under the current assistant turn.".to_string(),
            parameters_json_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "summary": { "type": "string" },
                    "entity": { 
                        "type": "object",
                        "properties": {
                            "kind": { "type": "string", "enum": ["Character", "Event", "Setting", "WorldContext"] },
                            "label": { "type": "string" },
                            "summary": { "type": "string" }
                        },
                        "required": ["kind", "label", "summary"]
                    },
                    "relationship": { "type": "object" }
                },
                "required": ["title", "summary"]
            }),
        },
        McpToolSchema {
            name: "assistant.add_story_task".to_string(),
            description: "Add a new task to the story backlog (e.g. 'Define goal for Rajan').".to_string(),
            parameters_json_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "task": { "type": "object" }
                },
                "required": ["task"]
            }),
        },
    ]
}

#[derive(Debug, Serialize)]
pub enum McpToolResult {
    Snapshot(NarrativeSnapshot),
    Screenplay(Screenplay),
    SyncRun(SyncRun),
    WorkingMemory(WorkingMemory),
    Empty,
}

pub fn execute_tool(state: &AppState, call: McpToolCall) -> Result<McpToolResult, String> {
    match call {
        McpToolCall::GetNarrativeSnapshot | McpToolCall::ProposeOntologyCommit { .. } => {
            ontology::execute(state, call)
        }

        McpToolCall::GetActiveScreenplay => document::execute(state, call),

        McpToolCall::GetWorkingMemory
        | McpToolCall::AddStoryTask { .. }
        | McpToolCall::ResolveStoryTask { .. } => assistant::execute(state, call),

        McpToolCall::StartNarrativeSyncRun { .. }
        | McpToolCall::RecordAppliedOntologyCandidate { .. }
        | McpToolCall::FinishSyncRun { .. } => sync::execute(state, call),
    }
}

#[derive(Debug, Clone)]
pub enum McpToolCall {
    GetNarrativeSnapshot,
    GetActiveScreenplay,
    GetWorkingMemory,
    AddStoryTask {
        task: StoryTask,
    },
    ResolveStoryTask {
        task_id: String,
    },
    ProposeOntologyCommit {
        sync_run_id: Option<Uuid>,
        title: String,
        summary: String,
        entity: Option<OntologyEntity>,
        relationship: Option<OntologyRelationship>,
    },
    StartNarrativeSyncRun {
        prompt: String,
        document_version: u64,
    },
    RecordAppliedOntologyCandidate {
        sync_run_id: Uuid,
        prompt: String,
        target_kind: SyncTargetKind,
        action_kind: SyncActionKind,
        title: String,
        summary: String,
        payload_json: String,
        derived_kind: String,
        derived_ref: Option<String>,
    },
    FinishSyncRun {
        sync_run: SyncRun,
    },
}

impl McpToolCall {
    pub fn name(&self) -> &'static str {
        match self {
            McpToolCall::GetNarrativeSnapshot => "ontology.get_snapshot",
            McpToolCall::GetActiveScreenplay => "document.get_active_screenplay",
            McpToolCall::GetWorkingMemory => "assistant.get_working_memory",
            McpToolCall::AddStoryTask { .. } => "assistant.add_story_task",
            McpToolCall::ResolveStoryTask { .. } => "assistant.resolve_story_task",
            McpToolCall::ProposeOntologyCommit { .. } => "ontology.propose_commit",
            McpToolCall::StartNarrativeSyncRun { .. } => "sync.start_run",
            McpToolCall::RecordAppliedOntologyCandidate { .. } => "sync.record_candidate",
            McpToolCall::FinishSyncRun { .. } => "sync.finish_run",
        }
    }
}
