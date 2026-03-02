use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

use super::StubNarrativeEngine;
use crate::application::{
    DialogueAct, DialogueActClassifier, DialogueActContext, InterpretationTarget,
    TurnInterpretation, TurnRoute,
};
use crate::domain::ConversationTopic;
use crate::domain::{
    AssistantIntent, NarrativeCharacter, NarrativeEvent, NarrativeSnapshot, WorkingMemory,
};
use crate::ports::{
    AssistantAgent, AssistantResponse, CharacterParser, EventParser, FollowUpDirective,
    NarrativeProvider,
};

#[derive(Clone)]
pub struct OpenAiCompatibleNarrativeEngine {
    client: Client,
    base_url: String,
    api_key: String,
    model: String,
    provider_label: String,
}

impl OpenAiCompatibleNarrativeEngine {
    pub fn new_from_env() -> Result<Self, String> {
        let provider = std::env::var("LEKHANI_LLM_PROVIDER").unwrap_or_default();
        if !matches!(
            provider.as_str(),
            "openai" | "deepseek" | "openai-compatible"
        ) {
            return Err("openai-compatible provider not selected".to_string());
        }

        let timeout_seconds = std::env::var("LEKHANI_OPENAI_TIMEOUT_SECS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(30);
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|err| err.to_string())?;

        let base_url = std::env::var("LEKHANI_OPENAI_BASE_URL")
            .unwrap_or_else(|_| default_base_url(&provider).to_string())
            .trim_end_matches('/')
            .to_string();
        let api_key = resolve_api_key(&provider)?;
        let model = std::env::var("LEKHANI_OPENAI_MODEL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "LEKHANI_OPENAI_MODEL is required for the selected cloud provider".to_string())?;

        Ok(Self {
            client,
            base_url,
            api_key,
            model,
            provider_label: provider,
        })
    }

    pub fn provider_label(&self) -> &str {
        &self.provider_label
    }

    fn options() -> Value {
        json!({
            "temperature": 0.0,
            "max_tokens": 400
        })
    }

    fn respond_json(&self, label: &str, system: &str, user: &str) -> Result<Value, String> {
        trace_block(
            label,
            "request",
            &format!("SYSTEM:\n{}\n\nUSER:\n{}", system, user),
        );
        let payload = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": format!("{}\nReturn only a single valid JSON object and no surrounding commentary.", system) },
                { "role": "user", "content": user }
            ],
            "temperature": Self::options()["temperature"],
            "max_tokens": Self::options()["max_tokens"]
        });

        let content = self.request_completion_content(&payload)?;
        trace_block(label, "response_raw", &content);
        let parsed = parse_single_json_object(&content)?;
        trace_block(label, "response_json", &parsed.to_string());
        Ok(parsed)
    }

    fn request_completion_content(&self, payload: &Value) -> Result<String, String> {
        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(&self.api_key)
            .json(payload)
            .send()
            .map_err(|err| err.to_string())?;
        let status = response.status();
        let body = response.text().map_err(|err| err.to_string())?;
        if !status.is_success() {
            let trimmed = body.trim();
            let detail = if trimmed.is_empty() { "<empty body>" } else { trimmed };
            return Err(format!(
                "HTTP {} from {}: {}",
                status, self.provider_label, detail
            ));
        }
        let completion: ChatCompletionsResponse = serde_json::from_str(&body).map_err(|err| {
            format!(
                "Invalid {} response JSON: {err}. Body: {}",
                self.provider_label,
                body.trim()
            )
        })?;
        completion
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| format!("{} returned no completion content", self.provider_label))
    }
}

impl DialogueActClassifier for OpenAiCompatibleNarrativeEngine {
    fn classify(&self, context: DialogueActContext<'_>) -> TurnInterpretation {
        let value = self.respond_json(
            "classify_dialogue_act",
            DIALOGUE_ACT_SYSTEM_PROMPT,
            &format!(
                "Prompt: {}\nCurrent mode: {:?}\nCurrent focus: {}",
                context.prompt,
                context.memory.conversation_mode,
                context
                    .memory
                    .current_focus
                    .as_ref()
                    .map(|f| f.summary.as_str())
                    .unwrap_or("None")
            ),
        );
        value.ok().map_or(default_interpretation(), |value| interpretation_from_value(&value))
    }
}

impl NarrativeProvider for OpenAiCompatibleNarrativeEngine {
    fn classify_dialogue_act(&self, context: DialogueActContext<'_>) -> TurnInterpretation {
        <Self as DialogueActClassifier>::classify(self, context)
    }
}

