use crate::adapters::mcp::{apply_suggestion_action, get_debug_state, preview_turn, submit_turn, SyncDebugState};
use crate::adapters::tauri::dto::{
    AssistantTurnDto, CommitNarrativeInputRequest, DocumentFileDto, LlmStatusDto, NarrativeCharacterDto,
    NarrativeEventDto, NarrativeNudgeDto, NarrativeSnapshotDto, NarrativeSuggestionActionRequest,
    NarrativeTurnDto, ParseDescriptionRequest, PreviewNarrativeInputDto, SaveDocumentRequest,
    SyncDebugDto, SaveScreenplayRequest, ScreenplayDto, WorkingMemoryDto,
};
use crate::adapters::tauri::state::AppState;
use crate::domain::WorkingMemory;
use crate::ports::WorkingMemoryRepository;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn get_time() -> String {
    format!("Current time: {}", chrono::Local::now().to_rfc2822())
}

#[tauri::command]
pub fn get_llm_status(state: State<'_, AppState>) -> Result<LlmStatusDto, String> {
    Ok(LlmStatusDto {
        backend: state.llm_backend.clone(),
        detail: state.llm_detail.clone(),
    })
}

#[tauri::command]
pub fn get_sync_debug(state: State<'_, AppState>) -> Result<SyncDebugDto, String> {
    let SyncDebugState {
        runs,
        pending_candidates,
        recent_provenance,
    } = get_debug_state(&state)?;

    Ok(SyncDebugDto {
        runs,
        pending_candidates,
        recent_provenance,
    })
}

#[tauri::command]
pub fn get_working_memory(state: State<'_, AppState>) -> Result<WorkingMemoryDto, String> {
    let Some(repository) = state.sqlite_repository.as_ref() else {
        return Ok(WorkingMemory::default());
    };

    repository
        .load_working_memory("current-project", "main")
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_screenplays(state: State<'_, AppState>) -> Result<Vec<ScreenplayDto>, String> {
    state
        .screenplay_service
        .list_screenplays()
        .map(|screenplays| screenplays.into_iter().map(Into::into).collect())
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_active_screenplay(state: State<'_, AppState>) -> Result<ScreenplayDto, String> {
    state
        .screenplay_service
        .get_active_screenplay()
        .map(Into::into)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_current_project(state: State<'_, AppState>) -> Result<DocumentFileDto, String> {
    Ok(DocumentFileDto {
        screenplay: state
            .screenplay_service
            .get_active_screenplay()
            .map(Into::into)
            .map_err(|err| err.to_string())?,
        file_path: state.current_project_path()?,
    })
}

#[tauri::command]
pub fn save_screenplay(state: State<'_, AppState>, request: SaveScreenplayRequest) -> Result<ScreenplayDto, String> {
    state
        .screenplay_service
        .save_screenplay(request.screenplay.into())
        .map(Into::into)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn import_fountain_document(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Option<ScreenplayDto>, String> {
    let file_path = app
        .dialog()
        .file()
        .add_filter("Fountain", &["fountain", "txt"])
        .blocking_pick_file()
        .and_then(resolve_dialog_path);

    let Some(file_path) = file_path else {
        return Ok(None);
    };

    let fountain_text = std::fs::read_to_string(&file_path)
        .map_err(|err| format!("failed to read fountain document: {err}"))?;
    let mut screenplay = state
        .screenplay_service
        .get_active_screenplay()
        .map_err(|err| err.to_string())?;

    screenplay.title = file_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| screenplay.title.clone());
    screenplay.fountain_text = fountain_text;

    state
        .screenplay_service
        .save_screenplay(screenplay)
        .map(Into::into)
        .map(Some)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn export_fountain_document(
    app: AppHandle,
    request: SaveDocumentRequest,
) -> Result<Option<String>, String> {
    let file_path = app
        .dialog()
        .file()
        .add_filter("Fountain", &["fountain", "txt"])
        .set_file_name(default_fountain_filename(&request.screenplay.title))
        .blocking_save_file()
        .and_then(resolve_dialog_path);

    let Some(file_path) = file_path else {
        return Ok(None);
    };

    std::fs::write(&file_path, request.screenplay.fountain_text)
        .map_err(|err| format!("failed to write fountain export: {err}"))?;

    Ok(Some(file_path.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn open_project_document(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Option<DocumentFileDto>, String> {
    let file_path = app
        .dialog()
        .file()
        .add_filter("Lekhani Project", &["lekhani"])
        .blocking_pick_file()
        .and_then(resolve_dialog_path);

    let Some(file_path) = file_path else {
        return Ok(None);
    };

    state.switch_project_path(&file_path)?;
    let screenplay = state
        .screenplay_service
        .get_active_screenplay()
        .map(Into::into)
        .map_err(|err| err.to_string())?;

    Ok(Some(DocumentFileDto {
        screenplay,
        file_path: Some(file_path.to_string_lossy().to_string()),
    }))
}

#[tauri::command]
pub fn save_project_document_as(
    state: State<'_, AppState>,
    app: AppHandle,
    request: SaveDocumentRequest,
) -> Result<Option<DocumentFileDto>, String> {
    let file_path = request.file_path.clone().map(PathBuf::from).or_else(|| {
        app.dialog()
            .file()
            .add_filter("Lekhani Project", &["lekhani"])
            .set_file_name(default_project_filename(&request.screenplay.title))
            .blocking_save_file()
            .and_then(resolve_dialog_path)
    });

    let Some(file_path) = file_path else {
        return Ok(None);
    };

    state.clone_project_to(&file_path)?;

    Ok(Some(DocumentFileDto {
        screenplay: state
            .screenplay_service
            .get_active_screenplay()
            .map(Into::into)
            .map_err(|err| err.to_string())?,
        file_path: Some(file_path.to_string_lossy().to_string()),
    }))
}

#[tauri::command]
pub fn parse_character(
    state: State<'_, AppState>,
    request: ParseDescriptionRequest,
) -> Result<NarrativeCharacterDto, String> {
    let snapshot = state.get_snapshot()?;
    let character = state
        .narrative_service
        .parse_character(&request.description, &snapshot)?;
    state.store_character(character.clone())?;
    Ok(character)
}

#[tauri::command]
pub async fn preview_narrative_input(
    app: AppHandle,
    request: ParseDescriptionRequest,
) -> Result<PreviewNarrativeInputDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        preview_narrative_input_inner(&state, request)
    })
    .await
    .map_err(|err| format!("failed to join preview task: {err}"))?
}

#[tauri::command]
pub async fn commit_narrative_input(
    app: AppHandle,
    request: CommitNarrativeInputRequest,
) -> Result<PreviewNarrativeInputDto, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        commit_narrative_input_inner(&state, request)
    })
    .await
    .map_err(|err| format!("failed to join commit task: {err}"))?
}

#[tauri::command]
pub async fn submit_narrative_turn(
    app: AppHandle,
    request: CommitNarrativeInputRequest,
) -> Result<NarrativeTurnDto, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<NarrativeTurnDto, String> {
        let state = app.state::<AppState>();
        submit_turn(&state, &request.prompt).map(|turn| NarrativeTurnDto {
            reply_title: turn.reply_title,
            reply_body: turn.reply_body,
            committed: turn.committed,
        })
    })
    .await
    .unwrap_or_else(|err| Err(format!("failed to join narrative turn task: {err}")))
}

#[tauri::command]
pub async fn submit_assistant_turn(
    app: AppHandle,
    request: CommitNarrativeInputRequest,
) -> Result<AssistantTurnDto, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<AssistantTurnDto, String> {
        let state = app.state::<AppState>();
        submit_turn(&state, &request.prompt).map(|turn| AssistantTurnDto {
            intent: turn.intent,
            capabilities: turn.capabilities,
            write_policy: turn.write_policy,
            reply_title: turn.reply_title,
            reply_body: turn.reply_body,
            committed: turn.committed,
            working_memory: turn.working_memory,
            suggested_actions: turn.suggested_actions,
        })
    })
    .await
    .unwrap_or_else(|err| Err(format!("failed to join assistant turn task: {err}")))
}

