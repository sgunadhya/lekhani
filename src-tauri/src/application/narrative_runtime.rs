use crate::application::assistant_turn::is_underspecified_followup;
use crate::domain::{
    AssistantCapability, AssistantIntent, ConversationMode, ConversationTopic,
    NarrativeCommitTarget, NarrativeMessagePreview, NarrativeSuggestedAction,
    NarrativeSuggestionAction, WorkingMemory, WritePolicy,
};
use crate::ports::{
    AssistantResponse, FollowUpDirective, MutationGateway, NarrativeGenerationGateway,
    QueryGateway,
};

use super::{
    CapabilityPlan, CapabilityPlanningContext, DialogueAct, DialogueActContext,
    DialogueStateContext, NarrativeConversationSupport,
};

pub struct NarrativeRuntimeDeps<'a> {
    pub query_gateway: &'a dyn QueryGateway,
    pub mutation_gateway: &'a dyn MutationGateway,
    pub generation_gateway: &'a dyn NarrativeGenerationGateway,
    pub conversation_support: &'a dyn NarrativeConversationSupport,
}

pub struct NarrativeTurnOutcome {
    pub intent: AssistantIntent,
    pub capabilities: Vec<AssistantCapability>,
    pub write_policy: WritePolicy,
    pub reply_title: String,
    pub reply_body: String,
    pub committed: NarrativeMessagePreview,
    pub working_memory: WorkingMemory,
    pub suggested_actions: Vec<NarrativeSuggestedAction>,
}

pub trait NarrativeRuntime: Send + Sync {
    fn preview_turn(
        &self,
        deps: &NarrativeRuntimeDeps<'_>,
        prompt: &str,
    ) -> Result<NarrativeMessagePreview, String>;

    fn submit_turn(
        &self,
        deps: &NarrativeRuntimeDeps<'_>,
        prompt: &str,
    ) -> Result<NarrativeTurnOutcome, String>;

    fn apply_suggestion_action(
        &self,
        deps: &NarrativeRuntimeDeps<'_>,
        action: NarrativeSuggestionAction,
    ) -> Result<NarrativeTurnOutcome, String>;
}

pub struct DefaultNarrativeRuntime;

impl NarrativeRuntime for DefaultNarrativeRuntime {
    fn preview_turn(
        &self,
        deps: &NarrativeRuntimeDeps<'_>,
        prompt: &str,
    ) -> Result<NarrativeMessagePreview, String> {
        let story = deps.query_gateway.load_story_snapshot()?;
        deps.generation_gateway.preview_message(prompt, &story.narrative)
    }

