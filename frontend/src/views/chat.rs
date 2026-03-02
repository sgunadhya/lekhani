use crate::api::dto::{
    AssistantIntentDto, BeatIdDto, ConversationModeDto, ConversationTopicDto,
    InterpretationTargetDto, NarrativeModeDto, NarrativeSuggestedActionViewDto,
    NarrativeSuggestionActionDto, TaskStatusDto, ThreadScopeDto, ThreadStatusDto, TurnRouteDto,
    WorkingMemoryDto,
};
use crate::state::document::DocumentContext;
use crate::state::narrative::{
    create_llm_status_resource, create_suggestion_action, create_turn_action, ChatMessage,
    ChatRole, NarrativeChatContext,
};
use leptos::*;

#[component]
pub fn ChatInterface() -> impl IntoView {
    let document = use_context::<DocumentContext>().expect("document context should exist");
    let chat = use_context::<NarrativeChatContext>().expect("narrative chat context should exist");
    let prompt = Signal::derive(move || chat.prompt.get());
    let set_prompt = move |value: String| chat.prompt.set(value);
    let llm_status = create_llm_status_resource();
    let turn_action = create_turn_action();
    let suggestion_action = create_suggestion_action();

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
        turn_action.dispatch(message);
    };

    let apply_turn = move |turn: crate::api::dto::AssistantTurnDto| {
        chat.working_memory.set(Some(turn.working_memory.clone()));
        chat.last_intent.set(Some(turn.intent.clone()));
        chat.last_mode.set(Some(turn.narrative_mode.clone()));
        chat.last_thread_status.set(Some(turn.thread_status.clone()));
        chat.last_interpretation_target
            .set(Some(turn.interpretation_target.clone()));
        chat.last_interpretation_route
            .set(Some(turn.interpretation_route.clone()));
        chat.last_interpretation_confidence
            .set(Some(turn.interpretation_confidence));
        chat.last_beat.set(turn.active_beat.clone());
        chat.last_evaluation_nudge.set(turn.evaluation_nudge.clone());
        chat.suggested_actions.set(turn.suggested_actions.clone());
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
    };

    Effect::new({
        let apply_turn = apply_turn.clone();
        move |_| {
            if let Some(Ok(turn)) = turn_action.value().get() {
                apply_turn(turn);
            }
        }
    });

    Effect::new({
        let apply_turn = apply_turn.clone();
        move |_| {
            if let Some(Ok(turn)) = suggestion_action.value().get() {
                apply_turn(turn);
            }
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
                <div class="narrative-context-grid">
                    {move || {
                        let mode = chat.last_mode.get();
                        let status = chat.last_thread_status.get();
                        let target = chat.last_interpretation_target.get();
                        let route = chat.last_interpretation_route.get();
                        let confidence = chat.last_interpretation_confidence.get();
                        let beat = chat.last_beat.get();

                        if mode.is_none() && status.is_none() && beat.is_none() && target.is_none() && route.is_none() {
                            view! { <div></div> }.into_view()
                        } else {
                            view! {
                                <section class="narrative-context-panel">
                                    <span class="eyebrow">"Interaction state"</span>
                                    <p class="narrative-context-value">
                                        {interaction_state_label(mode.as_ref(), status.as_ref())}
                                    </p>
                                    {beat.map(|beat| {
                                        view! {
                                            <p class="muted narrative-context-note">
                                                {format!("Beat: {}", beat_label(&beat))}
                                            </p>
                                        }
                                    })}
                                    {target.map(|target| {
                                        view! {
                                            <p class="muted narrative-context-note">
                                                {format!(
                                                    "Interpretation: {}{}",
                                                    interpretation_target_label(&target),
                                                    confidence
                                                        .map(|value| format!(" ({:.0}%)", value * 100.0))
                                                        .unwrap_or_default()
                                                )}
                                            </p>
                                        }
                                    })}
                                    {route.map(|route| {
                                        view! {
                                            <p class="muted narrative-context-note">
                                                {format!("Route: {}", turn_route_label(&route))}
                                            </p>
                                        }
                                    })}
                                </section>
                            }
                                .into_view()
                        }
                    }}
                </div>
            </div>

            {move || {
                let actions = chat.suggested_actions.get();
                if actions.is_empty() {
                    view! { <div></div> }.into_view()
                } else {
                    view! {
                        <div class="narrative-suggestion-row">
                            {actions
                                .into_iter()
                                .map(|action| render_suggested_action(action, suggestion_action))
                                .collect_view()}
                        </div>
                    }
                        .into_view()
                }
            }}

            <div class="narrative-nudge-strip">
                <span class="eyebrow">"Next nudge"</span>
                {move || {
                    if let Some(nudge) = chat.last_evaluation_nudge.get() {
                        view! { <p>{nudge}</p> }.into_view()
                    } else {
                        view! { <p class="muted">"Take one more turn to surface the next move."</p> }
                            .into_view()
                    }
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
                {move || if turn_action.pending().get() || suggestion_action.pending().get() {
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
                    <p class="muted">
                        "Typing stays local. Nothing is inferred or committed until you send a message or choose a suggestion."
                    </p>
                    <button class="primary-button" on:click=send_message>
                        "Send"
                    </button>
                </div>
            </div>
        </section>
    }
}

fn render_suggested_action(
    action: NarrativeSuggestedActionViewDto,
    suggestion_action: Action<NarrativeSuggestionActionDto, Result<crate::api::dto::AssistantTurnDto, String>>,
) -> impl IntoView {
    let button_class = if action.primary {
        "primary-button"
    } else {
        "secondary-button"
    };
    let button_action = action.action.clone();
    let label = action.label.clone();

    view! {
        <button class=button_class on:click=move |_| suggestion_action.dispatch(button_action.clone())>
            {label}
        </button>
    }
}

fn render_working_memory(memory: &WorkingMemoryDto) -> impl IntoView {
    let current_thread = current_thread_label(memory);
    let thread_status = current_thread_status_label(&memory.current_thread.status);
    let thread_scope = thread_scope_label(&memory.current_thread.scope);
    let anchor = memory
        .current_thread
        .current_focus
        .as_ref()
        .map(|focus| focus.summary.clone());
    let return_thread = memory
        .return_thread
        .as_ref()
        .map(|thread| thread_label(thread));
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
                <p class="muted narrative-context-note">{conversation_mode_label(&memory.conversation_mode)}</p>
                <p class="muted narrative-context-note">{thread_status}</p>
                <p class="muted narrative-context-note">{thread_scope}</p>
                <p class="muted narrative-context-note">{format!("Turn {}", memory.current_thread.turn_count.max(memory.turn_count))}</p>
            </section>

            {return_thread.map(|goal| {
                view! {
                    <section class="narrative-context-panel">
                        <span class="eyebrow">"Return thread"</span>
                        <p class="narrative-context-value">{goal}</p>
                    </section>
                }
            })}

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
    thread_label(&memory.current_thread)
}

fn thread_label(thread: &crate::api::dto::NarrativeThreadDto) -> String {
    if !thread.goal.trim().is_empty() {
        return thread.goal.clone();
    }

    match thread.topic {
        ConversationTopicDto::Setting => return "Setting".to_string(),
        ConversationTopicDto::Character => return "Character".to_string(),
        ConversationTopicDto::Event => return "Event".to_string(),
        ConversationTopicDto::Relationship => return "Relationship".to_string(),
        ConversationTopicDto::General => {}
    }

    "Story".to_string()
}

fn current_thread_status_label(status: &crate::api::dto::NarrativeThreadStatusDto) -> &'static str {
    match status {
        crate::api::dto::NarrativeThreadStatusDto::Active => "Active thread",
        crate::api::dto::NarrativeThreadStatusDto::Parked => "Parked thread",
        crate::api::dto::NarrativeThreadStatusDto::Committed => "Committed thread",
    }
}

fn thread_scope_label(scope: &ThreadScopeDto) -> &'static str {
    match scope {
        ThreadScopeDto::Main => "Main thread",
        ThreadScopeDto::Sidequest => "Sidequest",
    }
}

fn conversation_mode_label(mode: &ConversationModeDto) -> &'static str {
    match mode {
        ConversationModeDto::Brainstorming => "Exploring ideas",
        ConversationModeDto::Refining => "Refining the current idea",
        ConversationModeDto::Committing => "Recording the current direction",
    }
}

fn interaction_state_label(
    mode: Option<&NarrativeModeDto>,
    status: Option<&ThreadStatusDto>,
) -> String {
    let mode = mode.map(narrative_mode_label).unwrap_or("Unknown mode");
    let status = status.map(thread_status_label).unwrap_or("Unknown status");
    format!("{mode} · {status}")
}

fn narrative_mode_label(mode: &NarrativeModeDto) -> &'static str {
    match mode {
        NarrativeModeDto::Idle => "Idle",
        NarrativeModeDto::Brainstorming => "Brainstorming",
        NarrativeModeDto::Converging => "Converging",
        NarrativeModeDto::Elaborating => "Elaborating",
        NarrativeModeDto::Committing => "Committing",
        NarrativeModeDto::TunnelingSidequest => "Sidequest",
        NarrativeModeDto::Drifting => "Drifting",
    }
}

fn thread_status_label(status: &ThreadStatusDto) -> &'static str {
    match status {
        ThreadStatusDto::Active => "Active",
        ThreadStatusDto::Drifting => "Drifting",
        ThreadStatusDto::Converging => "Converging",
        ThreadStatusDto::Stalled => "Stalled",
    }
}

