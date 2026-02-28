use std::sync::Arc;

use fm_rs::{GenerationOptions, Session, SystemLanguageModel};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use super::StubNarrativeEngine;
use crate::adapters::mcp::{McpToolCall, McpToolResult};
use crate::domain::{
    AssistantIntent, NarrativeCharacter, NarrativeEvent, NarrativeNudge, NarrativeSnapshot,
    OntologyEntity, OntologyRelationship, StoryTask, WorkingMemory,
};
use crate::ports::{
    AssistantAgent, AssistantResponse, AssistantToolCall, CharacterParser, EventParser,
    NudgeGenerator,
};

#[derive(Clone)]
pub struct FmRsNarrativeEngine {
    model: Arc<SystemLanguageModel>,
}

impl AssistantAgent for FmRsNarrativeEngine {
    fn process_turn(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
        observations: &[(McpToolCall, McpToolResult)],
    ) -> Result<AssistantResponse, String> {
        let session = Session::with_instructions(&self.model, AGENT_SYSTEM_PROMPT)
            .map_err(|err| err.to_string())?;

        let agent_prompt = build_agent_turn_prompt(prompt, memory, observations);
        
        let response = session
            .respond(&agent_prompt, &Self::options())
            .map_err(|err| err.to_string())?;

        // Parse the response to see if it's a tool call or a final reply
        parse_agent_response(&format!("{}", response))
    }

    fn generate_nudge(
        &self,
        snapshot: &NarrativeSnapshot,
        memory: &WorkingMemory,
    ) -> Result<NarrativeNudge, String> {
        let session = Session::with_instructions(&self.model, NUDGE_SYSTEM_PROMPT)
            .map_err(|err| err.to_string())?;

        let open_tasks: Vec<_> = memory.story_backlog.iter()
            .filter(|t| matches!(t.status, crate::domain::TaskStatus::Open))
            .map(|t| &t.description)
            .collect();

        let prompt = format!(
            "Open Goals: {:?}\nModel Status: {} characters, {} events.\nNudge:",
            open_tasks,
            snapshot.characters.len(),
            snapshot.events.len()
        );

        let response = session
            .respond(&prompt, &Self::options())
            .map_err(|err| err.to_string())?;

        Ok(NarrativeNudge {
            message: format!("{}", response).trim().to_string(),
        })
    }
}

const AGENT_SYSTEM_PROMPT: &str = r#"You are Lekhani, a Story Architect.
Help the writer build a structured screenplay model.

RULES:
1. The writer is the user. Never address them as a character.
2. Never commit canonical ontology changes directly. You may only propose changes via ontology.propose_commit.
3. Be concise.
4. Your entire response must be exactly one JSON object in one of these two forms:
   - {"tool_calls":[{"name":"ontology.get_snapshot","parameters":{},"thought":"short optional reason"}]}
   - {"final_reply":{"intent":"Guide","title":"Idea","body":"How about a desert setting?"}}
5. Do not return any other top-level keys. In particular, never return a bare {"thought": ...} object.
6. If you are unsure, return final_reply instead of a tool call.

Allowed tools: ontology.get_snapshot, assistant.get_working_memory, ontology.propose_commit, assistant.add_story_task."#;

const NUDGE_SYSTEM_PROMPT: &str = "You are a story coach. Based on the state, give a friendly 1-sentence nudge to keep the user writing.";

fn build_agent_turn_prompt(
    prompt: &str,
    memory: &WorkingMemory,
    observations: &[(McpToolCall, McpToolResult)],
) -> String {
    let mut parts = Vec::new();
    parts.push(format!("User: {}", prompt));
    
    // Just the count and summary, no full backlog
    parts.push(format!(
        "Status: {} open tasks. Focus: {}.",
        memory.story_backlog.iter().filter(|t| matches!(t.status, crate::domain::TaskStatus::Open)).count(),
        memory.current_focus.as_ref().map(|f| f.summary.as_str()).unwrap_or("None")
    ));

    if !observations.is_empty() {
        parts.push("Recent tool results:".to_string());
        // Only keep last 2 observations to save space
        for (call, result) in observations.iter().rev().take(2).rev() {
            let result_str = match result {
                McpToolResult::Snapshot(s) => format!("{} chars, {} events", s.characters.len(), s.events.len()),
                McpToolResult::Empty => "Success".to_string(),
                _ => "Data returned".to_string(),
            };
            parts.push(format!("{}: {}", call.name(), result_str));
        }
    }

    parts.push(
        "Respond with exactly one JSON object using either {\"tool_calls\":[...]} or {\"final_reply\":{...}}."
            .to_string(),
    );
    parts.join("\n")
}