    fn submit_turn(
        &self,
        deps: &NarrativeRuntimeDeps<'_>,
        prompt: &str,
    ) -> Result<NarrativeTurnOutcome, String> {
        let trimmed_prompt = prompt.trim().to_string();
        if trimmed_prompt.is_empty() {
            return Err("prompt is empty".to_string());
        }

        let story = deps.query_gateway.load_story_snapshot()?;
        let mut working_memory = story.working_memory;
        let classification_preview = empty_preview(&trimmed_prompt);
        let dialogue_act = deps.conversation_support.classify_dialogue_act(DialogueActContext {
            prompt: &trimmed_prompt,
            preview: &classification_preview,
            memory: &working_memory,
        });
        let state_update = deps.conversation_support.update_belief_state(DialogueStateContext {
            prompt: &trimmed_prompt,
            preview: &classification_preview,
            memory: &working_memory,
            dialogue_act: &dialogue_act,
        });
        let prior_memory = working_memory.clone();
        working_memory = state_update.working_memory;
        let is_followup_turn =
            prior_memory.current_focus.is_some()
                && is_underspecified_followup(&trimmed_prompt, &classification_preview);
        let substantive_preview = if !is_followup_turn {
            deps.generation_gateway
                .preview_message(&trimmed_prompt, &story.narrative)
                .ok()
        } else {
            None
        };
        let has_substantive_new_anchor = substantive_preview
            .as_ref()
            .map(|preview| !preview.changes.is_empty())
            .unwrap_or(false);
        let dialogue_act = if has_substantive_new_anchor
            && matches!(
                dialogue_act,
                DialogueAct::Constraint | DialogueAct::Correction
            ) {
            DialogueAct::Brainstorm
        } else {
            dialogue_act
        };

        if has_substantive_new_anchor && !is_followup_turn {
            working_memory.recent_corrections.clear();
            working_memory.open_questions.clear();
            working_memory.conversation_mode = ConversationMode::Brainstorming;
        }

        if is_followup_turn
            && !matches!(prior_memory.conversation_mode, ConversationMode::Committing)
        {
            let plan = deps.conversation_support.plan_capabilities(CapabilityPlanningContext {
                prompt: &trimmed_prompt,
                preview: &classification_preview,
                dialogue_act: &dialogue_act,
                mutation_allowed: false,
            });
            let directive = deps
                .generation_gateway
                .interpret_followup(&trimmed_prompt, &prior_memory)
                .unwrap_or(FollowUpDirective::Unknown);

            return match directive {
                FollowUpDirective::ElaborateCurrent | FollowUpDirective::Unknown => {
                    working_memory.conversation_mode = ConversationMode::Refining;
                    respond_from_specialized_path(
                        deps,
                        &trimmed_prompt,
                        working_memory,
                        plan,
                        deps.generation_gateway
                            .elaborate_focus(&trimmed_prompt, &prior_memory)?,
                        WritePolicy::NoWrite,
                    )
                }
                FollowUpDirective::AlternativeOption | FollowUpDirective::RejectCurrent => {
                    working_memory.conversation_mode = ConversationMode::Brainstorming;
                    working_memory.current_focus = None;
                    respond_from_specialized_path(
                        deps,
                        &trimmed_prompt,
                        working_memory,
                        plan,
                        deps.generation_gateway
                            .suggest_alternative(&trimmed_prompt, &prior_memory)?,
                        WritePolicy::NoWrite,
                    )
                }
                FollowUpDirective::ConfirmCurrent => respond_from_specialized_path(
                    deps,
                    &trimmed_prompt,
                    working_memory,
                    plan,
                    deps.generation_gateway
                        .respond_in_context(&trimmed_prompt, &prior_memory)?,
                    WritePolicy::NoWrite,
                ),
                FollowUpDirective::ShiftToCharacter => {
                    working_memory.conversation_mode = ConversationMode::Brainstorming;
                    working_memory.conversation_topic = ConversationTopic::Character;
                    let response = deps
                        .generation_gateway
                        .brainstorm_topic(&trimmed_prompt, &working_memory)?;
                    respond_from_specialized_path(
                        deps,
                        &trimmed_prompt,
                        working_memory,
                        plan_for_topic_shift(AssistantIntent::Guide),
                        response,
                        WritePolicy::NoWrite,
                    )
                }
                FollowUpDirective::ShiftToEvent => {
                    working_memory.conversation_mode = ConversationMode::Brainstorming;
                    working_memory.conversation_topic = ConversationTopic::Event;
                    let response = deps
                        .generation_gateway
                        .brainstorm_topic(&trimmed_prompt, &working_memory)?;
                    respond_from_specialized_path(
                        deps,
                        &trimmed_prompt,
                        working_memory,
                        plan_for_topic_shift(AssistantIntent::Guide),
                        response,
                        WritePolicy::NoWrite,
                    )
                }
                FollowUpDirective::AddToScreenplay => {
                    respond_from_specialized_path(
                        deps,
                        &trimmed_prompt,
                        working_memory,
                        plan_for_topic_shift(AssistantIntent::Guide),
                        deps.generation_gateway
                            .respond_in_context(&trimmed_prompt, &prior_memory)?,
                        WritePolicy::NoWrite,
                    )
                }
            };
        }

        if prior_memory.current_focus.is_some() && !is_followup_turn {
            working_memory.conversation_mode = ConversationMode::Brainstorming;
            working_memory.current_focus = None;
        }

        if !matches!(dialogue_act, DialogueAct::RewriteRequest) {
            let plan = deps.conversation_support.plan_capabilities(CapabilityPlanningContext {
                prompt: &trimmed_prompt,
                preview: &classification_preview,
                dialogue_act: &dialogue_act,
                mutation_allowed: false,
            });
            working_memory.conversation_mode = ConversationMode::Brainstorming;
            return respond_from_specialized_path(
                deps,
                &trimmed_prompt,
                working_memory,
                plan,
                deps.generation_gateway
                    .respond_in_context(&trimmed_prompt, &prior_memory)?,
                WritePolicy::NoWrite,
            );
        }

        if matches!(dialogue_act, DialogueAct::RewriteRequest) && prior_memory.current_focus.is_some()
        {
            return respond_from_specialized_path(
                deps,
                &trimmed_prompt,
                working_memory,
                plan_for_topic_shift(AssistantIntent::Guide),
                deps.generation_gateway
                    .draft_from_focus(&trimmed_prompt, &prior_memory)?,
                WritePolicy::NoWrite,
            );
        }
        let plan = deps.conversation_support.plan_capabilities(CapabilityPlanningContext {
            prompt: &trimmed_prompt,
            preview: &classification_preview,
            dialogue_act: &dialogue_act,
            mutation_allowed: false,
        });
        respond_from_specialized_path(
            deps,
            &trimmed_prompt,
            working_memory,
            plan,
            deps.generation_gateway
                .respond_in_context(&trimmed_prompt, &prior_memory)?,
            WritePolicy::NoWrite,
        )
    }

