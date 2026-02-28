use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OntologyEntityKind {
    Character,
    Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyEntity {
    pub id: Uuid,
    pub kind: OntologyEntityKind,
    pub label: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OntologyRelationshipKind {
    NarrativeProjection,
    ParticipantInEvent,
    SupportsCharacter,
    OpposesCharacter,
    AdvisesCharacter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyRelationship {
    pub id: Uuid,
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub kind: OntologyRelationshipKind,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OntologyGraph {
    pub entities: Vec<OntologyEntity>,
    pub relationships: Vec<OntologyRelationship>,
}
