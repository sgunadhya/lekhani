use crate::domain::{
    AssistantCapability, AssistantIntent, Constraint, ConstraintOperator, ConstraintScope,
    ConstraintStatus, ConversationMode, ConversationTopic, FocusItem, FocusKind, NarrativeMessagePreview,
    NarrativeThreadStatus, OpenQuestion, RecentCorrection, WorkingMemory,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DialogueAct {
    Query,
    Brainstorm,
    Constraint,
    Correction,
    Confirmation,
    Commit,
    RewriteRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterpretationTarget {
    CurrentCandidate,
    NewTopic(ConversationTopic),
    Screenplay,
    General,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnRoute {
    Continue,
    ElaborateCurrent,
    AlternativeCurrent,
    ConfirmCurrent,
    RejectCurrent,
    ShiftToCharacter,
    ShiftToEvent,
    AddToScreenplay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnInterpretation {
    pub dialogue_act: DialogueAct,
    pub target: InterpretationTarget,
    pub route: TurnRoute,
    pub confidence: f32,
}

pub struct DialogueActContext<'a> {
    pub prompt: &'a str,
    pub memory: &'a WorkingMemory,
}

pub trait DialogueActClassifier: Send + Sync {
    fn classify(&self, context: DialogueActContext<'_>) -> TurnInterpretation;
}

pub struct DialogueStateContext<'a> {
    pub prompt: &'a str,
    pub preview: &'a NarrativeMessagePreview,
    pub memory: &'a WorkingMemory,
    pub interpretation: &'a TurnInterpretation,
}

pub struct DialogueStateUpdate {
    pub working_memory: WorkingMemory,
}

pub trait BeliefStateUpdater: Send + Sync {
    fn update(&self, context: DialogueStateContext<'_>) -> DialogueStateUpdate;
}

pub struct CapabilityPlanningContext<'a> {
    pub prompt: &'a str,
    pub preview: &'a NarrativeMessagePreview,
    pub interpretation: &'a TurnInterpretation,
    pub mutation_allowed: bool,
}

pub struct CapabilityPlan {
    pub intent: AssistantIntent,
    pub capabilities: Vec<AssistantCapability>,
}

pub trait AssistantCapabilityPlanner: Send + Sync {
    fn plan(&self, context: CapabilityPlanningContext<'_>) -> CapabilityPlan;
}

pub trait ResponseStateFinalizer: Send + Sync {
    fn finalize(
        &self,
        memory: WorkingMemory,
        prompt: &str,
        plan: &CapabilityPlan,
        title: &str,
        body: &str,
        focus_summary: Option<&str>,
    ) -> WorkingMemory;
}

pub trait NarrativeConversationSupport: Send + Sync {
    fn classify_dialogue_act(&self, context: DialogueActContext<'_>) -> TurnInterpretation;
    fn update_belief_state(&self, context: DialogueStateContext<'_>) -> DialogueStateUpdate;
    fn plan_capabilities(&self, context: CapabilityPlanningContext<'_>) -> CapabilityPlan;
    fn finalize_response_state(
        &self,
        memory: WorkingMemory,
        prompt: &str,
        plan: &CapabilityPlan,
        title: &str,
        body: &str,
        focus_summary: Option<&str>,
    ) -> WorkingMemory;
}

pub fn is_underspecified_followup(prompt: &str, preview: &NarrativeMessagePreview) -> bool {
    is_low_information_followup(prompt, preview)
}

pub struct HeuristicBeliefStateUpdater;
pub struct HeuristicAssistantCapabilityPlanner;
pub struct HeuristicResponseStateFinalizer;

impl BeliefStateUpdater for HeuristicBeliefStateUpdater {
    fn update(&self, context: DialogueStateContext<'_>) -> DialogueStateUpdate {
        let mut memory = context.memory.clone();
        let dialogue_act = &context.interpretation.dialogue_act;

        match dialogue_act {
            DialogueAct::Constraint | DialogueAct::Correction => {
                memory.conversation_mode = ConversationMode::Brainstorming;

                if let Some(summary) = interpret_correction(context.prompt) {
                    let correction = RecentCorrection {
                        id: uuid::Uuid::new_v4().to_string(),
                        summary: summary.clone(),
                        corrected_ref: memory.current_focus.as_ref().map(|focus| focus.summary.clone()),
                    };
                    memory.recent_corrections.insert(0, correction);
                    memory.recent_corrections.truncate(8);

                    memory.current_focus = Some(FocusItem {
                        kind: FocusKind::Structure,
                        summary: summary.clone(),
                        related_refs: Vec::new(),
                    });
                    memory.current_thread.current_focus = memory.current_focus.clone();

                    let summary_lower = summary.to_ascii_lowercase();
                    memory.active_assumptions.retain(|assumption| {
                        !assumption.summary.to_ascii_lowercase().contains(&summary_lower)
                    });
                    memory
                        .pinned_decisions
                        .retain(|decision| !decision.summary.to_ascii_lowercase().contains(&summary_lower));
                }

                if let Some(constraint) = derive_constraint(context.prompt) {
                    if constraint.operator != ConstraintOperator::Correct {
                        let value_lower = constraint.value.to_ascii_lowercase();
                        memory.constraints.retain(|existing| {
                            !(existing.status == ConstraintStatus::Active
                                && existing.scope == constraint.scope
                                && existing.value.to_ascii_lowercase() == value_lower)
                        });
                        memory.constraints.insert(0, constraint);
                        memory.constraints.truncate(8);
                    }
                }

                memory.open_questions.retain(|question| {
                    let lower = question.question.to_ascii_lowercase();
                    !lower.contains("what concrete detail should replace")
                });
                memory.open_questions.insert(
                    0,
                    OpenQuestion {
                        id: uuid::Uuid::new_v4().to_string(),
                        question:
                            "What concrete detail should replace the rejected or corrected idea?"
                                .to_string(),
                        related_refs: Vec::new(),
                        priority: 1,
                    },
                );
                memory.open_questions.truncate(5);
                memory.current_thread.open_questions = memory.open_questions.clone();
            }
            DialogueAct::Commit => {
                memory.recent_corrections.clear();
                memory.conversation_mode = ConversationMode::Committing;
                if let Some(change) = context.preview.changes.first() {
                    memory.current_focus = Some(FocusItem {
                        kind: FocusKind::Structure,
                        summary: change.label.clone(),
                        related_refs: Vec::new(),
                    });
                    memory.current_thread.current_focus = memory.current_focus.clone();
                }
            }
            DialogueAct::Query => {
                memory.recent_corrections.clear();
                if memory.current_focus.is_some() {
                    memory.conversation_mode = ConversationMode::Refining;
                }
            }
            DialogueAct::Brainstorm => {
                memory.recent_corrections.clear();
                memory.conversation_mode = ConversationMode::Brainstorming;
                memory.open_questions.clear();
            }
            DialogueAct::Confirmation => {
                memory.recent_corrections.clear();
                if memory.current_focus.is_some() {
                    memory.conversation_mode = ConversationMode::Committing;
                }
            }
            DialogueAct::RewriteRequest => {
                memory.recent_corrections.clear();
            }
        }

        if let InterpretationTarget::NewTopic(topic) = &context.interpretation.target {
            memory.conversation_topic = topic.clone();
            memory.current_thread.topic = memory.conversation_topic.clone();
        } else if let Some(topic) = infer_conversation_topic(context.prompt) {
            memory.conversation_topic = topic;
            memory.current_thread.topic = memory.conversation_topic.clone();
        } else if matches!(memory.conversation_mode, ConversationMode::Refining)
            && memory.current_focus.is_some()
        {
            // keep existing topic
        } else if matches!(dialogue_act, DialogueAct::Brainstorm) {
            memory.conversation_topic = ConversationTopic::General;
            memory.current_thread.topic = memory.conversation_topic.clone();
        }

        memory.current_thread.turn_count = memory.turn_count;
        memory.current_thread.status = NarrativeThreadStatus::Active;
        sync_legacy_fields_from_thread(&mut memory);
        memory.updated_at = chrono::Utc::now();

        DialogueStateUpdate {
            working_memory: memory,
        }
    }
}

impl AssistantCapabilityPlanner for HeuristicAssistantCapabilityPlanner {
    fn plan(&self, context: CapabilityPlanningContext<'_>) -> CapabilityPlan {
        let _ = context.prompt;
        let dialogue_act = &context.interpretation.dialogue_act;

        let mut capabilities = vec![AssistantCapability::UnderstandTurn];

        match dialogue_act {
            DialogueAct::Query => {
                capabilities.push(AssistantCapability::InspectProjectState);
                capabilities.push(AssistantCapability::GuideNextStep);
                CapabilityPlan {
                    intent: AssistantIntent::Query,
                    capabilities,
                }
            }
            DialogueAct::Brainstorm => {
                if !context.preview.changes.is_empty() {
                    capabilities.push(AssistantCapability::ExtractStructure);
                }
                capabilities.push(AssistantCapability::GuideNextStep);
                CapabilityPlan {
                    intent: AssistantIntent::Guide,
                    capabilities,
                }
            }
            DialogueAct::Constraint | DialogueAct::Correction => {
                if !context.preview.changes.is_empty() {
                    capabilities.push(AssistantCapability::ExtractStructure);
                }
                capabilities.push(AssistantCapability::ResolveAmbiguity);
                capabilities.push(AssistantCapability::GuideNextStep);
                CapabilityPlan {
                    intent: AssistantIntent::Clarify,
                    capabilities,
                }
            }
            DialogueAct::Confirmation | DialogueAct::Commit if context.mutation_allowed => {
                capabilities.push(AssistantCapability::InspectProjectState);
                capabilities.push(AssistantCapability::ExtractStructure);
                capabilities.push(AssistantCapability::CommitStructure);
                capabilities.push(AssistantCapability::GuideNextStep);
                CapabilityPlan {
                    intent: AssistantIntent::MutateOntology,
                    capabilities,
                }
            }
            DialogueAct::Confirmation | DialogueAct::Commit => {
                capabilities.push(AssistantCapability::InspectProjectState);
                capabilities.push(AssistantCapability::ExtractStructure);
                capabilities.push(AssistantCapability::ResolveAmbiguity);
                capabilities.push(AssistantCapability::GuideNextStep);
                CapabilityPlan {
                    intent: AssistantIntent::Clarify,
                    capabilities,
                }
            }
            DialogueAct::RewriteRequest => {
                capabilities.push(AssistantCapability::InspectProjectState);
                capabilities.push(AssistantCapability::ProposeDocumentChange);
                CapabilityPlan {
                    intent: AssistantIntent::MutateDocument,
                    capabilities,
                }
            }
        }
    }
}

impl ResponseStateFinalizer for HeuristicResponseStateFinalizer {
    fn finalize(
        &self,
        mut memory: WorkingMemory,
        prompt: &str,
        plan: &CapabilityPlan,
        title: &str,
        body: &str,
        focus_summary: Option<&str>,
    ) -> WorkingMemory {
        match plan.intent {
            AssistantIntent::Guide | AssistantIntent::Query | AssistantIntent::Clarify => {
                if !matches!(memory.conversation_mode, ConversationMode::Refining)
                    || memory.current_focus.is_none()
                {
                    if let Some(summary) = focus_summary
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string)
                        .or_else(|| select_focus_summary(body))
                        .or_else(|| summarize_reply_focus(title))
                    {
                        memory.current_focus = Some(FocusItem {
                            kind: FocusKind::Structure,
                            summary,
                            related_refs: meaningful_tokens(prompt),
                        });
                        memory.current_thread.current_focus = memory.current_focus.clone();
                    }
                }
            }
            AssistantIntent::MutateOntology => {
                memory.conversation_mode = ConversationMode::Committing;
                memory.current_thread.status = NarrativeThreadStatus::Committed;
            }
            _ => {}
        }

        memory.current_thread.topic = memory.conversation_topic.clone();
        memory.current_thread.turn_count = memory.turn_count;
        memory.current_thread.open_questions = memory.open_questions.clone();
        sync_legacy_fields_from_thread(&mut memory);
        memory.updated_at = chrono::Utc::now();
        memory
    }
}

fn sync_legacy_fields_from_thread(memory: &mut WorkingMemory) {
    memory.conversation_topic = memory.current_thread.topic.clone();
    memory.current_focus = memory.current_thread.current_focus.clone();
    memory.open_questions = memory.current_thread.open_questions.clone();
    memory.turn_count = memory.current_thread.turn_count.max(memory.turn_count);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{NarrativeChangeKind, NarrativeChangeSummary, NarrativeCommitTarget};

    fn empty_preview() -> NarrativeMessagePreview {
        NarrativeMessagePreview {
            prompt: String::new(),
            suggested_target: NarrativeCommitTarget::Character,
            character: None,
            event: None,
            relationships: Vec::new(),
            changes: Vec::new(),
            reply_title: None,
            reply_body: None,
        }
    }

    fn preview_with_change() -> NarrativeMessagePreview {
        NarrativeMessagePreview {
            prompt: "Shyam is Ram's brother".to_string(),
            suggested_target: NarrativeCommitTarget::Character,
            character: None,
            event: None,
            relationships: Vec::new(),
            changes: vec![NarrativeChangeSummary {
                kind: NarrativeChangeKind::AddRelationship,
                label: "Sibling".to_string(),
                detail: "Shyam is Ram's brother".to_string(),
            }],
            reply_title: Some("Story Direction".to_string()),
            reply_body: Some("I can work with this relationship.".to_string()),
        }
    }

    #[test]
    fn belief_updater_adds_constraint_and_forces_no_write() {
        let updater = HeuristicBeliefStateUpdater;
        let update = updater.update(DialogueStateContext {
            prompt: "apart from desert",
            preview: &empty_preview(),
            memory: &WorkingMemory::default(),
            interpretation: &TurnInterpretation {
                dialogue_act: DialogueAct::Constraint,
                target: InterpretationTarget::General,
                route: TurnRoute::Continue,
                confidence: 1.0,
            },
        });

        assert_eq!(update.working_memory.constraints.len(), 1);
        assert_eq!(update.working_memory.constraints[0].value, "desert");
        assert_eq!(update.working_memory.open_questions.len(), 1);
    }

    #[test]
    fn planner_uses_safe_commit_for_grounded_commit() {
        let planner = HeuristicAssistantCapabilityPlanner;
        let plan = planner.plan(CapabilityPlanningContext {
            prompt: "Shyam is Ram's brother",
            preview: &preview_with_change(),
            interpretation: &TurnInterpretation {
                dialogue_act: DialogueAct::Commit,
                target: InterpretationTarget::CurrentCandidate,
                route: TurnRoute::ConfirmCurrent,
                confidence: 1.0,
            },
            mutation_allowed: true,
        });

        assert_eq!(plan.intent, AssistantIntent::MutateOntology);
        assert!(plan
            .capabilities
            .contains(&AssistantCapability::CommitStructure));
    }

    #[test]
    fn finalizer_keeps_last_guidance_as_focus() {
        let finalizer = HeuristicResponseStateFinalizer;
        let memory = WorkingMemory::default();
        let plan = CapabilityPlan {
            intent: AssistantIntent::Guide,
            capabilities: vec![AssistantCapability::GuideNextStep],
        };

        let updated = finalizer.finalize(
            memory,
            "what should be the setting",
            &plan,
            "Idea",
            "How about a forest setting?",
            Some("Forest setting"),
        );

        assert_eq!(
            updated.current_focus.as_ref().map(|f| f.summary.as_str()),
            Some("Forest setting")
        );
        assert_eq!(updated.conversation_mode, ConversationMode::Brainstorming);
    }

    #[test]
    fn commit_turn_clears_stale_recent_corrections() {
        let updater = HeuristicBeliefStateUpdater;
        let mut memory = WorkingMemory::default();
        memory.recent_corrections.push(RecentCorrection {
            id: "1".to_string(),
            summary: "Correction from the writer: about the setting".to_string(),
            corrected_ref: None,
        });

        let update = updater.update(DialogueStateContext {
            prompt: "suggest me a setting for my story",
            preview: &preview_with_change(),
            memory: &memory,
            interpretation: &TurnInterpretation {
                dialogue_act: DialogueAct::Commit,
                target: InterpretationTarget::CurrentCandidate,
                route: TurnRoute::ConfirmCurrent,
                confidence: 1.0,
            },
        });

        assert!(update.working_memory.recent_corrections.is_empty());
    }

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

fn is_low_information_followup(prompt: &str, preview: &NarrativeMessagePreview) -> bool {
    if !preview.changes.is_empty() {
        return false;
    }

    let trimmed = prompt.trim();
    if trimmed.is_empty() || trimmed.ends_with('?') {
        return false;
    }

    let tokens = meaningful_tokens(trimmed);
    tokens.len() <= 2
}

fn infer_conversation_topic(prompt: &str) -> Option<ConversationTopic> {
    let lower = prompt.to_ascii_lowercase();
    if lower.contains("setting")
        || lower.contains("place")
        || lower.contains("location")
        || lower.contains("world")
    {
        Some(ConversationTopic::Setting)
    } else if lower.contains("character") {
        Some(ConversationTopic::Character)
    } else if lower.contains("event") || lower.contains("scene") {
        Some(ConversationTopic::Event)
    } else if lower.contains("relationship") {
        Some(ConversationTopic::Relationship)
    } else {
        None
    }
}

fn derive_constraint(prompt: &str) -> Option<Constraint> {
    let trimmed = prompt.trim();
    let lower = trimmed.to_ascii_lowercase();

    let (operator, value) = if let Some(rest) = lower.strip_prefix("apart from ") {
        (ConstraintOperator::Avoid, trimmed[trimmed.len() - rest.len()..].trim())
    } else if let Some(rest) = lower.strip_prefix("other than ") {
        (ConstraintOperator::Avoid, trimmed[trimmed.len() - rest.len()..].trim())
    } else if let Some(rest) = lower.strip_prefix("except ") {
        (ConstraintOperator::Avoid, trimmed[trimmed.len() - rest.len()..].trim())
    } else if let Some(rest) = lower.strip_prefix("not ") {
        (ConstraintOperator::Avoid, trimmed[trimmed.len() - rest.len()..].trim())
    } else if lower.starts_with("no ") || lower.starts_with("no,") {
        let remainder = trimmed
            .trim_start_matches(|ch: char| {
                ch.eq_ignore_ascii_case(&'n')
                    || ch.eq_ignore_ascii_case(&'o')
                    || ch == ','
                    || ch.is_whitespace()
            })
            .trim();
        (ConstraintOperator::Correct, remainder)
    } else {
        return None;
    };

    if value.is_empty() {
        return None;
    }

    Some(Constraint {
        id: uuid::Uuid::new_v4().to_string(),
        scope: infer_constraint_scope(value),
        operator,
        value: value.to_string(),
        source: trimmed.to_string(),
        status: ConstraintStatus::Active,
    })
}

fn infer_constraint_scope(value: &str) -> ConstraintScope {
    let lower = value.to_ascii_lowercase();
    if lower.contains("desert")
        || lower.contains("forest")
        || lower.contains("city")
        || lower.contains("village")
        || lower.contains("kingdom")
        || lower.contains("setting")
    {
        ConstraintScope::Setting
    } else if lower.contains("tone") || lower.contains("funny") || lower.contains("dark") {
        ConstraintScope::Tone
    } else if lower.contains("relationship") || lower.contains("brother") || lower.contains("advisor") {
        ConstraintScope::Relationship
    } else {
        ConstraintScope::General
    }
}

fn interpret_correction(prompt: &str) -> Option<String> {
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();

    for prefix in ["apart from ", "other than ", "except ", "anything but ", "not "] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let phrase = trimmed[trimmed.len() - rest.len()..].trim();
            if !phrase.is_empty() {
                return Some(format!("Avoid or reject this idea: {}", phrase));
            }
        }
    }

    if lower.starts_with("no ") || lower.starts_with("no,") {
        let remainder = trimmed
            .trim_start_matches(|ch: char| {
                ch.eq_ignore_ascii_case(&'n')
                    || ch.eq_ignore_ascii_case(&'o')
                    || ch == ','
                    || ch.is_whitespace()
            })
            .trim();
        if !remainder.is_empty() {
            return Some(format!("Correction from the writer: {}", remainder));
        }
        return Some("The writer rejected the previous idea.".to_string());
    }

    if lower.starts_with("actually ") || lower.starts_with("instead ") {
        let remainder = trimmed
            .split_once(' ')
            .map(|(_, tail)| tail.trim())
            .unwrap_or(trimmed);
        if !remainder.is_empty() {
            return Some(format!("Correction from the writer: {}", remainder));
        }
    }

    None
}

fn summarize_reply_focus(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let sentence = trimmed
        .split_terminator(['.', '!', '?'])
        .next()
        .unwrap_or(trimmed)
        .trim();
    if sentence.is_empty() {
        return None;
    }

    Some(sentence.to_string())
}

fn select_focus_summary(body: &str) -> Option<String> {
    let paragraphs = body
        .split("\n\n")
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    for paragraph in &paragraphs {
        if paragraph.len() <= 180 {
            return summarize_reply_focus(paragraph);
        }
    }

    summarize_reply_focus(body)
}
