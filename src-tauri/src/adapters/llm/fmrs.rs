use std::sync::Arc;

use fm_rs::{GenerationOptions, Session, SystemLanguageModel};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use super::StubNarrativeEngine;
use crate::application::{DialogueAct, DialogueActClassifier, DialogueActContext};
use crate::domain::{
    AssistantIntent, NarrativeCharacter, NarrativeEvent, NarrativeNudge, NarrativeSnapshot,
    WorkingMemory,
};
use crate::ports::{
    AssistantAgent, AssistantResponse, CharacterParser, EventParser, FollowUpDirective,
    NudgeGenerator,
};

#[derive(Clone)]
pub struct FmRsNarrativeEngine {
    model: Arc<SystemLanguageModel>,
}

impl DialogueActClassifier for FmRsNarrativeEngine {
    fn classify(&self, context: DialogueActContext<'_>) -> DialogueAct {
        let session = match Session::with_instructions(&self.model, DIALOGUE_ACT_SYSTEM_PROMPT) {
            Ok(session) => session,
            Err(_) => return DialogueAct::Brainstorm,
        };
        let response = match session.respond(
            &format!(
                "Prompt: {}\nCurrent mode: {:?}\nCurrent focus: {}\nReturn exactly one JSON object: {{\"dialogue_act\":\"Query|Brainstorm|Constraint|Correction|Confirmation|Commit|RewriteRequest\"}}",
                context.prompt,
                context.memory.conversation_mode,
                context.memory.current_focus.as_ref().map(|f| f.summary.as_str()).unwrap_or("None")
            ),
            &Self::options(),
        ) {
            Ok(response) => response,
            Err(_) => return DialogueAct::Brainstorm,
        };
        let value = match parse_single_json_object(&format!("{}", response)) {
            Ok(value) => value,
            Err(_) => return DialogueAct::Brainstorm,
        };
        match value
            .get("dialogue_act")
            .and_then(|v| v.as_str())
            .unwrap_or("Brainstorm")
        {
            "Query" => DialogueAct::Query,
            "Constraint" => DialogueAct::Constraint,
            "Correction" => DialogueAct::Correction,
            "Confirmation" => DialogueAct::Confirmation,
            "Commit" => DialogueAct::Commit,
            "RewriteRequest" => DialogueAct::RewriteRequest,
            _ => DialogueAct::Brainstorm,
        }
    }
}