impl AssistantAgent for OpenAiCompatibleNarrativeEngine {
    fn interpret_followup(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<FollowUpDirective, String> {
        let value = self.respond_json(
            "interpret_followup",
            FOLLOWUP_INTERPRETER_SYSTEM_PROMPT,
            &format!(
                "Topic: {:?}\nMode: {:?}\nCurrent focus: {}\nUser: {}",
                memory.conversation_topic,
                memory.conversation_mode,
                memory
                    .current_focus
                    .as_ref()
                    .map(|item| item.summary.as_str())
                    .unwrap_or("None"),
                prompt
            ),
        )?;
        Ok(match value
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
        })
    }

    fn elaborate_focus(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        let focus = memory
            .current_focus
            .as_ref()
            .map(|item| item.summary.as_str())
            .unwrap_or("the current idea");
        let reply = parse_focus_reply(
            self.respond_json(
                "elaborate_focus",
                REFINE_FOCUS_SYSTEM_PROMPT,
                &format!(
                    "Topic: {:?}\nCurrent focus: {}\nWriter follow-up: {}",
                    memory.conversation_topic, focus, prompt
                ),
            )?,
            focus,
        )?;
        Ok(final_reply("Idea", AssistantIntent::Guide, reply))
    }

    fn suggest_alternative(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
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
        let reply = parse_focus_reply(
            self.respond_json(
                "suggest_alternative",
                ALTERNATIVE_OPTION_SYSTEM_PROMPT,
                &format!(
                    "Topic: {:?}\nCurrent option: {}\nRejected constraints: {}\nWriter follow-up: {}",
                    memory.conversation_topic, focus, rejected, prompt
                ),
            )?,
            "alternative idea",
        )?;
        Ok(final_reply("Idea", AssistantIntent::Guide, reply))
    }

    fn brainstorm_topic(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
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
        let reply = parse_focus_reply(
            self.respond_json(
                "brainstorm_topic",
                BRAINSTORM_TOPIC_SYSTEM_PROMPT,
                &format!(
                    "Topic: {:?}\nCurrent focus: {}\nRejected constraints: {}\nWriter: {}",
                    memory.conversation_topic,
                    memory
                        .current_focus
                        .as_ref()
                        .map(|f| f.summary.as_str())
                        .unwrap_or("None"),
                    rejected,
                    prompt
                ),
            )?,
            "story idea",
        )?;
        Ok(final_reply("Idea", AssistantIntent::Guide, reply))
    }

    fn respond_in_context(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        let reply = parse_focus_reply(
            self.respond_json(
                "respond_in_context",
                RESPOND_IN_CONTEXT_SYSTEM_PROMPT,
                &format!(
                    "Topic: {:?}\nMode: {:?}\nCurrent focus: {}\nWriter: {}",
                    memory.conversation_topic,
                    memory.conversation_mode,
                    memory
                        .current_focus
                        .as_ref()
                        .map(|f| f.summary.as_str())
                        .unwrap_or("None"),
                    prompt
                ),
            )?,
            prompt.trim(),
        )?;
        Ok(final_reply("Story Direction", AssistantIntent::Guide, reply))
    }

    fn draft_from_focus(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        let focus = memory
            .current_focus
            .as_ref()
            .map(|item| item.summary.as_str())
            .unwrap_or("the current idea");
        let reply = parse_focus_reply(
            self.respond_json(
                "draft_from_focus",
                SCREENPLAY_DRAFT_SYSTEM_PROMPT,
                &format!(
                    "Current focus: {}\nTopic: {:?}\nWriter request: {}",
                    focus, memory.conversation_topic, prompt
                ),
            )?,
            focus,
        )?;
        Ok(final_reply(
            "Screenplay Draft",
            AssistantIntent::MutateDocument,
            reply,
        ))
    }
}

impl CharacterParser for OpenAiCompatibleNarrativeEngine {
    fn parse_character(
        &self,
        description: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeCharacter, String> {
        let value = self
            .respond_json(
                "parse_character",
                CHARACTER_INSTRUCTIONS,
                &build_character_prompt(description, snapshot),
            )
            .or_else(|_| {
                StubNarrativeEngine
                    .parse_character(description, snapshot)
                    .map(character_to_value)
            })?;

        Ok(NarrativeCharacter {
            id: Uuid::new_v4(),
            ontology_entity_id: None,
            name: value["name"]
                .as_str()
                .unwrap_or("Unnamed Character")
                .trim()
                .to_string(),
            summary: value["summary"]
                .as_str()
                .unwrap_or(description)
                .trim()
                .to_string(),
            tags: value["tags"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
        })
    }
}

impl EventParser for OpenAiCompatibleNarrativeEngine {
    fn parse_event(
        &self,
        description: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeEvent, String> {
        let value = self
            .respond_json(
                "parse_event",
                EVENT_INSTRUCTIONS,
                &build_event_prompt(description, snapshot),
            )
            .or_else(|_| {
                StubNarrativeEngine
                    .parse_event(description, snapshot)
                    .map(event_to_value)
            })?;

        Ok(NarrativeEvent {
            id: Uuid::new_v4(),
            ontology_entity_id: None,
            title: value["title"]
                .as_str()
                .unwrap_or("Untitled Event")
                .trim()
                .to_string(),
            summary: value["summary"]
                .as_str()
                .unwrap_or(description)
                .trim()
                .to_string(),
            participants: resolve_participants_from_value(&value, snapshot),
        })
    }
}

#[derive(Deserialize)]
struct ChatCompletionsResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct FocusReply {
    focus_summary: String,
    body: String,
}

fn resolve_api_key(provider: &str) -> Result<String, String> {
    std::env::var("LEKHANI_OPENAI_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            if provider == "openai" {
                std::env::var("OPENAI_API_KEY").ok()
            } else {
                None
            }
        })
        .or_else(|| {
            if provider == "deepseek" {
                std::env::var("DEEPSEEK_API_KEY").ok()
            } else {
                None
            }
        })
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("API key is required for cloud provider `{provider}`"))
}

