use crate::api::dto::{
    NarrativeSnapshotDto, OntologyRelationshipKindDto, SyncDebugDto, ThreadScopeDto,
    WorkingMemoryDto,
};
use crate::state::narrative::{
    create_snapshot_resource, create_sync_debug_resource, create_working_memory_resource,
};
use leptos::*;

#[component]
pub fn TimelineView() -> impl IntoView {
    let (snapshot_nonce, set_snapshot_nonce) = create_signal(0_u64);
    let snapshot = create_snapshot_resource(snapshot_nonce);
    let sync_debug = create_sync_debug_resource(snapshot_nonce);
    let working_memory = create_working_memory_resource(snapshot_nonce);

    view! {
        <section class="visual-inspector">
            <div class="mode-header">
                <span class="eyebrow">"Visual"</span>
                <h2>"Inspect the current narrative model"</h2>
                <p>"This view is a readable projection of the story state. Make changes through Narrative mode, then come back here to inspect what the system now understands."</p>
            </div>

            <div class="visual-toolbar">
                <button class="secondary-button" on:click=move |_| set_snapshot_nonce.update(|value| *value += 1)>
                    "Refresh Derived Data"
                </button>
            </div>

            {move || match snapshot.get() {
                None => view! { <p class="muted">"Loading narrative inspector..."</p> }.into_view(),
                Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                Some(Ok(snapshot)) => {
                    let participant_relationships = describe_participant_relationships(&snapshot);
                    let character_relationships = describe_character_relationships(&snapshot);
                    let projection_relationships = describe_projection_relationships(&snapshot);

                    view! {
                        <div class="visual-inspector-body">
                            <section class="visual-section">
                                <span class="eyebrow">"Summary"</span>
                                <div class="progress-grid">
                                    <div class="metric-card">
                                        <span class="metric-label">"Scenes"</span>
                                        <strong>{snapshot.metrics.scene_count}</strong>
                                    </div>
                                    <div class="metric-card">
                                        <span class="metric-label">"Characters"</span>
                                        <strong>{snapshot.metrics.character_count}</strong>
                                    </div>
                                    <div class="metric-card">
                                        <span class="metric-label">"Events"</span>
                                        <strong>{snapshot.metrics.event_count}</strong>
                                    </div>
                                    <div class="metric-card">
                                        <span class="metric-label">"Entities"</span>
                                        <strong>{snapshot.ontology_graph.entities.len()}</strong>
                                    </div>
                                    <div class="metric-card">
                                        <span class="metric-label">"Relationships"</span>
                                        <strong>{snapshot.ontology_graph.relationships.len()}</strong>
                                    </div>
                                    <div class="metric-card">
                                        <span class="metric-label">"Projections"</span>
                                        <strong>{snapshot.projection_relationships.len()}</strong>
                                    </div>
                                </div>
                            </section>

                            <section class="visual-section">
                                <span class="eyebrow">"Assistant state"</span>
                                {render_working_memory(working_memory.get())}
                            </section>

                            <section class="visual-section">
                                <span class="eyebrow">"Timeline"</span>
                                {if snapshot.events.is_empty() {
                                    view! { <p class="muted">"No narrative events parsed yet."</p> }.into_view()
                                } else {
                                    view! {
                                        <div class="inspector-list">
                                            {snapshot.events.iter().map(|event| view! {
                                                <div class="inspector-row" data-id=event.id.to_string()>
                                                    <strong>{&event.title}</strong>
                                                    <p>{&event.summary}</p>
                                                </div>
                                            }).collect_view()}
                                        </div>
                                    }.into_view()
                                }}
                            </section>

                            <section class="visual-section">
                                <span class="eyebrow">"Characters"</span>
                                {if snapshot.characters.is_empty() {
                                    view! { <p class="muted">"No characters parsed yet."</p> }.into_view()
                                } else {
                                    view! {
                                        <div class="inspector-list">
                                            {snapshot.characters.iter().map(|character| view! {
                                                <div class="inspector-row" data-id=character.id.to_string()>
                                                    <strong>{&character.name}</strong>
                                                    <p>{&character.summary}</p>
                                                </div>
                                            }).collect_view()}
                                        </div>
                                    }.into_view()
                                }}
                            </section>

                            <section class="visual-section">
                                <span class="eyebrow">"Relationships"</span>
                                <div class="relationship-stack">
                                    <div class="relationship-group">
                                        <strong>"Participation"</strong>
                                        {if participant_relationships.is_empty() {
                                            view! { <p class="muted">"No event participation links yet."</p> }.into_view()
                                        } else {
                                            view! {
                                                <div class="inspector-list">
                                                    {participant_relationships.into_iter().map(|description| view! {
                                                        <div class="inspector-row">
                                                            <p>{description}</p>
                                                        </div>
                                                    }).collect_view()}
                                                </div>
                                            }.into_view()
                                        }}
                                    </div>

                                    <div class="relationship-group">
                                        <strong>"Character links"</strong>
                                        {if character_relationships.is_empty() {
                                            view! { <p class="muted">"No character-to-character relationships yet."</p> }.into_view()
                                        } else {
                                            view! {
                                                <div class="inspector-list">
                                                    {character_relationships.into_iter().map(|description| view! {
                                                        <div class="inspector-row">
                                                            <p>{description}</p>
                                                        </div>
                                                    }).collect_view()}
                                                </div>
                                            }.into_view()
                                        }}
                                    </div>

                                    <div class="relationship-group">
                                        <strong>"Narrative projections"</strong>
                                        {if projection_relationships.is_empty() {
                                            view! { <p class="muted">"No narrative-to-ontology projections yet."</p> }.into_view()
                                        } else {
                                            view! {
                                                <div class="inspector-list">
                                                    {projection_relationships.into_iter().map(|description| view! {
                                                        <div class="inspector-row">
                                                            <p>{description}</p>
                                                        </div>
                                                    }).collect_view()}
                                                </div>
                                            }.into_view()
                                        }}
                                    </div>
                                </div>
                            </section>

                            <section class="visual-section">
                                <span class="eyebrow">"Sync activity"</span>
                                {render_sync_debug(sync_debug.get())}
                            </section>
                        </div>
                    }.into_view()
                }
            }}
        </section>
    }
}

