use crate::api::dto::{NarrativeSnapshotDto, OntologyRelationshipKindDto};
use crate::state::narrative::create_snapshot_resource;
use leptos::*;

#[component]
pub fn TimelineView() -> impl IntoView {
    let (snapshot_nonce, set_snapshot_nonce) = create_signal(0_u64);
    let snapshot = create_snapshot_resource(snapshot_nonce);
    let visual_tabs = ["Timeline", "Characters", "Metrics"];

    view! {
        <section class="visual-mode">
            <div class="mode-header">
                <span class="eyebrow">"Visual Mode"</span>
                <h2>"Browse derived narrative views"</h2>
                <p>"Inspect timelines, character profiles, and story metrics. Use the assistant to make changes rather than editing data structures directly."</p>
            </div>

            <div class="visual-layout">
                <aside class="visual-nav">
                    <button class="secondary-button" on:click=move |_| set_snapshot_nonce.update(|value| *value += 1)>
                        "Refresh Derived Data"
                    </button>
                    {visual_tabs.into_iter().map(|tab| view! {
                        <button class="visual-tab">{tab}</button>
                    }).collect_view()}
                </aside>

                <div class="visual-content">
                    <div class="visual-panel">
                        <h3>"Timeline"</h3>
                        {move || match snapshot.get() {
                            None => view! { <p>"Loading timeline..."</p> }.into_view(),
                            Some(Ok(snapshot)) if snapshot.events.is_empty() => view! {
                                <p>"No narrative events parsed yet. Use Narrative mode to describe story events."</p>
                            }.into_view(),
                            Some(Ok(snapshot)) => view! {
                                <ul class="screenplay-list">
                                    {snapshot.events.iter().map(|event| view! {
                                        <li class="screenplay-list-item" data-id=event.id.to_string()>
                                            <span class="screenplay-bullet"></span>
                                            <span>{&event.title}</span>
                                        </li>
                                    }).collect_view()}
                                </ul>
                            }.into_view(),
                            Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                        }}
                    </div>

                    <div class="visual-panel">
                        <h3>"Character Profiles"</h3>
                        {move || match snapshot.get() {
                            None => view! { <p>"Loading profiles..."</p> }.into_view(),
                            Some(Ok(snapshot)) if snapshot.characters.is_empty() => view! {
                                <p>"No characters parsed yet. Use Narrative mode to define the cast."</p>
                            }.into_view(),
                            Some(Ok(snapshot)) => view! {
                                <ul class="screenplay-list">
                                    {snapshot.characters.iter().map(|character| view! {
                                        <li class="screenplay-list-item" data-id=character.id.to_string()>
                                            <span class="screenplay-bullet"></span>
                                            <span>{&character.name}</span>
                                        </li>
                                    }).collect_view()}
                                </ul>
                            }.into_view(),
                            Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                        }}
                    </div>

                    <div class="visual-panel">
                        <h3>"Current Metrics"</h3>
                        {move || match snapshot.get() {
                            None => view! { <p>"Calculating..."</p> }.into_view(),
                            Some(Ok(snapshot)) => view! {
                                <div class="metrics-grid">
                                    <div class="metric-card">
                                        <span class="metric-label">"Scenes"</span>
                                        <strong>{snapshot.metrics.scene_count}</strong>
                                    </div>
                                    <div class="metric-card">
                                        <span class="metric-label">"Tracked Characters"</span>
                                        <strong>{snapshot.metrics.character_count}</strong>
                                    </div>
                                    <div class="metric-card">
                                        <span class="metric-label">"Tracked Events"</span>
                                        <strong>{snapshot.metrics.event_count}</strong>
                                    </div>
                                    <div class="metric-card">
                                        <span class="metric-label">"Model Entities"</span>
                                        <strong>{snapshot.ontology_graph.entities.len()}</strong>
                                    </div>
                                    <div class="metric-card">
                                        <span class="metric-label">"Model Relationships"</span>
                                        <strong>{snapshot.ontology_graph.relationships.len()}</strong>
                                    </div>
                                    <div class="metric-card">
                                        <span class="metric-label">"Projection Links"</span>
                                        <strong>{snapshot.projection_relationships.len()}</strong>
                                    </div>
                                </div>
                            }.into_view(),
                            Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                        }}
                    </div>

                    <div class="visual-panel">
                        <h3>"Ontology Relationships"</h3>
                        {move || match snapshot.get() {
                            None => view! { <p>"Loading relationship graph..."</p> }.into_view(),
                            Some(Ok(snapshot)) => {
                                let participant_relationships = describe_participant_relationships(&snapshot);
                                let projection_relationships = describe_projection_relationships(&snapshot);

                                view! {
                                    <div class="relationship-groups">
                                        <div class="parsed-block">
                                            <span class="eyebrow">"Participants"</span>
                                            {if participant_relationships.is_empty() {
                                                view! { <p>"No event participation links yet."</p> }.into_view()
                                            } else {
                                                view! {
                                                    <ul class="screenplay-list">
                                                        {participant_relationships.into_iter().map(|description| view! {
                                                            <li class="screenplay-list-item">
                                                                <span class="screenplay-bullet"></span>
                                                                <span>{description}</span>
                                                            </li>
                                                        }).collect_view()}
                                                    </ul>
                                                }.into_view()
                                            }}
                                        </div>

                                        <div class="parsed-block">
                                            <span class="eyebrow">"Narrative Projections"</span>
                                            {if projection_relationships.is_empty() {
                                                view! { <p>"No narrative-to-ontology projections yet."</p> }.into_view()
                                            } else {
                                                view! {
                                                    <ul class="screenplay-list">
                                                        {projection_relationships.into_iter().map(|description| view! {
                                                            <li class="screenplay-list-item">
                                                                <span class="screenplay-bullet"></span>
                                                                <span>{description}</span>
                                                            </li>
                                                        }).collect_view()}
                                                    </ul>
                                                }.into_view()
                                            }}
                                        </div>
                                    </div>
                                }.into_view()
                            }
                            Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                        }}
                    </div>
                </div>
            </div>
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
