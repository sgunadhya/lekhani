use crate::domain::{NarrativeCharacter, NarrativeEvent, NarrativeNudge};

pub trait CharacterParser: Send + Sync {
    fn parse_character(&self, description: &str) -> Result<NarrativeCharacter, String>;
}

pub trait EventParser: Send + Sync {
    fn parse_event(&self, description: &str) -> Result<NarrativeEvent, String>;
}

pub trait NudgeGenerator: Send + Sync {
    fn generate_nudge(&self) -> Result<NarrativeNudge, String>;
}

impl<T: CharacterParser + ?Sized> CharacterParser for Box<T> {
    fn parse_character(&self, description: &str) -> Result<NarrativeCharacter, String> {
        (**self).parse_character(description)
    }
}

impl<T: EventParser + ?Sized> EventParser for Box<T> {
    fn parse_event(&self, description: &str) -> Result<NarrativeEvent, String> {
        (**self).parse_event(description)
    }
}

impl<T: NudgeGenerator + ?Sized> NudgeGenerator for Box<T> {
    fn generate_nudge(&self) -> Result<NarrativeNudge, String> {
        (**self).generate_nudge()
    }
}