fn parse_agent_response(text: &str) -> Result<AssistantResponse, String> {
    // Try to repair truncated JSON
    let mut repaired_text = text.trim().to_string();
    let mut in_string = false;
    let mut escaped = false;
    for c in repaired_text.chars() {
        if c == '\\' { escaped = !escaped; }
        else if c == '"' && !escaped { in_string = !in_string; escaped = false; }
        else { escaped = false; }
    }
    if in_string { repaired_text.push('"'); }
    let open_braces = repaired_text.chars().filter(|&c| c == '{').count();
    let close_braces = repaired_text.chars().filter(|&c| c == '}').count();
    if open_braces > close_braces {
        for _ in 0..(open_braces - close_braces) { repaired_text.push('}'); }
    }
    let text = &repaired_text;

    let json_val = parse_single_json_object(text)?;

    // 1. Final Reply detection
    if let Some(r) = json_val.get("final_reply") {
        let intent = match r.get("intent").and_then(|v| v.as_str()).unwrap_or("") {
            "MutateOntology" => AssistantIntent::MutateOntology,
            "Query" => AssistantIntent::Query,
            "Clarify" => AssistantIntent::Clarify,
            _ => AssistantIntent::Guide,
        };
        let title = r.get("title").and_then(|v| v.as_str()).unwrap_or("Architect").to_string();
        let body = match r.get("body") {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Object(obj)) => obj.iter().map(|(k, v)| format!("• {}: {}", k.replace('_', " "), v.as_str().unwrap_or(&v.to_string()))).collect::<Vec<_>>().join("\n"),
            _ => "".to_string(),
        };
        return Ok(AssistantResponse::FinalReply { intent, title, body });
    }

    // 2. Tool Call detection
    if let Some(calls) = json_val.get("tool_calls").and_then(|v| v.as_array()) {
        if calls.is_empty() {
            return Err("assistant returned an empty tool_calls array".to_string());
        }

        let mut tool_calls = Vec::with_capacity(calls.len());
        for call_val in calls {
            let name = call_val
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "assistant tool call is missing name".to_string())?;
            let params = call_val
                .get("parameters")
                .or_else(|| call_val.get("arguments"))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let thought = call_val
                .get("thought")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let call = map_tool_call(name, &params)?;
            tool_calls.push(AssistantToolCall { call, thought });
        }

        return Ok(AssistantResponse::ToolCalls(tool_calls));
    }

    Err("assistant returned JSON without a valid final_reply or tool_calls envelope".to_string())
}

fn map_tool_call(name: &str, params: &serde_json::Value) -> Result<McpToolCall, String> {
    match name {
        "ontology.get_snapshot" => Ok(McpToolCall::GetNarrativeSnapshot),
        "assistant.get_working_memory" => Ok(McpToolCall::GetWorkingMemory),
        "ontology.propose_commit" => {
            let title = params.get("title").and_then(|v| v.as_str()).unwrap_or("Proposal").to_string();
            let summary = params.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let entity = params.get("entity").and_then(|v| serde_json::from_value(v.clone()).ok());
            let relationship = params.get("relationship").and_then(|v| serde_json::from_value(v.clone()).ok());
            Ok(McpToolCall::ProposeOntologyCommit {
                sync_run_id: None,
                title,
                summary,
                entity,
                relationship,
            })
        },
        "assistant.add_story_task" => {
            let task_val = params.get("task").ok_or("Missing task")?;
            let task: StoryTask = serde_json::from_value(task_val.clone()).map_err(|e| e.to_string())?;
            Ok(McpToolCall::AddStoryTask { task })
        },
        _ => Err(format!("Unknown tool: {}", name)),
    }
}

fn parse_single_json_object(text: &str) -> Result<serde_json::Value, String> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        return Ok(value);
    }

    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return serde_json::from_str(&text[start..=end])
                .map_err(|err| format!("assistant returned invalid JSON: {err}"));
        }
    }

    Err("assistant did not return a JSON object".to_string())
}