#[tauri::command]
pub async fn apply_narrative_suggestion(
    app: AppHandle,
    request: NarrativeSuggestionActionRequest,
) -> Result<AssistantTurnDto, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<AssistantTurnDto, String> {
        let state = app.state::<AppState>();
        apply_suggestion_action(&state, request.action).map(|turn| AssistantTurnDto {
            intent: turn.intent,
            capabilities: turn.capabilities,
            write_policy: turn.write_policy,
            reply_title: turn.reply_title,
            reply_body: turn.reply_body,
            committed: turn.committed,
            working_memory: turn.working_memory,
            suggested_actions: turn.suggested_actions,
        })
    })
    .await
    .unwrap_or_else(|err| Err(format!("failed to join narrative suggestion task: {err}")))
}

fn preview_narrative_input_inner(
    state: &AppState,
    request: ParseDescriptionRequest,
) -> Result<PreviewNarrativeInputDto, String> {
    preview_turn(state, &request.description)
}

fn commit_narrative_input_inner(
    state: &AppState,
    request: CommitNarrativeInputRequest,
) -> Result<PreviewNarrativeInputDto, String> {
    submit_turn(state, &request.prompt).map(|turn| turn.committed)
}

#[tauri::command]
pub fn parse_event(
    state: State<'_, AppState>,
    request: ParseDescriptionRequest,
) -> Result<NarrativeEventDto, String> {
    let snapshot = state.get_snapshot()?;
    let event = state
        .narrative_service
        .parse_event(&request.description, &snapshot)?;
    state.store_event(event.clone())?;
    Ok(event)
}

#[tauri::command]
pub fn get_nudge(state: State<'_, AppState>) -> Result<NarrativeNudgeDto, String> {
    let snapshot = state.get_snapshot()?;
    state.narrative_service.get_nudge(&snapshot)
}

#[tauri::command]
pub fn get_narrative_snapshot(state: State<'_, AppState>) -> Result<NarrativeSnapshotDto, String> {
    state.get_snapshot()
}

#[tauri::command]
pub fn clear_narrative_state(state: State<'_, AppState>) -> Result<(), String> {
    state.clear_narrative_state()?;
    Ok::<(), String>(())
}

fn resolve_dialog_path(file_path: tauri_plugin_dialog::FilePath) -> Option<PathBuf> {
    match file_path {
        tauri_plugin_dialog::FilePath::Path(path) => Some(path),
        _ => None,
    }
}

fn default_project_filename(title: &str) -> String {
    let slug = slugify_title(title);

    if slug.is_empty() {
        "untitled-screenplay.lekhani".to_string()
    } else {
        format!("{slug}.lekhani")
    }
}

fn default_fountain_filename(title: &str) -> String {
    let slug = slugify_title(title);

    if slug.is_empty() {
        "untitled-screenplay.fountain".to_string()
    } else {
        format!("{slug}.fountain")
    }
}


fn slugify_title(title: &str) -> String {
    let slug = title
        .trim()
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' => character.to_ascii_lowercase(),
            _ => '-',
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();

    slug
}
