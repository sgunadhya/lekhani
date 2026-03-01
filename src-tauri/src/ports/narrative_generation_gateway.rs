use crate::domain::{NarrativeMessagePreview, NarrativeSnapshot, WorkingMemory};
use crate::ports::{AssistantResponse, FollowUpDirective};

pub trait NarrativeGenerationGateway: Send + Sync {
    fn preview_message(
        &self,
        prompt: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<NarrativeMessagePreview, String>;

    fn interpret_followup(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<FollowUpDirective, String>;

    fn elaborate_focus(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String>;

    fn suggest_alternative(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String>;

    fn brainstorm_topic(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String>;

    fn respond_in_context(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String>;

    fn draft_from_focus(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String>;
}