    fn apply_suggestion_action(
        &self,
        deps: &NarrativeRuntimeDeps<'_>,
        action: NarrativeSuggestionAction,
    ) -> Result<NarrativeTurnOutcome, String> {
        let prior_memory = deps.query_gateway.load_working_memory()?;
        let mut working_memory = prior_memory.clone();
        let plan = plan_for_topic_shift(match action {
            NarrativeSuggestionAction::UseThis => AssistantIntent::MutateOntology,
            NarrativeSuggestionAction::TryAnother => AssistantIntent::Guide,
            NarrativeSuggestionAction::ExpandThis => AssistantIntent::Guide,
            NarrativeSuggestionAction::AddToScreenplay => AssistantIntent::MutateDocument,
        });

        match action {
            NarrativeSuggestionAction::UseThis => {
                confirm_current_focus(deps, "Use this", prior_memory, working_memory, plan)
            }
            NarrativeSuggestionAction::TryAnother => {
                working_memory.conversation_mode = ConversationMode::Brainstorming;
                working_memory.current_focus = None;
                    respond_from_specialized_path(
                        deps,
                        "Try another",
                        working_memory,
                        plan,
                        deps.generation_gateway.suggest_alternative("", &prior_memory)?,
                        WritePolicy::NoWrite,
                    )
                }
            NarrativeSuggestionAction::ExpandThis => {
                working_memory.conversation_mode = ConversationMode::Refining;
                    respond_from_specialized_path(
                        deps,
                        "Expand this",
                        working_memory,
                        plan,
                        deps.generation_gateway.elaborate_focus("", &prior_memory)?,
                        WritePolicy::NoWrite,
                    )
                }
            NarrativeSuggestionAction::AddToScreenplay => {
                working_memory.conversation_mode = ConversationMode::Committing;
                    respond_from_specialized_path(
                        deps,
                        "Add to screenplay",
                        working_memory,
                        plan,
                        deps.generation_gateway.draft_from_focus("", &prior_memory)?,
                        WritePolicy::CandidateOnly,
                    )
                }
        }
    }
}

fn confirm_current_focus(
    deps: &NarrativeRuntimeDeps<'_>,
    prompt: &str,
    prior_memory: WorkingMemory,
    mut working_memory: WorkingMemory,
    plan: CapabilityPlan,
) -> Result<NarrativeTurnOutcome, String> {
    let Some(focus) = prior_memory.current_focus.as_ref() else {
        return Err("no active focus to confirm".to_string());
    };

    let _ = deps
        .mutation_gateway
        .confirm_current_focus(prior_memory.conversation_topic.clone(), &focus.summary)?;

    working_memory.conversation_mode = ConversationMode::Committing;
    working_memory.recent_corrections.clear();
    working_memory.open_questions.clear();
    let _ = deps.mutation_gateway.save_working_memory(working_memory.clone());
    let suggested_actions = suggested_actions_for_turn(
        &AssistantIntent::MutateOntology,
        WritePolicy::CandidateOnly,
        &working_memory.conversation_topic,
        &working_memory.current_focus,
    );

    Ok(NarrativeTurnOutcome {
        intent: AssistantIntent::MutateOntology,
        capabilities: plan.capabilities,
        write_policy: WritePolicy::CandidateOnly,
        reply_title: "Story Direction".to_string(),
        reply_body: format!("I’ve recorded this as the current direction:\n{}", focus.summary),
        committed: empty_preview(prompt),
        working_memory,
        suggested_actions,
    })
}

