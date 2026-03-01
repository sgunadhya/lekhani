use serde::{Deserialize, Serialize};

use crate::domain::{NarrativeSnapshot, WorkingMemory};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorySnapshot {
    pub screenplay_title: String,
    pub fountain_text: String,
    pub narrative: NarrativeSnapshot,
    pub working_memory: WorkingMemory,
}
