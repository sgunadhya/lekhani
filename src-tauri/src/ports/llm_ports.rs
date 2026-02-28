use crate::domain::{NarrativeCharacter, NarrativeEvent, NarrativeNudge, NarrativeSnapshot};

pub trait CharacterParser: Send + Sync {
    fn parse_character(
        &self,
        description: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeCharacter, String>;
}

pub trait EventParser: Send + Sync {
    fn parse_event(
        &self,
        description: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeEvent, String>;
}

pub trait NudgeGenerator: Send + Sync {
    fn generate_nudge(&self, snapshot: &NarrativeSnapshot) -> Result<NarrativeNudge, String>;
}

impl<T: CharacterParser + ?Sized> CharacterParser for Box<T> {
    fn parse_character(
        &self,
        description: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeCharacter, String> {
        (**self).parse_character(description, snapshot)
    }
}

impl<T: EventParser + ?Sized> EventParser for Box<T> {
    fn parse_event(
        &self,
        description: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeEvent, String> {
        (**self).parse_event(description, snapshot)
    }
}

impl<T: NudgeGenerator + ?Sized> NudgeGenerator for Box<T> {
    fn generate_nudge(&self, snapshot: &NarrativeSnapshot) -> Result<NarrativeNudge, String> {
        (**self).generate_nudge(snapshot)
    }
}
