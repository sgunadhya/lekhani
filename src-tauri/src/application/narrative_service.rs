use crate::domain::{NarrativeCharacter, NarrativeEvent, NarrativeNudge};
use crate::ports::{CharacterParser, EventParser, NudgeGenerator};

pub struct NarrativeService<C, E, N> {
    character_parser: C,
    event_parser: E,
    nudge_generator: N,
}

impl<C, E, N> NarrativeService<C, E, N> {
    pub fn new(character_parser: C, event_parser: E, nudge_generator: N) -> Self {
        Self {
            character_parser,
            event_parser,
            nudge_generator,
        }
    }
}

impl<C, E, N> NarrativeService<C, E, N>
where
    C: CharacterParser,
    E: EventParser,
    N: NudgeGenerator,
{
    pub fn parse_character(&self, description: &str) -> Result<NarrativeCharacter, String> {
        self.character_parser.parse_character(description)
    }

    pub fn parse_event(&self, description: &str) -> Result<NarrativeEvent, String> {
        self.event_parser.parse_event(description)
    }

    pub fn get_nudge(&self) -> Result<NarrativeNudge, String> {
        self.nudge_generator.generate_nudge()
    }
}
