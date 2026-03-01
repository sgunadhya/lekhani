use crate::domain::{NarrativeSnapshot, StorySnapshot, WorkingMemory};

pub trait QueryGateway: Send + Sync {
    fn load_working_memory(&self) -> Result<WorkingMemory, String>;
    fn load_narrative_snapshot(&self) -> Result<NarrativeSnapshot, String>;
    fn load_story_snapshot(&self) -> Result<StorySnapshot, String>;
}
