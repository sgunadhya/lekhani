use crate::application::{DialogueActContext, TurnInterpretation};
use crate::domain::{NarrativeCharacter, NarrativeEvent, NarrativeSnapshot, WorkingMemory};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FollowUpDirective {
    ElaborateCurrent,
    AlternativeOption,
    ConfirmCurrent,
    RejectCurrent,
    ShiftToCharacter,
    ShiftToEvent,
    AddToScreenplay,
    Unknown,
}

pub enum AssistantResponse {
    FinalReply {
        title: String,
        focus_summary: Option<String>,
        body: String,
    },
}

pub trait AssistantAgent: Send + Sync {
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

pub trait NarrativeProvider: AssistantAgent + Send + Sync {
    fn classify_dialogue_act(&self, context: DialogueActContext<'_>) -> TurnInterpretation;
}

impl<T: AssistantAgent + ?Sized> AssistantAgent for Box<T> {
    fn interpret_followup(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<FollowUpDirective, String> {
        (**self).interpret_followup(prompt, memory)
    }

    fn elaborate_focus(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        (**self).elaborate_focus(prompt, memory)
    }

    fn suggest_alternative(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        (**self).suggest_alternative(prompt, memory)
    }

    fn brainstorm_topic(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        (**self).brainstorm_topic(prompt, memory)
    }

    fn respond_in_context(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        (**self).respond_in_context(prompt, memory)
    }

    fn draft_from_focus(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<AssistantResponse, String> {
        (**self).draft_from_focus(prompt, memory)
    }

}

impl<T: NarrativeProvider + ?Sized> NarrativeProvider for Box<T> {
    fn classify_dialogue_act(&self, context: DialogueActContext<'_>) -> TurnInterpretation {
        (**self).classify_dialogue_act(context)
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
