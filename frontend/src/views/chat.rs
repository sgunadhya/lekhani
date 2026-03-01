use crate::api::dto::{
    AssistantIntentDto, ConstraintScopeDto, FocusKindDto, NarrativeChangeKindDto,
    NarrativeCommitTargetDto, PreviewNarrativeInputDto, TaskStatusDto, WorkingMemoryDto,
};
use crate::state::document::DocumentContext;
use crate::state::narrative::{
    create_llm_status_resource, create_nudge_resource, create_preview_resource,
    create_turn_action, ChatMessage, ChatRole, NarrativeChatContext,
};
use gloo_timers::callback::Timeout;
use leptos::*;
use std::cell::RefCell;
use std::rc::Rc;

#[component]
pub fn ChatInterface() -> impl IntoView {
    let document = use_context::<DocumentContext>().expect("document context should exist");
    let chat = use_context::<NarrativeChatContext>().expect("narrative chat context should exist");
    let prompt = Signal::derive(move || chat.prompt.get());
    let set_prompt = move |value: String| chat.prompt.set(value);
    let (debounced_prompt, set_debounced_prompt) = create_signal(String::new());
    let (nudge_nonce, set_nudge_nonce) = create_signal(0_u64);
    let llm_status = create_llm_status_resource();
    let nudge = create_nudge_resource(nudge_nonce);
    let preview = create_preview_resource(debounced_prompt);
    let turn_action = create_turn_action();
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
        chat.messages.update(|items| {
            items.push(ChatMessage {
                role: ChatRole::User,
                title: None,
                body: message.clone(),
            });
        });
        chat.prompt.set(String::new());
        set_debounced_prompt.set(String::new());
        turn_action.dispatch(message);
    };

    Effect::new(move |_| {
        if let Some(Ok(turn)) = turn_action.value().get() {
            set_nudge_nonce.update(|value| *value += 1);
            chat.working_memory.set(Some(turn.working_memory.clone()));
            chat.last_intent.set(Some(turn.intent.clone()));
            let committed = turn.committed;
            let prompt = committed.prompt.trim().to_string();
            if !prompt.is_empty() && turn.intent == AssistantIntentDto::MutateOntology {
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
            } else if turn.intent == AssistantIntentDto::MutateDocument {
                let draft_text = turn.reply_body.trim().to_string();
                if !draft_text.is_empty() {
                    document.document.update(|current| {
                        if let Some(current) = current {
                            if !current.fountain_text.trim().is_empty() {
                                current.fountain_text.push_str("\n\n");
                            }
                            current.fountain_text.push_str(&draft_text);
                        }
                    });
                }
            }

            chat.messages.update(|items| {
                items.push(ChatMessage {
                    role: ChatRole::Assistant,
                    title: Some(match turn.intent {
                        AssistantIntentDto::Query => format!("{} · Query", turn.reply_title),
                        AssistantIntentDto::Guide => format!("{} · Guidance", turn.reply_title),
                        AssistantIntentDto::Clarify => format!("{} · Clarify", turn.reply_title),
                        AssistantIntentDto::MutateOntology => turn.reply_title,
                        AssistantIntentDto::MutateDocument => format!("{} · Document", turn.reply_title),
                        AssistantIntentDto::ProposeSync => format!("{} · Sync", turn.reply_title),
                        AssistantIntentDto::ResolveLint => format!("{} · Lint", turn.reply_title),
                    }),
                    body: turn.reply_body,
                });
            });
        }
    });

    Effect::new(move |_| {
        if let Some(Err(err)) = turn_action.value().get() {
            chat.messages.update(|items| {
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

            <div class="narrative-context-strip">
                {move || match chat.working_memory.get() {
                    Some(memory) => render_working_memory(&memory).into_view(),
                    _ => view! {
                        <div class="narrative-context-grid">
                            <section class="narrative-context-panel">
                                <span class="eyebrow">"Current thread"</span>
                                <p class="narrative-context-value">"Story"</p>
                                <p class="muted">"Assistant context will appear here as the conversation settles."</p>
                            </section>
                        </div>
                    }.into_view(),
                }}
            </div>

            <div class="narrative-nudge-strip">
                <span class="eyebrow">"Next nudge"</span>
                {move || match nudge.get() {
                    None => view! { <p class="muted">"Thinking about the next useful move..."</p> }.into_view(),
                    Some(Ok(nudge)) => view! { <p>{nudge.message}</p> }.into_view(),
                    Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                }}
            </div>

            <div class="chat-thread">
                {move || {
                    chat.messages
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
                                    <div class="chat-message-body">{render_markdown(&message.body)}</div>
                                </article>
                            }
                        })
                        .collect_view()
                }}
                {move || if turn_action.pending().get() {
                    view! {
                        <div class="chat-message chat-message-assistant processing-indicator">
                            <span class="chat-message-title">"Lekhani is thinking..."</span>
                            <div class="dot-flashing"></div>
                        </div>
                    }.into_view()
                } else {
                    view! { <div></div> }.into_view()
                }}
            </div>

            <div class="chat-composer">
                <textarea
                    id="narrative-input"
                    class="assistant-input narrative-input-large"
                    prop:value=prompt
                    on:input=move |ev| set_prompt(event_target_value(&ev))
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

fn render_working_memory(memory: &WorkingMemoryDto) -> impl IntoView {
    let current_thread = current_thread_label(memory);
    let anchor = memory.current_focus.as_ref().map(|focus| focus.summary.clone());
    let active_constraint = memory
        .constraints
        .iter()
        .find(|constraint| matches!(constraint.status, crate::api::dto::ConstraintStatusDto::Active))
        .map(|constraint| constraint.value.clone());
    let active_question = memory.open_questions.first().map(|question| question.question.clone());
    let deferred = memory
        .story_backlog
        .iter()
        .filter(|task| matches!(task.status, TaskStatusDto::Open))
        .collect::<Vec<_>>();
    let deferred_summary = deferred.first().map(|task| {
        if deferred.len() > 1 {
            format!("{} (+{} more)", task.description, deferred.len() - 1)
        } else {
            task.description.clone()
        }
    });

    view! {
        <div class="narrative-context-grid">
            <section class="narrative-context-panel">
                <span class="eyebrow">"Current thread"</span>
                <p class="narrative-context-value">{current_thread}</p>
            </section>

            {anchor.map(|anchor| {
                view! {
                    <section class="narrative-context-panel">
                        <span class="eyebrow">"Current anchor"</span>
                        <p class="narrative-context-value">{anchor}</p>
                    </section>
                }
            })}

            {active_constraint.map(|constraint| {
                view! {
                    <section class="narrative-context-panel">
                        <span class="eyebrow">"Constraint"</span>
                        <p class="narrative-context-value">{constraint}</p>
                    </section>
                }
            })}

            {active_question.map(|question| {
                view! {
                    <section class="narrative-context-panel">
                        <span class="eyebrow">"Next question"</span>
                        <p class="narrative-context-value">{question}</p>
                    </section>
                }
            })}

            {deferred_summary.map(|summary| {
                view! {
                    <section class="narrative-context-panel">
                        <span class="eyebrow">"Deferred"</span>
                        <p class="narrative-context-value">{summary}</p>
                    </section>
                }
            })}
        </div>
    }
}

fn current_thread_label(memory: &WorkingMemoryDto) -> String {
    if let Some(focus) = &memory.current_focus {
        return match focus.kind {
            FocusKindDto::Character => "Character".to_string(),
            FocusKindDto::Event => "Event".to_string(),
            FocusKindDto::Relationship => "Relationship".to_string(),
            FocusKindDto::Scene => "Scene".to_string(),
            FocusKindDto::Structure => "Structure".to_string(),
            FocusKindDto::LintResolution => "Lint".to_string(),
            FocusKindDto::OpenQuestion => "Open question".to_string(),
        };
    }

    if let Some(constraint) = memory.constraints.first() {
        return match constraint.scope {
            ConstraintScopeDto::Setting => "Setting".to_string(),
            ConstraintScopeDto::Character => "Character".to_string(),
            ConstraintScopeDto::Event => "Event".to_string(),
            ConstraintScopeDto::Relationship => "Relationship".to_string(),
            ConstraintScopeDto::Tone => "Tone".to_string(),
            ConstraintScopeDto::Structure => "Structure".to_string(),
            ConstraintScopeDto::General => "Story".to_string(),
        };
    }

    "Story".to_string()
}

fn render_markdown(text: &str) -> impl IntoView {
    let mut result = String::new();
    let mut is_bold = false;
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if is_bold {
                result.push_str("</strong>");
            } else {
                result.push_str("<strong>");
            }
            is_bold = !is_bold;
            i += 2;
        } else if chars[i] == '\n' {
            result.push_str("<br/>");
            i += 1;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    if is_bold {
        result.push_str("</strong>");
    }

    view! { <div inner_html=result></div> }
}