fn respond_from_specialized_path(
    deps: &NarrativeRuntimeDeps<'_>,
    prompt: &str,
    mut working_memory: WorkingMemory,
    plan: CapabilityPlan,
    response: AssistantResponse,
    write_policy: WritePolicy,
) -> Result<NarrativeTurnOutcome, String> {
    let AssistantResponse::FinalReply {
        title,
        body,
        focus_summary,
        ..
    } = response;

    working_memory = deps.conversation_support.finalize_response_state(
        working_memory,
        prompt,
        &plan,
        &title,
        &body,
        focus_summary.as_deref(),
    );
    let _ = deps.mutation_gateway.save_working_memory(working_memory.clone());
    let suggested_actions = suggested_actions_for_turn(
        &plan.intent,
        write_policy.clone(),
        &working_memory.conversation_topic,
        &working_memory.current_focus,
    );

    Ok(NarrativeTurnOutcome {
        intent: plan.intent,
        capabilities: plan.capabilities,
        write_policy,
        reply_title: title,
        reply_body: body,
        committed: empty_preview(prompt),
        working_memory,
        suggested_actions,
    })
}

fn empty_preview(prompt: &str) -> NarrativeMessagePreview {
    NarrativeMessagePreview {
        prompt: prompt.to_string(),
        suggested_target: NarrativeCommitTarget::Character,
        character: None,
        event: None,
        relationships: Vec::new(),
        changes: Vec::new(),
        reply_title: None,
        reply_body: None,
    }
}

fn plan_for_topic_shift(intent: AssistantIntent) -> CapabilityPlan {
    let mut capabilities = vec![AssistantCapability::UnderstandTurn];
    match intent {
        AssistantIntent::Guide => {
            capabilities.push(AssistantCapability::GuideNextStep);
            CapabilityPlan {
                intent,
                capabilities,
                write_policy: WritePolicy::NoWrite,
            }
        }
        AssistantIntent::MutateDocument => {
            capabilities.push(AssistantCapability::ProposeDocumentChange);
            CapabilityPlan {
                intent,
                capabilities,
                write_policy: WritePolicy::CandidateOnly,
            }
        }
        _ => CapabilityPlan {
            intent,
            capabilities,
            write_policy: WritePolicy::NoWrite,
        },
    }
}

fn suggested_actions_for_turn(
    intent: &AssistantIntent,
    write_policy: WritePolicy,
    topic: &ConversationTopic,
    current_focus: &Option<crate::domain::FocusItem>,
) -> Vec<NarrativeSuggestedAction> {
    if current_focus.is_none() {
        return Vec::new();
    }

    let use_label = match topic {
        ConversationTopic::Setting => "Use this setting",
        ConversationTopic::Character => "Use this character",
        ConversationTopic::Event => "Use this event",
        ConversationTopic::Relationship => "Use this relationship",
        ConversationTopic::General => "Use this idea",
    };

    let expand_label = match topic {
        ConversationTopic::Setting => "Expand this setting",
        ConversationTopic::Character => "Deepen this character",
        ConversationTopic::Event => "Expand this event",
        ConversationTopic::Relationship => "Deepen this relationship",
        ConversationTopic::General => "Expand this",
    };

    let alternative_label = match topic {
        ConversationTopic::Setting => "Try another setting",
        ConversationTopic::Character => "Try another character",
        ConversationTopic::Event => "Try another event",
        ConversationTopic::Relationship => "Try another relationship",
        ConversationTopic::General => "Try another idea",
    };

    let screenplay_label = match intent {
        AssistantIntent::MutateDocument => "Add this to screenplay",
        _ => "Add to screenplay",
    };

    let primary_action = match write_policy {
        WritePolicy::NoWrite => NarrativeSuggestionAction::UseThis,
        WritePolicy::CandidateOnly | WritePolicy::SafeCommit => NarrativeSuggestionAction::AddToScreenplay,
    };

    [
        (NarrativeSuggestionAction::UseThis, use_label),
        (NarrativeSuggestionAction::TryAnother, alternative_label),
        (NarrativeSuggestionAction::ExpandThis, expand_label),
        (NarrativeSuggestionAction::AddToScreenplay, screenplay_label),
    ]
    .into_iter()
    .map(|(action, label)| NarrativeSuggestedAction {
        primary: action == primary_action,
        action,
        label: label.to_string(),
    })
    .collect()
}
