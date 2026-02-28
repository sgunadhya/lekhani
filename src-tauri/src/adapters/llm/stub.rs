use crate::domain::{NarrativeCharacter, NarrativeEvent, NarrativeNudge};
use crate::ports::{CharacterParser, EventParser, NudgeGenerator};
use uuid::Uuid;

#[derive(Default)]
pub struct StubNarrativeEngine;

impl CharacterParser for StubNarrativeEngine {
    fn parse_character(&self, description: &str) -> Result<NarrativeCharacter, String> {
        Ok(NarrativeCharacter {
            id: Uuid::new_v4(),
            ontology_entity_id: None,
            name: "Draft Character".to_string(),
            summary: description.trim().to_string(),
            tags: vec!["draft".to_string(), "narrative".to_string()],
        })
    }
}

impl EventParser for StubNarrativeEngine {
    fn parse_event(&self, description: &str) -> Result<NarrativeEvent, String> {
        Ok(NarrativeEvent {
            id: Uuid::new_v4(),
            ontology_entity_id: None,
            title: "Draft Event".to_string(),
            summary: description.trim().to_string(),
            participants: Vec::new(),
        })
    }
}

impl NudgeGenerator for StubNarrativeEngine {
    fn generate_nudge(&self) -> Result<NarrativeNudge, String> {
        Ok(NarrativeNudge {
            message: "Consider sharpening the central conflict in the next scene.".to_string(),
        })
    }
}