fn default_base_url(provider: &str) -> &'static str {
    match provider {
        "deepseek" => "https://api.deepseek.com/v1",
        _ => "https://api.openai.com/v1",
    }
}

fn parse_single_json_object(text: &str) -> Result<Value, String> {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        return Ok(value);
    }
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return serde_json::from_str(&text[start..=end])
                .map_err(|err| format!("Cloud provider returned invalid JSON: {err}"));
        }
    }
    Err("Cloud provider did not return a JSON object".to_string())
}

fn parse_focus_reply(value: Value, fallback_focus: &str) -> Result<FocusReply, String> {
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
        .ok_or_else(|| "Cloud provider reply is missing body".to_string())?
        .to_string();
    Ok(FocusReply { focus_summary, body })
}

fn default_interpretation() -> TurnInterpretation {
    TurnInterpretation {
        dialogue_act: DialogueAct::Brainstorm,
        target: InterpretationTarget::General,
        route: TurnRoute::Continue,
        confidence: 0.0,
    }
}

fn interpretation_from_value(value: &Value) -> TurnInterpretation {
    let dialogue_act = match value.get("dialogue_act").and_then(|v| v.as_str()) {
        Some("Query") => DialogueAct::Query,
        Some("Constraint") => DialogueAct::Constraint,
        Some("Correction") => DialogueAct::Correction,
        Some("Confirmation") => DialogueAct::Confirmation,
        Some("Commit") => DialogueAct::Commit,
        Some("RewriteRequest") => DialogueAct::RewriteRequest,
        _ => DialogueAct::Brainstorm,
    };
    let target = match value.get("target").and_then(|v| v.as_str()) {
        Some("current_candidate") => InterpretationTarget::CurrentCandidate,
        Some("screenplay") => InterpretationTarget::Screenplay,
        Some("setting") => InterpretationTarget::NewTopic(ConversationTopic::Setting),
        Some("character") => InterpretationTarget::NewTopic(ConversationTopic::Character),
        Some("event") => InterpretationTarget::NewTopic(ConversationTopic::Event),
        Some("relationship") => InterpretationTarget::NewTopic(ConversationTopic::Relationship),
        _ => InterpretationTarget::General,
    };
    let confidence = value
        .get("confidence")
        .and_then(|v| v.as_f64())
        .map(|v| v.clamp(0.0, 1.0) as f32)
        .unwrap_or(0.5);
    let route = match value.get("route").and_then(|v| v.as_str()) {
        Some("ElaborateCurrent") => TurnRoute::ElaborateCurrent,
        Some("AlternativeCurrent") => TurnRoute::AlternativeCurrent,
        Some("ConfirmCurrent") => TurnRoute::ConfirmCurrent,
        Some("RejectCurrent") => TurnRoute::RejectCurrent,
        Some("ShiftToCharacter") => TurnRoute::ShiftToCharacter,
        Some("ShiftToEvent") => TurnRoute::ShiftToEvent,
        Some("AddToScreenplay") => TurnRoute::AddToScreenplay,
        _ => TurnRoute::Continue,
    };
    TurnInterpretation {
        dialogue_act,
        target,
        route,
        confidence,
    }
}

