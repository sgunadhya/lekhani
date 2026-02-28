use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeTypeDto {
    Insert,
    Delete,
    Replace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenplayChangeDto {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub author: String,
    pub change_type: ChangeTypeDto,
    pub range_start: usize,
    pub range_end: usize,
    pub new_text: String,
    pub old_text: String,
    pub provenance: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenplayDto {
    pub id: Uuid,
    pub title: String,
    pub fountain_text: String,
    pub version: u64,
    pub changes: Vec<ScreenplayChangeDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveScreenplayRequest {
    pub screenplay: ScreenplayDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentFileDto {
    pub screenplay: ScreenplayDto,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveDocumentRequest {
    pub screenplay: ScreenplayDto,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseDescriptionRequest {
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeCharacterDto {
    pub id: Uuid,
    pub ontology_entity_id: Option<Uuid>,
    pub name: String,
    pub summary: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeEventDto {
    pub id: Uuid,
    pub ontology_entity_id: Option<Uuid>,
    pub title: String,
    pub summary: String,
    pub participants: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeNudgeDto {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NarrativeMetricsDto {
    pub scene_count: usize,
    pub character_count: usize,
    pub event_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NarrativeSnapshotDto {
    pub characters: Vec<NarrativeCharacterDto>,
    pub events: Vec<NarrativeEventDto>,
    pub projection_relationships: Vec<OntologyRelationshipDto>,
    pub ontology_graph: OntologyGraphDto,
    pub metrics: NarrativeMetricsDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OntologyEntityKindDto {
    Character,
    Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyEntityDto {
    pub id: Uuid,
    pub kind: OntologyEntityKindDto,
    pub label: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OntologyRelationshipKindDto {
    NarrativeProjection,
    ParticipantInEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyRelationshipDto {
    pub id: Uuid,
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub kind: OntologyRelationshipKindDto,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OntologyGraphDto {
    pub entities: Vec<OntologyEntityDto>,
    pub relationships: Vec<OntologyRelationshipDto>,
}
