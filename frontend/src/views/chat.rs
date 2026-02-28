use crate::api::dto::{NarrativeChangeKindDto, NarrativeCommitTargetDto, PreviewNarrativeInputDto};
use crate::state::document::DocumentContext;
use crate::state::narrative::{
    create_commit_action, create_llm_status_resource, create_preview_resource,
};
use gloo_timers::callback::Timeout;
use leptos::*;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, PartialEq)]
enum ChatRole {
    User,
    Assistant,
}

#[derive(Clone, PartialEq)]
struct ChatMessage {
    role: ChatRole,
    title: Option<String>,
    body: String,
}

#[component]
pub fn ChatInterface() -> impl IntoView {
    let document = use_context::<DocumentContext>().expect("document context should exist");
    let (prompt, set_prompt) = create_signal(String::new());
    let (debounced_prompt, set_debounced_prompt) = create_signal(String::new());
    let llm_status = create_llm_status_resource();
    let preview = create_preview_resource(debounced_prompt);
    let commit_action = create_commit_action();
    let (messages, set_messages) = create_signal(vec![ChatMessage {
        role: ChatRole::Assistant,
        title: Some("Lekhani".to_string()),
        body: "Tell me about the story, a character, or a scene problem you are working through. I will turn that into structured narrative changes and help you keep moving.".to_string(),
    }]);
    let debounce_handle: Rc<RefCell<Option<Timeout>>> = Rc::new(RefCell::new(None));

    Effect::new({
        let debounce_handle = debounce_handle.clone();
        move |_| {
            let next_prompt = prompt.get();

            if let Some(timeout) = debounce_handle.borrow_mut().take() {
                timeout.cancel();
            }

            if next_prompt.trim().is_empty() {
                set_debounced_prompt.set(String::new());
                return;
            }

            let set_debounced_prompt = set_debounced_prompt;
            *debounce_handle.borrow_mut() = Some(Timeout::new(300, move || {
                set_debounced_prompt.set(next_prompt.clone());
            }));
        }
    });

    let send_message = move |_| {
        let current_prompt = prompt.get_untracked();
        if current_prompt.trim().is_empty() {
            return;
        }

        let message = current_prompt.trim().to_string();
        set_messages.update(|items| {
            items.push(ChatMessage {
                role: ChatRole::User,
                title: None,
                body: message.clone(),
            });
        });
        set_prompt.set(String::new());
        set_debounced_prompt.set(String::new());
        commit_action.dispatch(message);
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

            set_messages.update(|items| {
                items.push(ChatMessage {
                    role: ChatRole::Assistant,
                    title: Some(format!(
                        "Applied as {}",
                        match committed.suggested_target {
                            NarrativeCommitTargetDto::Character => "character",
                            NarrativeCommitTargetDto::Event => "event",
                        }
                    )),
                    body: summarize_commit(&committed),
                });
            });
        }
    });

    Effect::new(move |_| {
        if let Some(Err(err)) = commit_action.value().get() {
            set_messages.update(|items| {
                items.push(ChatMessage {
                    role: ChatRole::Assistant,
                    title: Some("Commit failed".to_string()),
                    body: err,
                });
            });
        }
    });

    view! {
        <section class="narrative-chat">
            <div class="mode-header narrative-header">
                <span class="eyebrow">"Narrative"</span>
                <h2>"Talk to the story assistant."</h2>
                <p>
                    "Describe characters, events, motivations, relationships, or structural problems. "
                    "Lekhani will turn that into narrative changes and keep the screenplay moving."
                </p>
                {move || match llm_status.get() {
                    None => view! { <span class="muted">"Checking assistant backend..."</span> }.into_view(),
                    Some(Ok(status)) => view! { <span class="muted">{format!("Assistant backend: {}", status.backend)}</span> }.into_view(),
                    Some(Err(_)) => view! { <span class="muted">"Assistant backend unavailable"</span> }.into_view(),
                }}
            </div>

            <div class="chat-thread">
                {move || {
                    messages
                        .get()
                        .into_iter()
                        .map(|message| {
                            let message_class = match message.role {
                                ChatRole::User => "chat-message chat-message-user",
                                ChatRole::Assistant => "chat-message chat-message-assistant",
                            };

                            view! {
                                <article class=message_class>
                                    {message.title.as_ref().map(|title| {
                                        view! { <span class="chat-message-title">{title.clone()}</span> }
                                    })}
                                    <p>{message.body}</p>
                                </article>
                            }
                        })
                        .collect_view()
                }}
            </div>

            <div class="chat-composer">
                <textarea
                    id="narrative-input"
                    class="assistant-input narrative-input-large"
                    prop:value=prompt
                    on:input=move |ev| set_prompt.set(event_target_value(&ev))
                    placeholder="Example: Rajan advises Prince Arjun, but secretly supports the rival claimant after the council attack."
                    rows=6
                />

                <div class="chat-draft-status">
                    {move || match preview.get() {
                        None => view! { <p class="muted">"Start typing to see how Lekhani is reading the message."</p> }.into_view(),
                        Some(Ok(preview)) if preview.changes.is_empty() => view! {
                            <p class="muted">"No inferred change yet."</p>
                        }.into_view(),
                        Some(Ok(preview)) => view! {
                            <div class="draft-readout">
                                <span class="eyebrow">"Current read"</span>
                                <strong>{match preview.suggested_target {
                                    NarrativeCommitTargetDto::Character => "Character",
                                    NarrativeCommitTargetDto::Event => "Event",
                                }}</strong>
                                <p>{summarize_preview(&preview)}</p>
                            </div>
                        }.into_view(),
                        Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                    }}
                    <button class="primary-button" on:click=send_message>
                        "Send"
                    </button>
                </div>
            </div>
        </section>
    }
}

fn summarize_preview(preview: &PreviewNarrativeInputDto) -> String {
    if preview.changes.is_empty() {
        return "No inferred change yet.".to_string();
    }

    preview
        .changes
        .iter()
        .map(|change| {
            let label = match change.kind {
                NarrativeChangeKindDto::AddCharacter => "Add character",
                NarrativeChangeKindDto::UpdateCharacter => "Update character",
                NarrativeChangeKindDto::AddEvent => "Add event",
                NarrativeChangeKindDto::UpdateEvent => "Update event",
                NarrativeChangeKindDto::AddRelationship => "Add relationship",
                NarrativeChangeKindDto::UpdateRelationship => "Update relationship",
            };
            format!("{label}: {}.", change.label)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn summarize_commit(preview: &PreviewNarrativeInputDto) -> String {
    if preview.changes.is_empty() {
        return "I did not infer a concrete structural change from that message.".to_string();
    }

    let mut lines = preview
        .changes
        .iter()
        .map(|change| {
            let label = match change.kind {
                NarrativeChangeKindDto::AddCharacter => "Added character",
                NarrativeChangeKindDto::UpdateCharacter => "Updated character",
                NarrativeChangeKindDto::AddEvent => "Added event",
                NarrativeChangeKindDto::UpdateEvent => "Updated event",
                NarrativeChangeKindDto::AddRelationship => "Added relationship",
                NarrativeChangeKindDto::UpdateRelationship => "Updated relationship",
            };
            format!("{label}: {}. {}", change.label, change.detail)
        })
        .collect::<Vec<_>>();

    if !preview.relationships.is_empty() {
        lines.push(format!(
            "Tracked {} relationship{} in the narrative model.",
            preview.relationships.len(),
            if preview.relationships.len() == 1 { "" } else { "s" }
        ));
    }

    lines.join(" ")
}
