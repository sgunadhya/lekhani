use crate::api::dto::{
    AssistantIntentDto, AssistantTurnDto, LlmStatusDto, NarrativeCommitTargetDto, NarrativeNudgeDto,
    NarrativeSnapshotDto, PreviewNarrativeInputDto, SyncDebugDto, WorkingMemoryDto,
};
use crate::api::tauri;
use leptos::*;

#[derive(Clone, PartialEq)]
pub enum ChatRole {
    User,
    Assistant,
}

#[derive(Clone, PartialEq)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub title: Option<String>,
    pub body: String,
}

#[derive(Clone, Copy)]
pub struct NarrativeChatContext {
    pub prompt: RwSignal<String>,
    pub messages: RwSignal<Vec<ChatMessage>>,
    pub working_memory: RwSignal<Option<WorkingMemoryDto>>,
    pub last_intent: RwSignal<Option<AssistantIntentDto>>,
}

impl NarrativeChatContext {
    pub fn new() -> Self {
        Self {
            prompt: create_rw_signal(String::new()),
            messages: create_rw_signal(vec![ChatMessage {
                role: ChatRole::Assistant,
                title: Some("Lekhani".to_string()),
                body: "Tell me about the story, a character, or a scene problem you are working through. I will turn that into structured narrative changes and help you keep moving.".to_string(),
            }]),
            working_memory: create_rw_signal(None),
            last_intent: create_rw_signal(None),
        }
    }
}

pub fn create_nudge_resource(
    refresh: ReadSignal<u64>,
) -> Resource<u64, Result<NarrativeNudgeDto, String>> {
    create_local_resource(move || refresh.get(), |nonce| async move {
        _ = nonce;
        tauri::get_nudge().await
    })
}

pub fn create_llm_status_resource() -> Resource<(), Result<LlmStatusDto, String>> {
    create_local_resource(|| (), |_| async move { tauri::get_llm_status().await })
}

pub fn create_sync_debug_resource(
    refresh: ReadSignal<u64>,
) -> Resource<u64, Result<SyncDebugDto, String>> {
    create_local_resource(move || refresh.get(), |nonce| async move {
        _ = nonce;
        tauri::get_sync_debug().await
    })
}

pub fn create_working_memory_resource(
    refresh: ReadSignal<u64>,
) -> Resource<u64, Result<WorkingMemoryDto, String>> {
    create_local_resource(move || refresh.get(), |nonce| async move {
        _ = nonce;
        tauri::get_working_memory().await
    })
}

pub fn create_preview_resource(
    prompt: ReadSignal<String>,
) -> Resource<String, Result<PreviewNarrativeInputDto, String>> {
    create_local_resource(move || prompt.get(), |prompt| async move {
        if prompt.trim().is_empty() {
            Ok(PreviewNarrativeInputDto {
                prompt,
                suggested_target: NarrativeCommitTargetDto::Character,
                character: None,
                event: None,
                relationships: Vec::new(),
                changes: Vec::new(),
            })
        } else {
            tauri::preview_narrative_input(prompt).await
        }
    })
}

pub fn create_turn_action() -> Action<String, Result<AssistantTurnDto, String>> {
    create_action(|prompt: &String| {
        let prompt = prompt.clone();
        async move { tauri::submit_assistant_turn(prompt).await }
    })
}

pub fn create_snapshot_resource(
    refresh: ReadSignal<u64>,
) -> Resource<u64, Result<NarrativeSnapshotDto, String>> {
    create_local_resource(move || refresh.get(), |nonce| async move {
        _ = nonce;
        tauri::get_narrative_snapshot().await
    })
}
