use crate::api::dto::{
    AssistantIntentDto, AssistantTurnDto, BeatIdDto, InterpretationTargetDto, LlmStatusDto,
    NarrativeModeDto, NarrativeSnapshotDto, NarrativeSuggestedActionViewDto,
    NarrativeSuggestionActionDto, SyncDebugDto, ThreadStatusDto, TurnRouteDto, WorkingMemoryDto,
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
    pub last_mode: RwSignal<Option<NarrativeModeDto>>,
    pub last_thread_status: RwSignal<Option<ThreadStatusDto>>,
    pub last_interpretation_target: RwSignal<Option<InterpretationTargetDto>>,
    pub last_interpretation_route: RwSignal<Option<TurnRouteDto>>,
    pub last_interpretation_confidence: RwSignal<Option<f32>>,
    pub last_beat: RwSignal<Option<BeatIdDto>>,
    pub last_evaluation_nudge: RwSignal<Option<String>>,
    pub suggested_actions: RwSignal<Vec<NarrativeSuggestedActionViewDto>>,
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
            last_mode: create_rw_signal(None),
            last_thread_status: create_rw_signal(None),
            last_interpretation_target: create_rw_signal(None),
            last_interpretation_route: create_rw_signal(None),
            last_interpretation_confidence: create_rw_signal(None),
            last_beat: create_rw_signal(None),
            last_evaluation_nudge: create_rw_signal(None),
            suggested_actions: create_rw_signal(Vec::new()),
        }
    }
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

pub fn create_turn_action() -> Action<String, Result<AssistantTurnDto, String>> {
    create_action(|prompt: &String| {
        let prompt = prompt.clone();
        async move { tauri::submit_assistant_turn(prompt).await }
    })
}

pub fn create_suggestion_action() -> Action<NarrativeSuggestionActionDto, Result<AssistantTurnDto, String>> {
    create_action(|action: &NarrativeSuggestionActionDto| {
        let action = action.clone();
        async move { tauri::apply_narrative_suggestion(action).await }
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
