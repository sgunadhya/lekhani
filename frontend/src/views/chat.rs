use crate::api::dto::{NarrativeChangeKindDto, NarrativeCommitTargetDto};
use crate::state::document::DocumentContext;
use crate::state::narrative::{
    create_commit_action, create_llm_status_resource, create_nudge_resource, create_preview_resource,
    create_snapshot_resource,
};
use gloo_timers::callback::Timeout;
use leptos::*;
use std::cell::RefCell;
use std::rc::Rc;

#[component]
pub fn ChatInterface() -> impl IntoView {
    let document = use_context::<DocumentContext>().expect("document context should exist");
    let (prompt, set_prompt) = create_signal(String::new());
    let (debounced_prompt, set_debounced_prompt) = create_signal(String::new());
    let (refresh_nonce, set_refresh_nonce) = create_signal(0_u64);
    let nudge = create_nudge_resource(refresh_nonce);
    let snapshot = create_snapshot_resource(refresh_nonce);
    let llm_status = create_llm_status_resource();
    let preview = create_preview_resource(debounced_prompt);
    let commit_action = create_commit_action();
    let debounce_handle: Rc<RefCell<Option<Timeout>>> = Rc::new(RefCell::new(None));

    Effect::new({
        let debounce_handle = debounce_handle.clone();
        move |_| {
            let next_prompt = prompt.get();

            if let Some(timeout) = debounce_handle.borrow_mut().take() {
                timeout.cancel();
            }

            if next_prompt.trim().is_empty() {
                set_debounced_prompt.set(next_prompt);
                return;
            }

            let set_debounced_prompt = set_debounced_prompt;
            *debounce_handle.borrow_mut() = Some(Timeout::new(300, move || {
                set_debounced_prompt.set(next_prompt.clone());
            }));
        }
    });

    let commit_preview = move |_| {
        let current_prompt = prompt.get_untracked();
        if !current_prompt.trim().is_empty() {
            commit_action.dispatch(current_prompt);
        }
    };

    Effect::new(move |_| {
        if let Some(Ok(committed)) = commit_action.value().get() {
            let prompt = committed.prompt.trim().to_string();
            if !prompt.is_empty() {
                document.document.update(|current| {
                    if let Some(current) = current {
                        if !current.fountain_text.trim().is_empty() {
                            current.fountain_text.push_str("\n\n");
                        }
                        current
                            .fountain_text
                            .push_str(&format!("[[Narrative note: {}]]", prompt));
                    }
                });
            }
            set_refresh_nonce.update(|value| *value += 1);
        }
    });

    let target_label = move || match preview.get() {
        Some(Ok(preview)) => match preview.suggested_target {
            NarrativeCommitTargetDto::Character => "Character",
            NarrativeCommitTargetDto::Event => "Event",
        },
        _ => "Narrative item",
    };

    view! {
        <section class="narrative-workspace">
            <div class="narrative-editor-panel">
                <div class="mode-header narrative-header">
                    <span class="eyebrow">"Narrative"</span>
                    <h2>"Describe the story. Commit when the side pane looks right."</h2>
                    <p>"Lekhani infers the ontology in the background and shows proposed additions, updates, and relationships as you type."</p>
                </div>

                <label class="chat-label" for="narrative-input">"Narrative input"</label>
                <textarea
                    id="narrative-input"
                    class="assistant-input narrative-input-large"
                    prop:value=prompt
                    on:input=move |ev| set_prompt.set(event_target_value(&ev))
                    placeholder="Example: Rajan advises Prince Arjun, but secretly supports the rival claimant after the council attack."
                    rows=14
                />

                <div class="narrative-editor-footer">
                    <div class="editor-status-line">
                        <span class="eyebrow">"Current read"</span>
                        <strong>{target_label}</strong>
                    </div>
                    <button class="primary-button" on:click=commit_preview>
                        "Commit"
                    </button>
                </div>
            </div>

            <aside class="narrative-preview-pane">
                <div class="preview-section">
                    <div class="preview-section-header">
                        <span class="eyebrow">"Preview"</span>
                        {move || match llm_status.get() {
                            None => view! { <span class="muted">"Checking backend..."</span> }.into_view(),
                            Some(Ok(status)) => view! { <span class="muted">{status.backend}</span> }.into_view(),
                            Some(Err(_)) => view! { <span class="muted">"backend unavailable"</span> }.into_view(),
                        }}
                    </div>

                    {move || match preview.get() {
                        None => view! { <p class="muted">"Start typing to see inferred ontology items."</p> }.into_view(),
                        Some(Ok(preview)) if preview.changes.is_empty() => view! {
                            <p class="muted">"No inferred change yet."</p>
                        }.into_view(),
                        Some(Ok(preview)) => view! {
                            <div class="inference-list">
                                {preview.changes.into_iter().map(|change| {
                                    let label = match change.kind {
                                        NarrativeChangeKindDto::AddCharacter => "Add character",
                                        NarrativeChangeKindDto::UpdateCharacter => "Update character",
                                        NarrativeChangeKindDto::AddEvent => "Add event",
                                        NarrativeChangeKindDto::UpdateEvent => "Update event",
                                        NarrativeChangeKindDto::AddRelationship => "Add relationship",
                                        NarrativeChangeKindDto::UpdateRelationship => "Update relationship",
                                    };

                                    view! {
                                        <div class="inference-item">
                                            <span class="inference-kind">{label}</span>
                                            <strong>{change.label}</strong>
                                            <p>{change.detail}</p>
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_view(),
                        Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                    }}
                </div>

                <div class="preview-section">
                    <span class="eyebrow">"Progress"</span>
                    {move || match snapshot.get() {
                        None => view! { <p class="muted">"Loading progress..."</p> }.into_view(),
                        Some(Ok(snapshot)) => view! {
                            <div class="progress-grid">
                                <div class="metric-card">
                                    <span class="metric-label">"Characters"</span>
                                    <strong>{snapshot.metrics.character_count}</strong>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Events"</span>
                                    <strong>{snapshot.metrics.event_count}</strong>
                                </div>
                                <div class="metric-card">
                                    <span class="metric-label">"Relationships"</span>
                                    <strong>{snapshot.ontology_graph.relationships.len()}</strong>
                                </div>
                            </div>
                        }.into_view(),
                        Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                    }}
                </div>

                <div class="preview-section">
                    <span class="eyebrow">"Next nudge"</span>
                    {move || match nudge.get() {
                        None => view! { <p class="muted">"Generating nudge..."</p> }.into_view(),
                        Some(Ok(nudge)) => view! { <p>{nudge.message}</p> }.into_view(),
                        Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                    }}
                </div>

                <div class="preview-section">
                    <span class="eyebrow">"Commit status"</span>
                    {move || match commit_action.value().get() {
                        None => view! { <p class="muted">"Nothing committed yet from the current prompt."</p> }.into_view(),
                        Some(Ok(preview)) => view! {
                            <p>{format!("Committed {}.", match preview.suggested_target {
                                NarrativeCommitTargetDto::Character => "a character-focused change",
                                NarrativeCommitTargetDto::Event => "an event-focused change",
                            })}</p>
                        }.into_view(),
                        Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                    }}
                </div>
            </aside>
        </section>
    }
}
