use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{OntologyGraph, OntologyRelationship};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeCharacter {
    pub id: Uuid,
    pub ontology_entity_id: Option<Uuid>,
    pub name: String,
    pub summary: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeEvent {
    pub id: Uuid,
    pub ontology_entity_id: Option<Uuid>,
    pub title: String,
    pub summary: String,
    pub participants: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NarrativeCommitTarget {
    Character,
    Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NarrativeChangeKind {
    AddCharacter,
    UpdateCharacter,
    AddEvent,
    UpdateEvent,
    AddRelationship,
    UpdateRelationship,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeChangeSummary {
    pub kind: NarrativeChangeKind,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeMessagePreview {
    pub prompt: String,
    pub suggested_target: NarrativeCommitTarget,
    pub character: Option<NarrativeCharacter>,
    pub event: Option<NarrativeEvent>,
    pub relationships: Vec<OntologyRelationship>,
    pub changes: Vec<NarrativeChangeSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NarrativeMetrics {
    pub scene_count: usize,
    pub character_count: usize,
    pub event_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NarrativeSnapshot {
    pub characters: Vec<NarrativeCharacter>,
    pub events: Vec<NarrativeEvent>,
    pub projection_relationships: Vec<OntologyRelationship>,
    pub ontology_graph: OntologyGraph,
    pub metrics: NarrativeMetrics,
}
