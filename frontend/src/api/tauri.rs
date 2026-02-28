use crate::api::dto::{
    AssistantTurnDto, CommitNarrativeInputRequest, DocumentFileDto, LlmStatusDto,
    NarrativeNudgeDto, NarrativeSnapshotDto, ParseDescriptionRequest, PreviewNarrativeInputDto,
    SaveDocumentRequest, SyncDebugDto, WorkingMemoryDto,
    SaveScreenplayRequest, ScreenplayDto,
};
use gloo_utils::format::JsValueSerdeExt;
use js_sys::{Function, Object, Promise, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

fn tauri_invoke_function() -> Result<Function, String> {
    let tauri = tauri_namespace()?;
    let namespace = Reflect::get(&tauri, &JsValue::from_str("core"))
        .ok()
        .filter(|value| !value.is_undefined() && !value.is_null())
        .or_else(|| Reflect::get(&tauri, &JsValue::from_str("tauri")).ok())
        .ok_or_else(|| "Tauri invoke namespace is not available".to_string())?;

    Reflect::get(&namespace, &JsValue::from_str("invoke"))
        .map_err(|err| format!("failed to access Tauri invoke: {err:?}"))?
        .dyn_into::<Function>()
        .map_err(|_| "Tauri invoke is not a function".to_string())
}

fn tauri_namespace() -> Result<JsValue, String> {
    let window = web_sys::window().ok_or_else(|| "window is not available".to_string())?;
    let tauri = Reflect::get(window.as_ref(), &JsValue::from_str("__TAURI__"))
        .map_err(|err| format!("failed to access __TAURI__: {err:?}"))?;

    if tauri.is_undefined() || tauri.is_null() {
        return Err("Tauri API is not available in this context".to_string());
    }

    Ok(tauri)
}

async fn invoke_tauri(cmd: &str, args: JsValue) -> Result<JsValue, String> {
    let invoke = tauri_invoke_function()?;
    let promise = invoke
        .call2(&JsValue::NULL, &JsValue::from_str(cmd), &args)
        .map_err(|err| format!("failed to call Tauri invoke: {err:?}"))?;

    JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|err| format!("invoke error: {err:?}"))
}

fn named_request_args<T: serde::Serialize>(request: &T) -> Result<JsValue, String> {
    let payload = serde_json::json!({ "request": request });
    JsValue::from_serde(&payload).map_err(|err| format!("JsValue conversion error: {err}"))
}

pub async fn get_screenplays() -> Result<Vec<ScreenplayDto>, String> {
    let js_value = invoke_tauri("get_screenplays", Object::new().into()).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn get_llm_status() -> Result<LlmStatusDto, String> {
    let js_value = invoke_tauri("get_llm_status", Object::new().into()).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn get_sync_debug() -> Result<SyncDebugDto, String> {
    let js_value = invoke_tauri("get_sync_debug", Object::new().into()).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn get_working_memory() -> Result<WorkingMemoryDto, String> {
    let js_value = invoke_tauri("get_working_memory", Object::new().into()).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn get_current_project() -> Result<DocumentFileDto, String> {
    let js_value = invoke_tauri("get_current_project", Object::new().into()).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn save_screenplay(screenplay: ScreenplayDto) -> Result<ScreenplayDto, String> {
    let args = SaveScreenplayRequest { screenplay };
    let js_args = named_request_args(&args)?;
    let js_value = invoke_tauri("save_screenplay", js_args).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn import_fountain_document() -> Result<Option<ScreenplayDto>, String> {
    let js_value = invoke_tauri("import_fountain_document", Object::new().into()).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn export_fountain_document(screenplay: ScreenplayDto) -> Result<Option<String>, String> {
    let args = SaveDocumentRequest {
        screenplay,
        file_path: None,
    };
    let js_args = named_request_args(&args)?;
    let js_value = invoke_tauri("export_fountain_document", js_args).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn open_project_document() -> Result<Option<DocumentFileDto>, String> {
    let js_value = invoke_tauri("open_project_document", Object::new().into()).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn save_project_document_as(
    screenplay: ScreenplayDto,
    file_path: Option<String>,
) -> Result<Option<DocumentFileDto>, String> {
    let args = SaveDocumentRequest {
        screenplay,
        file_path,
    };
    let js_args = named_request_args(&args)?;
    let js_value = invoke_tauri("save_project_document_as", js_args).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn preview_narrative_input(
    description: String,
) -> Result<PreviewNarrativeInputDto, String> {
    let args = ParseDescriptionRequest { description };
    let js_args = named_request_args(&args)?;
    let js_value = invoke_tauri("preview_narrative_input", js_args).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn submit_assistant_turn(
    prompt: String,
) -> Result<AssistantTurnDto, String> {
    let args = CommitNarrativeInputRequest { prompt };
    let js_args = named_request_args(&args)?;
    let js_value = invoke_tauri("submit_assistant_turn", js_args).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn get_nudge() -> Result<NarrativeNudgeDto, String> {
    let js_value = invoke_tauri("get_nudge", Object::new().into()).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn get_narrative_snapshot() -> Result<NarrativeSnapshotDto, String> {
    let js_value = invoke_tauri("get_narrative_snapshot", Object::new().into()).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

pub async fn listen_for_project_opened<F>(mut on_opened: F) -> Result<(), String>
where
    F: FnMut() + 'static,
{
    listen_for_event("project-opened", move |_payload| {
        on_opened();
    })
    .await
}

pub async fn listen_for_menu_action<F>(event_name: &'static str, mut on_action: F) -> Result<(), String>
where
    F: FnMut() + 'static,
{
    listen_for_event(event_name, move |_payload| {
        on_action();
    })
    .await
}

async fn listen_for_event<F>(event_name: &'static str, mut on_event: F) -> Result<(), String>
where
    F: FnMut(JsValue) + 'static,
{
    let tauri = tauri_namespace()?;
    let event_namespace = Reflect::get(&tauri, &JsValue::from_str("event"))
        .map_err(|err| format!("failed to access Tauri event namespace: {err:?}"))?;

    if event_namespace.is_undefined() || event_namespace.is_null() {
        return Ok(());
    }

    let listen = Reflect::get(&event_namespace, &JsValue::from_str("listen"))
        .map_err(|err| format!("failed to access Tauri event listener: {err:?}"))?
        .dyn_into::<Function>()
        .map_err(|_| "Tauri event listen is not a function".to_string())?;

    let callback = Closure::wrap(Box::new(move |payload: JsValue| {
        on_event(payload);
    }) as Box<dyn FnMut(JsValue)>);

    let promise = listen
        .call2(
            &event_namespace,
            &JsValue::from_str(event_name),
            callback.as_ref().unchecked_ref(),
        )
        .map_err(|err| format!("failed to register {event_name} listener: {err:?}"))?;

    let _ = JsFuture::from(Promise::from(promise))
        .await
        .map_err(|err| format!("failed to await {event_name} listener: {err:?}"))?;

    callback.forget();
    Ok(())
}