fn render_working_memory(
    memory: Option<Result<WorkingMemoryDto, String>>,
) -> impl IntoView {
    match memory {
        None => view! { <p class="muted">"Loading assistant state..."</p> }.into_view(),
        Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
        Some(Ok(memory)) => {
            let focus = memory
                .current_thread
                .current_focus
                .as_ref()
                .map(|focus| focus.summary.clone());
            let questions = memory
                .current_thread
                .open_questions
                .iter()
                .map(|question| question.question.clone())
                .collect::<Vec<_>>();
            let decisions = memory
                .pinned_decisions
                .iter()
                .map(|decision| decision.summary.clone())
                .collect::<Vec<_>>();
            let sidequests = memory
                .sidequests
                .iter()
                .map(|thread| thread.goal.clone())
                .collect::<Vec<_>>();
            let return_thread = memory.return_thread.as_ref().map(|thread| thread.goal.clone());
            let last_action = memory.last_tool_actions.first().cloned();

            view! {
                <div class="inspector-list">
                    <div class="inspector-row">
                        <strong>"Current thread"</strong>
                        <p>{memory.current_thread.goal}</p>
                    </div>
                    <div class="inspector-row">
                        <strong>"Current focus"</strong>
                        <p>{focus.unwrap_or_else(|| "No active focus yet.".to_string())}</p>
                    </div>
                    <div class="inspector-row">
                        <strong>"Thread scope"</strong>
                        <p>{thread_scope_label(&memory.current_thread.scope)}</p>
                    </div>
                    <div class="inspector-row">
                        <strong>"Return thread"</strong>
                        <p>{return_thread.unwrap_or_else(|| "No return thread.".to_string())}</p>
                    </div>
                    <div class="inspector-row">
                        <strong>"Open questions"</strong>
                        {if questions.is_empty() {
                            view! { <p class="muted">"No open questions tracked."</p> }.into_view()
                        } else {
                            view! {
                                <ul class="inspector-inline-list">
                                    {questions.into_iter().map(|question| view! { <li>{question}</li> }).collect_view()}
                                </ul>
                            }.into_view()
                        }}
                    </div>
                    <div class="inspector-row">
                        <strong>"Pinned decisions"</strong>
                        {if decisions.is_empty() {
                            view! { <p class="muted">"No pinned decisions yet."</p> }.into_view()
                        } else {
                            view! {
                                <ul class="inspector-inline-list">
                                    {decisions.into_iter().map(|decision| view! { <li>{decision}</li> }).collect_view()}
                                </ul>
                            }.into_view()
                        }}
                    </div>
                    <div class="inspector-row">
                        <strong>"Sidequests"</strong>
                        {if sidequests.is_empty() {
                            view! { <p class="muted">"No parked sidequests."</p> }.into_view()
                        } else {
                            view! {
                                <ul class="inspector-inline-list">
                                    {sidequests.into_iter().map(|goal| view! { <li>{goal}</li> }).collect_view()}
                                </ul>
                            }.into_view()
                        }}
                    </div>
                    <div class="inspector-row">
                        <strong>"Last tool action"</strong>
                        <p>
                            {last_action
                                .map(|action| format!("{}: {}", action.tool_name, action.summary))
                                .unwrap_or_else(|| "No tool action recorded yet.".to_string())}
                        </p>
                    </div>
                </div>
            }
            .into_view()
        }
    }
}