fn beat_label(beat: &BeatIdDto) -> &'static str {
    match beat {
        BeatIdDto::CandidateReady => "Candidate ready",
        BeatIdDto::DriftDetected => "Drift detected",
        BeatIdDto::ReadyToCommit => "Ready to commit",
        BeatIdDto::Stalled => "Stalled",
        BeatIdDto::SidequestOpened => "Sidequest opened",
        BeatIdDto::SidequestCloseable => "Sidequest closeable",
    }
}

fn interpretation_target_label(target: &InterpretationTargetDto) -> String {
    match target {
        InterpretationTargetDto::CurrentCandidate => "current candidate".to_string(),
        InterpretationTargetDto::NewTopic(topic) => {
            format!("new {}", current_topic_label(topic))
        }
        InterpretationTargetDto::Screenplay => "screenplay".to_string(),
        InterpretationTargetDto::General => "general".to_string(),
    }
}

fn turn_route_label(route: &TurnRouteDto) -> &'static str {
    match route {
        TurnRouteDto::Continue => "continue",
        TurnRouteDto::ElaborateCurrent => "elaborate current",
        TurnRouteDto::AlternativeCurrent => "try alternative",
        TurnRouteDto::ConfirmCurrent => "confirm current",
        TurnRouteDto::RejectCurrent => "reject current",
        TurnRouteDto::ShiftToCharacter => "shift to character",
        TurnRouteDto::ShiftToEvent => "shift to event",
        TurnRouteDto::AddToScreenplay => "add to screenplay",
    }
}

fn current_topic_label(topic: &ConversationTopicDto) -> &'static str {
    match topic {
        ConversationTopicDto::Setting => "setting",
        ConversationTopicDto::Character => "character",
        ConversationTopicDto::Event => "event",
        ConversationTopicDto::Relationship => "relationship",
        ConversationTopicDto::General => "story topic",
    }
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
