use crate::adapters::mcp::{McpToolCall, McpToolResult};
use crate::domain::{
    AssistantIntent, NarrativeCharacter, NarrativeEvent, NarrativeNudge, NarrativeSnapshot,
    WorkingMemory,
};

#[derive(Debug, Clone)]
pub struct AssistantToolCall {
    pub call: McpToolCall,
    pub thought: String,
}

pub enum AssistantResponse {
    ToolCalls(Vec<AssistantToolCall>),
    FinalReply {
        intent: AssistantIntent,
        title: String,
        body: String,
    },
}

pub trait AssistantAgent: Send + Sync {
    fn process_turn(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
        observations: &[(McpToolCall, McpToolResult)],
    ) -> Result<AssistantResponse, String>;

    fn generate_nudge(
        &self,
        snapshot: &NarrativeSnapshot,
        memory: &WorkingMemory,
    ) -> Result<NarrativeNudge, String>;
}

impl<T: AssistantAgent + ?Sized> AssistantAgent for Box<T> {
    fn process_turn(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
        observations: &[(McpToolCall, McpToolResult)],
    ) -> Result<AssistantResponse, String> {
        (**self).process_turn(prompt, memory, observations)
    }

    fn generate_nudge(
        &self,
        snapshot: &NarrativeSnapshot,
        memory: &WorkingMemory,
    ) -> Result<NarrativeNudge, String> {
        (**self).generate_nudge(snapshot, memory)
    }
}

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