impl AssistantAgent for FmRsNarrativeEngine {
    fn interpret_followup(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<FollowUpDirective, String> {
        let session = Session::with_instructions(&self.model, FOLLOWUP_INTERPRETER_SYSTEM_PROMPT)
            .map_err(|err| err.to_string())?;
        let focus = memory
            .current_focus
            .as_ref()
            .map(|item| item.summary.as_str())
            .unwrap_or("None");
        let response = session
            .respond(
                &format!(
                    "Topic: {:?}\nMode: {:?}\nCurrent focus: {}\nUser: {}\nReturn exactly one JSON object: {{\"directive\":\"ElaborateCurrent|AlternativeOption|ConfirmCurrent|RejectCurrent|Unknown\"}}",
                    memory.conversation_topic,
                    memory.conversation_mode,
                    focus,
                    prompt
                ),
                &Self::options(),
            )
            .map_err(|err| err.to_string())?;

        let value = parse_single_json_object(&format!("{}", response))?;
        let directive = match value
            .get("directive")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
        {
            "ElaborateCurrent" => FollowUpDirective::ElaborateCurrent,
            "AlternativeOption" => FollowUpDirective::AlternativeOption,
            "ConfirmCurrent" => FollowUpDirective::ConfirmCurrent,
            "RejectCurrent" => FollowUpDirective::RejectCurrent,
            "ShiftToCharacter" => FollowUpDirective::ShiftToCharacter,
            "ShiftToEvent" => FollowUpDirective::ShiftToEvent,
            "AddToScreenplay" => FollowUpDirective::AddToScreenplay,
            _ => FollowUpDirective::Unknown,
        };
        Ok(directive)
    }

    fn elaborate_focus(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        let session = Session::with_instructions(&self.model, REFINE_FOCUS_SYSTEM_PROMPT)
            .map_err(|err| err.to_string())?;
        let focus = memory
            .current_focus
            .as_ref()
            .map(|item| item.summary.as_str())
            .unwrap_or("the current idea");
        let response = session
            .respond(
                &format!(
                    "Topic: {:?}\nCurrent focus: {}\nWriter follow-up: {}\nReturn exactly one JSON object: {{\"focus_summary\":\"...\",\"body\":\"...\"}}. The focus_summary must restate the same current idea in a short phrase. The body must elaborate the same idea and must not replace it with a different option.",
                    memory.conversation_topic, focus, prompt
                ),
                &Self::options(),
            )
            .map_err(|err| err.to_string())?;
        let reply = parse_focus_reply(&format!("{}", response), focus)?;
        Ok(AssistantResponse::FinalReply {
            intent: AssistantIntent::Guide,
            title: "Idea".to_string(),
            focus_summary: Some(reply.focus_summary),
            body: clean_conversational_text(&reply.body),
        })
    }

    fn suggest_alternative(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        let session = Session::with_instructions(&self.model, ALTERNATIVE_OPTION_SYSTEM_PROMPT)
            .map_err(|err| err.to_string())?;
        let focus = memory
            .current_focus
            .as_ref()
            .map(|item| item.summary.as_str())
            .unwrap_or("the current idea");
        let rejected = memory
            .constraints
            .iter()
            .filter(|constraint| {
                matches!(constraint.status, crate::domain::ConstraintStatus::Active)
                    && !matches!(constraint.operator, crate::domain::ConstraintOperator::Correct)
            })
            .map(|constraint| constraint.value.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let response = session
            .respond(
                &format!(
                    "Topic: {:?}\nCurrent option: {}\nRejected constraints: {}\nWriter follow-up: {}\nReturn exactly one JSON object: {{\"focus_summary\":\"...\",\"body\":\"...\"}}. Suggest one different option in the same topic. Do not repeat the current option or rejected ideas. The focus_summary must name only the new option.",
                    memory.conversation_topic, focus, rejected, prompt
                ),
                &Self::options(),
            )
            .map_err(|err| err.to_string())?;
        let reply = parse_focus_reply(&format!("{}", response), "alternative idea")?;
        Ok(AssistantResponse::FinalReply {
            intent: AssistantIntent::Guide,
            title: "Idea".to_string(),
            focus_summary: Some(reply.focus_summary),
            body: clean_conversational_text(&reply.body),
        })
    }

    fn brainstorm_topic(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        let session = Session::with_instructions(&self.model, BRAINSTORM_TOPIC_SYSTEM_PROMPT)
            .map_err(|err| err.to_string())?;
        let rejected = memory
            .constraints
            .iter()
            .filter(|constraint| {
                matches!(constraint.status, crate::domain::ConstraintStatus::Active)
                    && !matches!(constraint.operator, crate::domain::ConstraintOperator::Correct)
            })
            .map(|constraint| constraint.value.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let response = session
            .respond(
                &format!(
                    "Topic: {:?}\nCurrent focus: {}\nRejected constraints: {}\nWriter: {}\nReturn exactly one JSON object: {{\"focus_summary\":\"...\",\"body\":\"...\"}}. Suggest one concise option in this topic. If there is current focus, make the new option grow naturally out of it. Keep it concrete and do not commit anything. The focus_summary must describe only the setting, character, event, or relationship anchor for this topic. Do not include unrelated characters or plot unless the topic requires them.",
                    memory.conversation_topic,
                    memory.current_focus.as_ref().map(|f| f.summary.as_str()).unwrap_or("None"),
                    rejected,
                    prompt
                ),
                &Self::options(),
            )
            .map_err(|err| err.to_string())?;
        let reply = parse_focus_reply(&format!("{}", response), "story idea")?;
        Ok(AssistantResponse::FinalReply {
            intent: AssistantIntent::Guide,
            title: "Idea".to_string(),
            focus_summary: Some(reply.focus_summary),
            body: clean_conversational_text(&reply.body),
        })
    }

    fn draft_from_focus(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        let session = Session::with_instructions(&self.model, SCREENPLAY_DRAFT_SYSTEM_PROMPT)
            .map_err(|err| err.to_string())?;
        let focus = memory
            .current_focus
            .as_ref()
            .map(|item| item.summary.as_str())
            .unwrap_or("the current idea");
        let response = session
            .respond(
                &format!(
                    "Current focus: {}\nTopic: {:?}\nWriter request: {}\nReturn exactly one JSON object: {{\"focus_summary\":\"...\",\"body\":\"...\"}}. The focus_summary should restate the current focus. The body should be a concise screenplay-style prose beat or scene fragment that uses the current focus.",
                    focus, memory.conversation_topic, prompt
                ),
                &Self::options(),
            )
            .map_err(|err| err.to_string())?;
        let reply = parse_focus_reply(&format!("{}", response), focus)?;
        Ok(AssistantResponse::FinalReply {
            intent: AssistantIntent::MutateDocument,
            title: "Screenplay Draft".to_string(),
            focus_summary: Some(reply.focus_summary),
            body: clean_conversational_text(&reply.body),
        })
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

const DIALOGUE_ACT_SYSTEM_PROMPT: &str = r#"Classify the writer's turn for a story-building assistant.
Return exactly one JSON object: {"dialogue_act":"Query|Brainstorm|Constraint|Correction|Confirmation|Commit|RewriteRequest"}.
Use:
- Query: asks for more, asks a question, or asks to continue/refine
- Brainstorm: asks for an idea or option
- Constraint: rules out a direction
- Correction: corrects an earlier assumption
- Confirmation: accepts the current idea
- Commit: provides concrete story structure to record
- RewriteRequest: asks to turn an idea into screenplay text"#;

const FOLLOWUP_INTERPRETER_SYSTEM_PROMPT: &str = r#"You classify follow-up intent for an active story conversation.
Return exactly one JSON object: {"directive":"ElaborateCurrent|AlternativeOption|ConfirmCurrent|RejectCurrent|ShiftToCharacter|ShiftToEvent|AddToScreenplay|Unknown"}.
Choose:
- ElaborateCurrent: the writer wants more depth on the active idea
- AlternativeOption: the writer wants a different option in the same topic
- ConfirmCurrent: the writer accepts the active idea
- RejectCurrent: the writer rejects the active idea
- ShiftToCharacter: the writer wants to move from the current idea into character design
- ShiftToEvent: the writer wants to move from the current idea into an event or scene
- AddToScreenplay: the writer wants the current idea turned into screenplay text
- Unknown: unclear"#;

const REFINE_FOCUS_SYSTEM_PROMPT: &str = r#"You are expanding an already chosen story idea.
Stay within the same topic and the same current idea.
Do not switch to a different option.
Do not use conversational preambles, acknowledgements, or offers to help.
Start directly with the content.
Write 2-4 concise sentences in natural language."#;

const ALTERNATIVE_OPTION_SYSTEM_PROMPT: &str = r#"You are suggesting a different story option in the same topic.
Offer one new option that contrasts with the current one.
Do not repeat rejected ideas.
Do not use conversational preambles, acknowledgements, or offers to help.
Start directly with the option.
Write 2-4 concise sentences in natural language."#;

const BRAINSTORM_TOPIC_SYSTEM_PROMPT: &str = r#"You are helping a writer brainstorm one concrete story option inside a topic.
Suggest exactly one option.
Do not switch topics.
Do not commit structure.
Do not use conversational preambles, acknowledgements, numbered lists, or offers to help.
Start directly with the suggestion.
Write 2-4 concise sentences in natural language."#;

const SCREENPLAY_DRAFT_SYSTEM_PROMPT: &str = r#"You turn a chosen story idea into screenplay-ready draft text.
Stay faithful to the current focus.
Do not introduce a different direction.
Do not use conversational preambles or explanations.
Write 3-6 concise sentences."#;

const NUDGE_SYSTEM_PROMPT: &str = "You are a story coach. Based on the state, give a friendly 1-sentence nudge to keep the user writing.";

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

fn clean_conversational_text(text: &str) -> String {
    let mut paragraphs = Vec::new();

    for raw in text
        .split("\n\n")
        .flat_map(|block| block.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if paragraphs.last().is_some_and(|prev: &String| prev == raw) {
            continue;
        }
        paragraphs.push(raw.to_string());
    }

    paragraphs.join("\n\n").trim().to_string()
}

#[derive(Deserialize)]
struct FocusReply {
    focus_summary: String,
    body: String,
}

fn parse_focus_reply(text: &str, fallback_focus: &str) -> Result<FocusReply, String> {
    let value = parse_single_json_object(text)?;
    let focus_summary = value
        .get("focus_summary")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback_focus)
        .to_string();
    let body = value
        .get("body")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "assistant reply is missing body".to_string())?
        .to_string();

    Ok(FocusReply { focus_summary, body })
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