fn final_reply(title: &str, _intent: AssistantIntent, reply: FocusReply) -> AssistantResponse {
    AssistantResponse::FinalReply {
        title: title.to_string(),
        focus_summary: Some(reply.focus_summary),
        body: clean_conversational_text(&reply.body),
    }
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

fn character_to_value(character: NarrativeCharacter) -> Value {
    json!({
        "name": character.name,
        "summary": character.summary,
        "tags": character.tags
    })
}

fn event_to_value(event: NarrativeEvent) -> Value {
    json!({
        "title": event.title,
        "summary": event.summary,
        "participant_names": event.participants
    })
}

fn resolve_participants_from_value(value: &Value, snapshot: &NarrativeSnapshot) -> Vec<Uuid> {
    let names = value
        .get("participant_names")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    names
        .into_iter()
        .filter_map(|name| {
            snapshot
                .characters
                .iter()
                .find(|character| character.name.eq_ignore_ascii_case(&name))
                .map(|character| character.id)
        })
        .collect()
}

fn build_character_prompt(description: &str, snapshot: &NarrativeSnapshot) -> String {
    let relevant_names = relevant_character_names(description, snapshot);
    format!(
        "Description: {}\nRelevant characters in the current context: {}\nReturn exactly one JSON object with keys name, summary, tags.",
        description,
        relevant_names.join(", ")
    )
}

fn build_event_prompt(description: &str, snapshot: &NarrativeSnapshot) -> String {
    let relevant_names = relevant_character_names(description, snapshot);
    format!(
        "Description: {}\nRelevant characters in the current context: {}\nReturn exactly one JSON object with keys title, summary, participant_names.",
        description,
        relevant_names.join(", ")
    )
}

fn relevant_character_names(description: &str, snapshot: &NarrativeSnapshot) -> Vec<String> {
    let lower_description = description.to_ascii_lowercase();
    let mut names = snapshot
        .characters
        .iter()
        .filter(|character| {
            let lower_name = character.name.to_ascii_lowercase();
            lower_description.contains(&lower_name)
                || character
                    .name
                    .split_whitespace()
                    .any(|part| lower_description.contains(&part.to_ascii_lowercase()))
        })
        .map(|character| character.name.clone())
        .collect::<Vec<_>>();

    if names.is_empty() {
        names = snapshot
            .characters
            .iter()
            .rev()
            .take(3)
            .map(|character| character.name.clone())
            .collect::<Vec<_>>();
        names.reverse();
    }

    if names.is_empty() {
        vec!["None".to_string()]
    } else {
        names
    }
}

fn trace_enabled() -> bool {
    matches!(
        std::env::var("LEKHANI_LLM_TRACE").ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("YES")
    )
}

fn trace_block(label: &str, stage: &str, body: &str) {
    if trace_enabled() {
        eprintln!("[cloud:{}:{}]\n{}\n", label, stage, body);
    }
}

const DIALOGUE_ACT_SYSTEM_PROMPT: &str = r#"Classify the writer's turn for a story-building assistant.
Return exactly one JSON object: {"dialogue_act":"Query|Brainstorm|Constraint|Correction|Confirmation|Commit|RewriteRequest","target":"current_candidate|setting|character|event|relationship|screenplay|general","route":"Continue|ElaborateCurrent|AlternativeCurrent|ConfirmCurrent|RejectCurrent|ShiftToCharacter|ShiftToEvent|AddToScreenplay","confidence":0.0}.
Choose target based on what the writer is acting on.
Choose route based on the most likely next conversational move relative to the current thread or candidate.
Confidence should be between 0 and 1."#;

const FOLLOWUP_INTERPRETER_SYSTEM_PROMPT: &str = r#"You classify follow-up intent for an active story conversation.
Return exactly one JSON object: {"directive":"ElaborateCurrent|AlternativeOption|ConfirmCurrent|RejectCurrent|ShiftToCharacter|ShiftToEvent|AddToScreenplay|Unknown"}."#;

const REFINE_FOCUS_SYSTEM_PROMPT: &str = r#"You are expanding an already chosen story idea.
Stay within the same topic and the same current idea.
Return exactly one JSON object with keys focus_summary and body."#;

const ALTERNATIVE_OPTION_SYSTEM_PROMPT: &str = r#"You are suggesting a different story option in the same topic.
Return exactly one JSON object with keys focus_summary and body."#;

const BRAINSTORM_TOPIC_SYSTEM_PROMPT: &str = r#"You are helping a writer brainstorm one concrete story option inside a topic.
Return exactly one JSON object with keys focus_summary and body."#;

const RESPOND_IN_CONTEXT_SYSTEM_PROMPT: &str = r#"You are helping a writer shape a story direction from facts they have already provided.
Preserve the writer's explicit anchors.
Do not introduce any new named person, place, culture, religion, profession, or time period that the writer did not mention.
If the writer gives a sparse setup, restate the anchors and ask one grounded next-step question instead of inventing more story facts.
Return exactly one JSON object with keys focus_summary and body."#;

const SCREENPLAY_DRAFT_SYSTEM_PROMPT: &str = r#"You turn a chosen story idea into screenplay-ready draft text.
Return exactly one JSON object with keys focus_summary and body."#;

const CHARACTER_INSTRUCTIONS: &str = r#"Extract a screenplay character.
Return exactly one JSON object with keys name, summary, tags."#;

const EVENT_INSTRUCTIONS: &str = r#"Extract a screenplay event.
Return exactly one JSON object with keys title, summary, participant_names."#;