impl FmRsNarrativeEngine {
    pub fn new() -> Result<Self, String> {
        let model = SystemLanguageModel::new().map_err(|err| err.to_string())?;
        model.ensure_available().map_err(|err| err.to_string())?;

        Ok(Self {
            model: Arc::new(model),
        })
    }

    fn options() -> GenerationOptions {
        GenerationOptions::builder()
            .temperature(0.2)
            .max_response_tokens(512)
            .build()
    }

    fn character_session(&self) -> Result<Session, String> {
        Session::with_instructions(&self.model, CHARACTER_INSTRUCTIONS).map_err(|err| err.to_string())
    }

    fn event_session(&self) -> Result<Session, String> {
        Session::with_instructions(&self.model, EVENT_INSTRUCTIONS).map_err(|err| err.to_string())
    }

    fn nudge_session(&self) -> Result<Session, String> {
        Session::with_instructions(&self.model, NUDGE_INSTRUCTIONS).map_err(|err| err.to_string())
    }
}

impl CharacterParser for FmRsNarrativeEngine {
    fn parse_character(
        &self,
        description: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeCharacter, String> {
        let payload = self
            .character_session()
            .and_then(|session| {
                session
                    .respond_structured(
                        &build_character_prompt(description, snapshot),
                        &character_schema(),
                        &Self::options(),
                    )
                    .map_err(|err| err.to_string())
            });

        let payload: CharacterHydration = match payload {
            Ok(payload) => payload,
            Err(_) => return StubNarrativeEngine.parse_character(description, snapshot),
        };

        Ok(NarrativeCharacter {
            id: Uuid::new_v4(),
            ontology_entity_id: None,
            name: payload.name.trim().to_string(),
            summary: payload.summary.trim().to_string(),
            tags: payload
                .tags
                .into_iter()
                .map(|tag| tag.trim().to_string())
                .filter(|tag| !tag.is_empty())
                .collect(),
        })
    }
}

impl EventParser for FmRsNarrativeEngine {
    fn parse_event(
        &self,
        description: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeEvent, String> {
        let payload = self
            .event_session()
            .and_then(|session| {
                session
                    .respond_structured(
                        &build_event_prompt(description, snapshot),
                        &event_schema(),
                        &Self::options(),
                    )
                    .map_err(|err| err.to_string())
            });

        let payload: EventHydration = match payload {
            Ok(payload) => payload,
            Err(_) => return StubNarrativeEngine.parse_event(description, snapshot),
        };

        Ok(NarrativeEvent {
            id: Uuid::new_v4(),
            ontology_entity_id: None,
            title: payload.title.trim().to_string(),
            summary: payload.summary.trim().to_string(),
            participants: resolve_participants(&payload.participant_names, snapshot),
        })
    }
}

impl NudgeGenerator for FmRsNarrativeEngine {
    fn generate_nudge(&self, snapshot: &NarrativeSnapshot) -> Result<NarrativeNudge, String> {
        let payload = self
            .nudge_session()
            .and_then(|session| {
                session
                    .respond_json(
                        &build_nudge_prompt(snapshot),
                        &nudge_schema(),
                        &Self::options(),
                    )
                    .map_err(|err| err.to_string())
            })
            .ok();
        let message = payload
            .as_deref()
            .and_then(parse_nudge_message)
            .or_else(|| heuristic_nudge(snapshot))
            .or_else(|| NudgeGenerator::generate_nudge(&StubNarrativeEngine, snapshot).ok().map(|nudge| nudge.message));

        Ok(NarrativeNudge {
            message: message.ok_or_else(|| {
                "Foundation Models returned a nudge payload without a usable message".to_string()
            })?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct CharacterHydration {
    name: String,
    summary: String,
    tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct EventHydration {
    title: String,
    summary: String,
    participant_names: Vec<String>,
}

const CHARACTER_INSTRUCTIONS: &str = r#"You are Lekhani's narrative hydration engine.
Extract one character object from the user's text.
Prefer concrete names. Reuse existing character naming if the text appears to refine an existing character.
Return only valid JSON matching the schema."#;

const EVENT_INSTRUCTIONS: &str = r#"You are Lekhani's narrative hydration engine.
Extract one event object from the user's text.
Use existing character names when the user refers to known characters.
Return participant_names as exact names from the known-character list whenever possible.
Return only valid JSON matching the schema."#;

const NUDGE_INSTRUCTIONS: &str = r#"You are Lekhani's narrative guide.
Given the current model snapshot, return exactly one concise next-step nudge for the user.
The nudge must target the most important missing narrative information.
Use a top-level field named message.
Return only valid JSON matching the schema."#;

fn character_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "summary": { "type": "string" },
            "tags": {
                "type": "array",
                "items": { "type": "string" }
            }
        },
        "required": ["name", "summary", "tags"]
    })
}

fn event_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "title": { "type": "string" },
            "summary": { "type": "string" },
            "participant_names": {
                "type": "array",
                "items": { "type": "string" }
            }
        },
        "required": ["title", "summary", "participant_names"]
    })
}

