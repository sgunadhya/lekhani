use crate::domain::{AssistantCapability, AssistantIntent, NarrativeMessagePreview, WritePolicy};

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

pub struct HeuristicAssistantIntentClassifier;
pub struct HeuristicMutationGate;
pub struct HeuristicAssistantCapabilityPlanner;

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
