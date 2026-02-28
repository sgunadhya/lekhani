use std::sync::Mutex;

use crate::domain::{
    AppError, NarrativeCharacter, NarrativeEvent, NarrativeSnapshot, OntologyEntity,
    OntologyEntityKind, OntologyRelationship, OntologyRelationshipKind,
};
use crate::ports::NarrativeRepository;
use uuid::Uuid;

#[derive(Default)]
pub struct MemoryNarrativeRepository {
    snapshot: Mutex<NarrativeSnapshot>,
}

impl NarrativeRepository for MemoryNarrativeRepository {
    fn save_character(&self, character: NarrativeCharacter) -> Result<NarrativeCharacter, AppError> {
        let mut snapshot = self
            .snapshot
            .lock()
            .map_err(|_| AppError::StatePoisoned("narrative store lock poisoned"))?;

        let ontology_entity_id = character.ontology_entity_id.unwrap_or_else(Uuid::new_v4);
        let projection_id = Uuid::new_v4();
        let character = NarrativeCharacter {
            ontology_entity_id: Some(ontology_entity_id),
            ..character
        };

        snapshot.characters.retain(|existing| existing.id != character.id);
        snapshot
            .ontology_graph
            .entities
            .retain(|entity| entity.id != ontology_entity_id);
        snapshot
            .projection_relationships
            .retain(|relationship| relationship.source_id != character.id);

        snapshot.ontology_graph.entities.push(OntologyEntity {
            id: ontology_entity_id,
            kind: OntologyEntityKind::Character,
            label: character.name.clone(),
            summary: character.summary.clone(),
        });
        snapshot.projection_relationships.push(OntologyRelationship {
            id: projection_id,
            source_id: character.id,
            target_id: ontology_entity_id,
            kind: OntologyRelationshipKind::NarrativeProjection,
            summary: "Narrative character projects to ontology character node".to_string(),
        });
        snapshot.characters.push(character.clone());
        snapshot.metrics.character_count = snapshot.characters.len();

        Ok(character)
    }

    fn save_event(&self, event: NarrativeEvent) -> Result<NarrativeEvent, AppError> {
        let mut snapshot = self
            .snapshot
            .lock()
            .map_err(|_| AppError::StatePoisoned("narrative store lock poisoned"))?;

        let ontology_entity_id = event.ontology_entity_id.unwrap_or_else(Uuid::new_v4);
        let projection_id = Uuid::new_v4();
        let event = NarrativeEvent {
            ontology_entity_id: Some(ontology_entity_id),
            ..event
        };

        snapshot.events.retain(|existing| existing.id != event.id);
        snapshot
            .ontology_graph
            .entities
            .retain(|entity| entity.id != ontology_entity_id);
        snapshot
            .ontology_graph
            .relationships
            .retain(|relationship| {
                relationship.target_id != ontology_entity_id
                    || !matches!(
                        relationship.kind,
                        OntologyRelationshipKind::ParticipantInEvent
                    )
            });
        snapshot
            .projection_relationships
            .retain(|relationship| relationship.source_id != event.id);

        snapshot.ontology_graph.entities.push(OntologyEntity {
            id: ontology_entity_id,
            kind: OntologyEntityKind::Event,
            label: event.title.clone(),
            summary: event.summary.clone(),
        });
        snapshot.projection_relationships.push(OntologyRelationship {
            id: projection_id,
            source_id: event.id,
            target_id: ontology_entity_id,
            kind: OntologyRelationshipKind::NarrativeProjection,
            summary: "Narrative event projects to ontology event node".to_string(),
        });

        for participant_id in &event.participants {
            if let Some(character_ontology_id) = snapshot
                .characters
                .iter()
                .find(|character| character.id == *participant_id)
                .and_then(|character| character.ontology_entity_id)
            {
                snapshot.ontology_graph.relationships.push(OntologyRelationship {
                    id: Uuid::new_v4(),
                    source_id: character_ontology_id,
                    target_id: ontology_entity_id,
                    kind: OntologyRelationshipKind::ParticipantInEvent,
                    summary: "Character participates in event".to_string(),
                });
            }
        }

        snapshot.events.push(event.clone());
        snapshot.metrics.event_count = snapshot.events.len();

        Ok(event)
    }

    fn save_relationship(
        &self,
        relationship: OntologyRelationship,
    ) -> Result<OntologyRelationship, AppError> {
        let mut snapshot = self
            .snapshot
            .lock()
            .map_err(|_| AppError::StatePoisoned("narrative store lock poisoned"))?;

        snapshot
            .ontology_graph
            .relationships
            .retain(|existing| existing.id != relationship.id);
        snapshot.ontology_graph.relationships.push(relationship.clone());

        Ok(relationship)
    }

    fn load_snapshot(&self) -> Result<NarrativeSnapshot, AppError> {
        self.snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .map_err(|_| AppError::StatePoisoned("narrative store lock poisoned"))
    }
}
