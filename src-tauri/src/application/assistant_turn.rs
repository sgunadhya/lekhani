use crate::domain::{
    AssistantCapability, AssistantIntent, NarrativeMessagePreview, WorkingMemory, WritePolicy,
};
use crate::ports::AssistantResponse;

pub struct AssistantIntentContext<'a> {
    pub prompt: &'a str,
    pub preview: &'a NarrativeMessagePreview,
}

pub trait AssistantIntentClassifier: Send + Sync {
    fn classify(&self, context: AssistantIntentContext<'_>) -> AssistantIntent;
}

pub trait MutationGate: Send + Sync {
    fn allow_mutation(&self, context: AssistantIntentContext<'_>) -> bool;
}

pub struct CapabilityPlanningContext<'a> {
    pub prompt: &'a str,
    pub preview: &'a NarrativeMessagePreview,
    pub intent: AssistantIntent,
    pub mutation_allowed: bool,
}

pub struct CapabilityPlan {
    pub intent: AssistantIntent,
    pub capabilities: Vec<AssistantCapability>,
    pub write_policy: WritePolicy,
}

pub trait AssistantCapabilityPlanner: Send + Sync {
    fn plan(&self, context: CapabilityPlanningContext<'_>) -> CapabilityPlan;
}

pub trait AssistantFallbackResponder: Send + Sync {
    fn respond(
        &self,
        prompt: &str,
        preview: &NarrativeMessagePreview,
        memory: &WorkingMemory,
        plan: &CapabilityPlan,
    ) -> AssistantResponse;
}

pub struct HeuristicAssistantIntentClassifier;
pub struct HeuristicMutationGate;
pub struct HeuristicAssistantCapabilityPlanner;
pub struct HeuristicAssistantFallbackResponder;

impl AssistantIntentClassifier for HeuristicAssistantIntentClassifier {
    fn classify(&self, context: AssistantIntentContext<'_>) -> AssistantIntent {
        let prompt = context.prompt.trim();

        if prompt.ends_with('?') {
            AssistantIntent::Query
        } else if context.preview.changes.is_empty() {
            AssistantIntent::Guide
        } else {
            AssistantIntent::MutateOntology
        }
    }
}

impl MutationGate for HeuristicMutationGate {
    fn allow_mutation(&self, context: AssistantIntentContext<'_>) -> bool {
        if context.preview.changes.is_empty() {
            return false;
        }

        let prompt_tokens = meaningful_tokens(context.prompt);
        if prompt_tokens.is_empty() {
            return false;
        }

        context
            .preview
            .changes
            .iter()
            .any(|change| is_grounded_label(&change.label, &prompt_tokens))
    }
}

impl AssistantCapabilityPlanner for HeuristicAssistantCapabilityPlanner {
    fn plan(&self, context: CapabilityPlanningContext<'_>) -> CapabilityPlan {
        let _ = context.prompt;

        let mut capabilities = vec![AssistantCapability::UnderstandTurn];

        match context.intent {
            AssistantIntent::Query => {
                capabilities.push(AssistantCapability::InspectProjectState);
                capabilities.push(AssistantCapability::GuideNextStep);
                CapabilityPlan {
                    intent: AssistantIntent::Query,
                    capabilities,
                    write_policy: WritePolicy::NoWrite,
                }
            }
            AssistantIntent::Guide => {
                if !context.preview.changes.is_empty() {
                    capabilities.push(AssistantCapability::ExtractStructure);
                }
                capabilities.push(AssistantCapability::GuideNextStep);
                CapabilityPlan {
                    intent: AssistantIntent::Guide,
                    capabilities,
                    write_policy: WritePolicy::NoWrite,
                }
            }
            AssistantIntent::Clarify => {
                if !context.preview.changes.is_empty() {
                    capabilities.push(AssistantCapability::ExtractStructure);
                }
                capabilities.push(AssistantCapability::ResolveAmbiguity);
                capabilities.push(AssistantCapability::GuideNextStep);
                CapabilityPlan {
                    intent: AssistantIntent::Clarify,
                    capabilities,
                    write_policy: WritePolicy::NoWrite,
                }
            }
            AssistantIntent::MutateOntology if context.mutation_allowed => {
                capabilities.push(AssistantCapability::InspectProjectState);
                capabilities.push(AssistantCapability::ExtractStructure);
                capabilities.push(AssistantCapability::CommitStructure);
                capabilities.push(AssistantCapability::GuideNextStep);
                CapabilityPlan {
                    intent: AssistantIntent::MutateOntology,
                    capabilities,
                    write_policy: WritePolicy::SafeCommit,
                }
            }
            AssistantIntent::MutateOntology => {
                capabilities.push(AssistantCapability::InspectProjectState);
                capabilities.push(AssistantCapability::ExtractStructure);
                capabilities.push(AssistantCapability::ResolveAmbiguity);
                capabilities.push(AssistantCapability::GuideNextStep);
                CapabilityPlan {
                    intent: AssistantIntent::Clarify,
                    capabilities,
                    write_policy: WritePolicy::NoWrite,
                }
            }
            AssistantIntent::MutateDocument => {
                capabilities.push(AssistantCapability::InspectProjectState);
                capabilities.push(AssistantCapability::ProposeDocumentChange);
                CapabilityPlan {
                    intent: AssistantIntent::MutateDocument,
                    capabilities,
                    write_policy: WritePolicy::CandidateOnly,
                }
            }
            AssistantIntent::ProposeSync => {
                capabilities.push(AssistantCapability::InspectAlignment);
                capabilities.push(AssistantCapability::ResolveAmbiguity);
                CapabilityPlan {
                    intent: AssistantIntent::ProposeSync,
                    capabilities,
                    write_policy: WritePolicy::CandidateOnly,
                }
            }
            AssistantIntent::ResolveLint => {
                capabilities.push(AssistantCapability::InspectAlignment);
                capabilities.push(AssistantCapability::ResolveLint);
                CapabilityPlan {
                    intent: AssistantIntent::ResolveLint,
                    capabilities,
                    write_policy: WritePolicy::CandidateOnly,
                }
            }
        }
    }
}

impl AssistantFallbackResponder for HeuristicAssistantFallbackResponder {
    fn respond(
        &self,
        prompt: &str,
        preview: &NarrativeMessagePreview,
        memory: &WorkingMemory,
        plan: &CapabilityPlan,
    ) -> AssistantResponse {
        let title = match plan.intent {
            AssistantIntent::Query => "Story Notes",
            AssistantIntent::Clarify => "Need One Anchor",
            AssistantIntent::MutateDocument => "Draft Suggestion",
            AssistantIntent::ProposeSync => "Alignment Note",
            AssistantIntent::ResolveLint => "Structure Check",
            _ => "Story Direction",
        }
        .to_string();

        let body = if let Some(reply_body) = preview.reply_body.as_ref().filter(|value| !value.trim().is_empty()) {
            reply_body.clone()
        } else if !preview.changes.is_empty() {
            let focus = preview
                .changes
                .iter()
                .take(2)
                .map(|change| format!("{}: {}", change.label, change.detail))
                .collect::<Vec<_>>()
                .join("\n");

            match plan.write_policy {
                WritePolicy::CandidateOnly => format!(
                    "I found a plausible story move, but I kept it as a proposal for now.\n{}",
                    focus
                ),
                WritePolicy::NoWrite => format!(
                    "I can work with this, but I need a bit more grounding before I treat it as story structure.\n{}",
                    focus
                ),
                WritePolicy::SafeCommit => format!(
                    "This looks grounded enough to shape the story model.\n{}",
                    focus
                ),
            }
        } else if let Some(question) = memory.open_questions.first() {
            format!(
                "I'm tracking this as the most useful next question:\n{}",
                question.question
            )
        } else if let Some(focus) = memory.current_focus.as_ref() {
            format!(
                "We're currently focused on {}. Give me one concrete detail to build from.",
                focus.summary
            )
        } else {
            let trimmed = prompt.trim();
            format!(
                "I can help shape this. Give me one concrete character, event, relationship, or setting detail to build from.\nCurrent note: {}",
                trimmed
            )
        };

        AssistantResponse::FinalReply {
            intent: plan.intent.clone(),
            title,
            body,
        }
    }
}

fn is_grounded_label(label: &str, prompt_tokens: &[String]) -> bool {
    let label_tokens = meaningful_tokens(label);
    if label_tokens.is_empty() {
        return false;
    }

    let generic_only = label_tokens.iter().all(|token| is_generic_story_token(token));
    if generic_only {
        return false;
    }

    label_tokens
        .iter()
        .filter(|token| !is_generic_story_token(token))
        .any(|token| prompt_tokens.iter().any(|prompt| prompt == token))
}

fn meaningful_tokens(value: &str) -> Vec<String> {
    value.split(|ch: char| !ch.is_alphanumeric())
        .filter_map(|token| {
            let token = token.trim().to_ascii_lowercase();
            if token.len() < 3 || is_stopword(&token) {
                None
            } else {
                Some(token)
            }
        })
        .collect()
}

fn is_stopword(token: &str) -> bool {
    matches!(
        token,
        "the"
            | "and"
            | "for"
            | "with"
            | "that"
            | "this"
            | "from"
            | "into"
            | "about"
            | "story"
            | "want"
            | "create"
            | "write"
            | "start"
            | "begin"
            | "help"
            | "make"
            | "some"
            | "idea"
            | "there"
            | "their"
            | "have"
    )
}

fn is_generic_story_token(token: &str) -> bool {
    matches!(
        token,
        "story"
            | "character"
            | "event"
            | "person"
            | "creator"
            | "documentary"
            | "scene"
            | "problem"
            | "thing"
            | "someone"
            | "somebody"
    )
}
