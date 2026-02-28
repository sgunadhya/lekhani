use crate::api::dto::{
    LlmStatusDto, NarrativeCommitTargetDto, NarrativeNudgeDto, NarrativeSnapshotDto,
    PreviewNarrativeInputDto,
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

pub fn create_llm_status_resource() -> Resource<(), Result<LlmStatusDto, String>> {
    create_local_resource(|| (), |_| async move { tauri::get_llm_status().await })
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

pub fn create_commit_action() -> Action<String, Result<PreviewNarrativeInputDto, String>> {
    create_action(|prompt: &String| {
        let prompt = prompt.clone();
        async move { tauri::commit_narrative_input(prompt).await }
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
