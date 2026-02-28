use chrono::{DateTime, Utc};
use gloo_utils::format::JsValueSerdeExt;
use js_sys::{Function, Object, Reflect};
use leptos::*;
use leptos_meta::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

fn tauri_invoke_function() -> Result<Function, String> {
    let window = web_sys::window().ok_or_else(|| "window is not available".to_string())?;
    let tauri = Reflect::get(window.as_ref(), &JsValue::from_str("__TAURI__"))
        .map_err(|err| format!("failed to access __TAURI__: {err:?}"))?;

    if tauri.is_undefined() || tauri.is_null() {
        return Err("Tauri API is not available in this context".to_string());
    }

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

async fn invoke_tauri(cmd: &str, args: JsValue) -> Result<JsValue, String> {
    let invoke = tauri_invoke_function()?;
    let promise = invoke
        .call2(&JsValue::NULL, &JsValue::from_str(cmd), &args)
        .map_err(|err| format!("failed to call Tauri invoke: {err:?}"))?;

    JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|err| format!("invoke error: {err:?}"))
}

async fn get_screenplays() -> Result<Vec<Screenplay>, String> {
    let js_value = invoke_tauri("get_screenplays", Object::new().into()).await?;
    js_value
        .into_serde()
        .map_err(|err| format!("Deserialize error: {err}"))
}

async fn save_screenplay(screenplay: Screenplay) -> Result<(), String> {
    let args = serde_json::json!({ "screenplay": screenplay });
    let js_args =
        JsValue::from_serde(&args).map_err(|err| format!("JsValue conversion error: {err}"))?;
    invoke_tauri("save_screenplay", js_args).await?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screenplay {
    pub id: Uuid,
    pub title: String,
    pub fountain_text: String,
    #[serde(skip)]
    pub parsed: Option<()>, // Placeholder
    pub version: u64,
    pub changes: Vec<ScreenplayChange>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenplayChange {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub author: String,
    pub change_type: ChangeType,
    pub range_start: usize,
    pub range_end: usize,
    pub new_text: String,
    pub old_text: String,
    pub provenance: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeType {
    Insert,
    Delete,
    Replace,
}



#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    _ = console_log::init_with_level(log::Level::Debug);

    let document = web_sys::window()
        .and_then(|window| window.document())
        .expect("document should be available");
    let root = document
        .get_element_by_id("root")
        .expect("root element should exist")
        .dyn_into::<web_sys::HtmlElement>()
        .expect("root should be an HtmlElement");

    mount_to(root, || view! { <App/> })
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Meta name="viewport" content="width=device-width, initial-scale=1.0"/>
        <Title text="Lekhani - Screenplay Writing Tool"/>

        <nav class="navbar">
            <h1>"Lekhani"</h1>
        </nav>
        <main class="main-content">
            <HomePage/>
        </main>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    let screenplays = create_resource(|| (), |_| async move { get_screenplays().await });

    view! {
        <div class="home">
            <h2>"Welcome to Mathura Struggle Screenplay Organizer"</h2>
            <p>"A tool for organizing your historical drama screenplay using GOLEM ontology."</p>
            <div>
                <h3>"Screenplays"</h3>
                {move || match screenplays.get() {
                    None => view! { <p>"Loading..."</p> }.into_view(),
                    Some(Ok(sps)) => view! {
                        <ul>
                            {sps.iter().map(|sp| view! {
                                <li>{&sp.title}</li>
                            }).collect_view()}
                        </ul>
                    }.into_view(),
                    Some(Err(e)) => view! { <p class="error">{"Error: "}{e}</p> }.into_view(),
                }}
            </div>
        </div>
    }
}

#[component]
fn ScreenplayEditor() -> impl IntoView {
    let (screenplay_text, set_screenplay_text) = create_signal(String::new());

    view! {
        <div class="editor">
            <h2>"Screenplay Editor"</h2>
            <textarea
                class="screenplay-textarea"
                prop:value=screenplay_text
                on:input=move |ev| set_screenplay_text.set(event_target_value(&ev))
                placeholder="Type your Fountain screenplay here..."
                rows=30
                cols=80
            />
        </div>
    }
}

#[component]
fn ChatInterface() -> impl IntoView {
    view! {
        <div class="chat">
            <h2>"Chat Interface"</h2>
            <p>"Natural language input for describing characters, events, etc."</p>
        </div>
    }
}

#[component]
fn TimelineView() -> impl IntoView {
    view! {
        <div class="timeline">
            <h2>"Timeline Visualization"</h2>
            <p>"Interactive timeline of events and character arcs."</p>
        </div>
    }
}
