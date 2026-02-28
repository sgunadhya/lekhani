use crate::api::dto::{
    NarrativeCharacterDto, NarrativeEventDto, NarrativeNudgeDto, NarrativeSnapshotDto,
};
use crate::api::tauri;
use leptos::*;

pub fn create_nudge_resource(
    refresh: ReadSignal<u64>,
) -> Resource<u64, Result<NarrativeNudgeDto, String>> {
    create_local_resource(move || refresh.get(), |nonce| async move {
        _ = nonce;
        tauri::get_nudge().await
    })
}

pub fn create_character_action() -> Action<String, Result<NarrativeCharacterDto, String>> {
    create_action(|description: &String| {
        let description = description.clone();
        async move { tauri::parse_character(description).await }
    })
}

pub fn create_event_action() -> Action<String, Result<NarrativeEventDto, String>> {
    create_action(|description: &String| {
        let description = description.clone();
        async move { tauri::parse_event(description).await }
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
