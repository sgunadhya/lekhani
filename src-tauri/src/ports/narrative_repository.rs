use crate::domain::{
    AppError, NarrativeCharacter, NarrativeEvent, NarrativeSnapshot, OntologyEntity,
    OntologyRelationship,
};
use std::sync::Arc;

pub trait NarrativeRepository: Send + Sync {
    fn save_character(&self, character: NarrativeCharacter) -> Result<NarrativeCharacter, AppError>;
    fn save_event(&self, event: NarrativeEvent) -> Result<NarrativeEvent, AppError>;
    fn save_entity(&self, entity: OntologyEntity) -> Result<OntologyEntity, AppError>;
    fn save_relationship(
        &self,
        relationship: OntologyRelationship,
    ) -> Result<OntologyRelationship, AppError>;
    fn load_snapshot(&self) -> Result<NarrativeSnapshot, AppError>;
    fn clear_all(&self) -> Result<(), AppError>;
}

impl<T: NarrativeRepository + ?Sized> NarrativeRepository for Box<T> {
    fn save_character(&self, character: NarrativeCharacter) -> Result<NarrativeCharacter, AppError> {
        (**self).save_character(character)
    }

    fn save_event(&self, event: NarrativeEvent) -> Result<NarrativeEvent, AppError> {
        (**self).save_event(event)
    }

    fn save_entity(&self, entity: OntologyEntity) -> Result<OntologyEntity, AppError> {
        (**self).save_entity(entity)
    }

    fn save_relationship(
        &self,
        relationship: OntologyRelationship,
    ) -> Result<OntologyRelationship, AppError> {
        (**self).save_relationship(relationship)
    }

    fn load_snapshot(&self) -> Result<NarrativeSnapshot, AppError> {
        (**self).load_snapshot()
    }

    fn clear_all(&self) -> Result<(), AppError> {
        (**self).clear_all()
    }
}

impl<T: NarrativeRepository + ?Sized> NarrativeRepository for Arc<T> {
    fn save_character(&self, character: NarrativeCharacter) -> Result<NarrativeCharacter, AppError> {
        (**self).save_character(character)
    }

    fn save_event(&self, event: NarrativeEvent) -> Result<NarrativeEvent, AppError> {
        (**self).save_event(event)
    }

    fn save_entity(&self, entity: OntologyEntity) -> Result<OntologyEntity, AppError> {
        (**self).save_entity(entity)
    }

    fn save_relationship(
        &self,
        relationship: OntologyRelationship,
    ) -> Result<OntologyRelationship, AppError> {
        (**self).save_relationship(relationship)
    }

    fn load_snapshot(&self) -> Result<NarrativeSnapshot, AppError> {
        (**self).load_snapshot()
    }

    fn clear_all(&self) -> Result<(), AppError> {
        (**self).clear_all()
    }
}
