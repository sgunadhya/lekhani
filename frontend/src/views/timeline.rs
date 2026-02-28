use crate::api::dto::{NarrativeSnapshotDto, OntologyRelationshipKindDto};
use crate::state::narrative::create_snapshot_resource;
use leptos::*;

#[component]
pub fn TimelineView() -> impl IntoView {
    let (snapshot_nonce, set_snapshot_nonce) = create_signal(0_u64);
    let snapshot = create_snapshot_resource(snapshot_nonce);

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
                        </div>
                    }.into_view()
                }
            }}
        </section>
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
