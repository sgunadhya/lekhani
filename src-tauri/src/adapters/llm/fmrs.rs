use std::sync::Arc;

use fm_rs::{GenerationOptions, Session, SystemLanguageModel};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use super::StubNarrativeEngine;
use crate::domain::{NarrativeCharacter, NarrativeEvent, NarrativeNudge, NarrativeSnapshot};
use crate::ports::{CharacterParser, EventParser, NudgeGenerator};

#[derive(Clone)]
pub struct FmRsNarrativeEngine {
    model: Arc<SystemLanguageModel>,
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
            .max_response_tokens(400)
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
            .or_else(|| StubNarrativeEngine.generate_nudge(snapshot).ok().map(|nudge| nudge.message));

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