fn nudge_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "message": { "type": "string" }
        },
        "required": ["message"]
    })
}

fn build_character_prompt(description: &str, snapshot: &NarrativeSnapshot) -> String {
    format!(
        "Known characters:\n{}\n\nKnown events:\n{}\n\nUser input:\n{}\n\nExtract a single character object.",
        format_character_list(snapshot),
        format_event_list(snapshot),
        description.trim()
    )
}

fn build_event_prompt(description: &str, snapshot: &NarrativeSnapshot) -> String {
    format!(
        "Known characters:\n{}\n\nKnown events:\n{}\n\nUser input:\n{}\n\nExtract a single event object and link participants to known character names when possible.",
        format_character_list(snapshot),
        format_event_list(snapshot),
        description.trim()
    )
}

fn build_nudge_prompt(snapshot: &NarrativeSnapshot) -> String {
    format!(
        "Known characters:\n{}\n\nKnown events:\n{}\n\nMetrics:\nscene_count={} character_count={} event_count={}\n\nReturn the single best next-step nudge.",
        format_character_list(snapshot),
        format_event_list(snapshot),
        snapshot.metrics.scene_count,
        snapshot.metrics.character_count,
        snapshot.metrics.event_count,
    )
}

fn format_character_list(snapshot: &NarrativeSnapshot) -> String {
    if snapshot.characters.is_empty() {
        return "- none".to_string();
    }

    snapshot
        .characters
        .iter()
        .map(|character| format!("- {}: {}", character.name, character.summary))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_event_list(snapshot: &NarrativeSnapshot) -> String {
    if snapshot.events.is_empty() {
        return "- none".to_string();
    }

    snapshot
        .events
        .iter()
        .map(|event| format!("- {}: {}", event.title, event.summary))
        .collect::<Vec<_>>()
        .join("\n")
}

fn resolve_participants(names: &[String], snapshot: &NarrativeSnapshot) -> Vec<Uuid> {
    names
        .iter()
        .filter_map(|name| {
            let lowered = name.trim().to_lowercase();
            snapshot
                .characters
                .iter()
                .find(|character| character.name.to_lowercase() == lowered)
                .map(|character| character.id)
                .or_else(|| {
                    snapshot
                        .characters
                        .iter()
                        .find(|character| character.name.to_lowercase().contains(&lowered))
                        .map(|character| character.id)
                })
        })
        .collect()
}

fn parse_nudge_message(payload: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    extract_string_value(&value)
}

fn extract_string_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => {
            let text = text.trim().to_string();
            (!text.is_empty()).then_some(text)
        }
        serde_json::Value::Object(map) => {
            for key in ["message", "next_nudge", "nudge", "guidance", "reason", "text"] {
                if let Some(found) = map.get(key).and_then(extract_string_value) {
                    return Some(found);
                }
            }

            map.values().find_map(extract_string_value)
        }
        serde_json::Value::Array(items) => items.iter().find_map(extract_string_value),
        _ => None,
    }
}

fn heuristic_nudge(snapshot: &NarrativeSnapshot) -> Option<String> {
    if snapshot.characters.is_empty() {
        Some("Define the protagonist in one sentence with a clear desire and pressure.".to_string())
    } else if snapshot.events.is_empty() {
        Some("Describe the first event that forces the protagonist to act.".to_string())
    } else if snapshot.events.iter().all(|event| event.participants.is_empty()) {
        Some("Link at least one tracked character to a narrative event so the model can reason about scene participation.".to_string())
    } else if snapshot.characters.len() == 1 {
        Some("Add a counter-force or ally so the story model has a relationship to reason about.".to_string())
    } else {
        Some("Clarify the turning point that changes the protagonist's options in the next sequence.".to_string())
    }
}