fn thread_scope_label(scope: &ThreadScopeDto) -> &'static str {
    match scope {
        ThreadScopeDto::Main => "Main thread",
        ThreadScopeDto::Sidequest => "Sidequest",
    }
}

fn render_sync_debug(
    debug: Option<Result<SyncDebugDto, String>>,
) -> impl IntoView {
    match debug {
        None => view! { <p class="muted">"Loading sync activity..."</p> }.into_view(),
        Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
        Some(Ok(debug)) => {
            let recent_run = debug.runs.first().cloned();
            let pending = debug.pending_candidates;
            let provenance = debug.recent_provenance;

            view! {
                <div class="inspector-list">
                    <div class="inspector-row">
                        <strong>"Latest run"</strong>
                        <p>
                            {recent_run
                                .map(|run| format!("{:?} · {:?} · {}", run.source_kind, run.status, run.created_at.format("%b %d, %H:%M")))
                                .unwrap_or_else(|| "No sync runs recorded yet.".to_string())}
                        </p>
                    </div>
                    <div class="inspector-row">
                        <strong>"Pending candidates"</strong>
                        <p>{if pending.is_empty() { "0".to_string() } else { pending.len().to_string() }}</p>
                    </div>
                    <div class="inspector-row">
                        <strong>"Recent provenance"</strong>
                        {if provenance.is_empty() {
                            view! { <p class="muted">"No provenance records yet."</p> }.into_view()
                        } else {
                            view! {
                                <ul class="inspector-inline-list">
                                    {provenance.into_iter().take(5).map(|record| view! {
                                        <li>{format!("{} -> {}", record.derived_kind, record.derived_ref)}</li>
                                    }).collect_view()}
                                </ul>
                            }.into_view()
                        }}
                    </div>
                </div>
            }
            .into_view()
        }
    }
}

fn describe_participant_relationships(snapshot: &NarrativeSnapshotDto) -> Vec<String> {
    snapshot
        .ontology_graph
        .relationships
        .iter()
        .filter(|relationship| {
            matches!(
                relationship.kind,
                OntologyRelationshipKindDto::ParticipantInEvent
            )
        })
        .map(|relationship| {
            let source = entity_label(snapshot, relationship.source_id);
            let target = entity_label(snapshot, relationship.target_id);
            format!("{source} participates in {target}")
        })
        .collect()
}

fn describe_projection_relationships(snapshot: &NarrativeSnapshotDto) -> Vec<String> {
    snapshot
        .projection_relationships
        .iter()
        .map(|relationship| {
            let narrative_label = narrative_label(snapshot, relationship.source_id);
            let ontology_label = entity_label(snapshot, relationship.target_id);
            format!("{narrative_label} projects to {ontology_label}")
        })
        .collect()
}

fn describe_character_relationships(snapshot: &NarrativeSnapshotDto) -> Vec<String> {
    snapshot
        .ontology_graph
        .relationships
        .iter()
        .filter_map(|relationship| {
            let source = entity_label(snapshot, relationship.source_id);
            let target = entity_label(snapshot, relationship.target_id);

            let description = match relationship.kind {
                OntologyRelationshipKindDto::SupportsCharacter => {
                    Some(format!("{source} supports {target}"))
                }
                OntologyRelationshipKindDto::OpposesCharacter => {
                    Some(format!("{source} opposes {target}"))
                }
                OntologyRelationshipKindDto::AdvisesCharacter => {
                    Some(format!("{source} advises {target}"))
                }
                _ => None,
            }?;

            Some(description)
        })
        .collect()
}

fn entity_label(snapshot: &NarrativeSnapshotDto, id: uuid::Uuid) -> String {
    snapshot
        .ontology_graph
        .entities
        .iter()
        .find(|entity| entity.id == id)
        .map(|entity| entity.label.clone())
        .unwrap_or_else(|| id.to_string())
}

fn narrative_label(snapshot: &NarrativeSnapshotDto, id: uuid::Uuid) -> String {
    snapshot
        .characters
        .iter()
        .find(|character| character.id == id)
        .map(|character| character.name.clone())
        .or_else(|| {
            snapshot
                .events
                .iter()
                .find(|event| event.id == id)
                .map(|event| event.title.clone())
        })
        .unwrap_or_else(|| id.to_string())
}
