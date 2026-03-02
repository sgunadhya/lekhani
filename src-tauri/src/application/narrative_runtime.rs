use crate::application::assistant_turn::is_underspecified_followup;
use crate::domain::{
    AssistantCapability, AssistantIntent, ConversationMode, ConversationTopic, InteractionState,
    NarrativeCommitTarget, NarrativeMessagePreview, NarrativeMode, NarrativeSuggestedAction,
    NarrativeSuggestionAction, WorkingMemory, WritePolicy,
};
use crate::ports::{
    AssistantResponse, FollowUpDirective, MutationGateway, NarrativeGenerationGateway,
    QueryGateway,
};

use super::{
    derive_thread_status, CapabilityPlan, CapabilityPlanningContext, DialogueAct,
    DialogueActContext, DialogueStateContext, InterpretationTarget,
    NarrativeConversationSupport, NarrativeEngine, TurnInterpretation, TurnRoute,
};

pub struct NarrativeRuntimeDeps<'a> {
    pub query_gateway: &'a dyn QueryGateway,
    pub mutation_gateway: &'a dyn MutationGateway,
    pub generation_gateway: &'a dyn NarrativeGenerationGateway,
    pub conversation_support: &'a dyn NarrativeConversationSupport,
    pub narrative_engine: &'a dyn NarrativeEngine,
}

pub struct NarrativeTurnOutcome {
    pub intent: AssistantIntent,
    pub capabilities: Vec<AssistantCapability>,
    pub write_policy: WritePolicy,
    pub interpretation_target: InterpretationTarget,
    pub interpretation_route: TurnRoute,
    pub interpretation_confidence: f32,
    pub reply_title: String,
    pub reply_body: String,
    pub narrative_mode: NarrativeMode,
    pub thread_status: crate::domain::ThreadStatus,
    pub active_beat: Option<crate::domain::BeatId>,
    pub evaluation_nudge: Option<String>,
    pub committed: NarrativeMessagePreview,
    pub working_memory: WorkingMemory,
    pub suggested_actions: Vec<NarrativeSuggestedAction>,
}

#[derive(Debug, Clone)]
enum NarrativeMsg {
    Followup(FollowUpDirective),
    Conversational,
    RewriteRequest,
    Suggestion(NarrativeSuggestionAction),
}

#[derive(Debug, Clone)]
enum NarrativeEffect {
    RespondInContext,
    ElaborateFocus,
    SuggestAlternative,
    BrainstormTopic,
    DraftFromFocus,
    ConfirmCurrentFocus,
    ParkThread,
    ResumeThread,
    CommitSidequest,
}

struct RuntimeTransition {
    working_memory: WorkingMemory,
    plan: CapabilityPlan,
    interpretation: TurnInterpretation,
    effect: NarrativeEffect,
    write_policy: WritePolicy,
}

struct ReducedMessage {
    prior_memory: WorkingMemory,
    prompt: String,
    transition: RuntimeTransition,
}

struct SuggestionTransition {
    prior_memory: WorkingMemory,
    transition: RuntimeTransition,
    prompt: &'static str,
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
        let mut interpretation =
            deps.conversation_support.classify_dialogue_act(DialogueActContext {
            prompt: &trimmed_prompt,
            memory: &working_memory,
        });
        let state_update = deps.conversation_support.update_belief_state(DialogueStateContext {
            prompt: &trimmed_prompt,
            preview: &classification_preview,
            memory: &working_memory,
            interpretation: &interpretation,
        });
        let prior_memory = working_memory.clone();
        working_memory = state_update.working_memory;
        working_memory.turn_count = working_memory.turn_count.saturating_add(1);
        working_memory.current_thread.turn_count = working_memory.turn_count;
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
        if has_substantive_new_anchor
            && matches!(
                interpretation.dialogue_act,
                DialogueAct::Constraint | DialogueAct::Correction
            )
        {
            interpretation.dialogue_act = DialogueAct::Brainstorm;
            interpretation.target = InterpretationTarget::General;
        }

        if has_substantive_new_anchor && !is_followup_turn {
            working_memory.recent_corrections.clear();
            working_memory.open_questions.clear();
            working_memory.conversation_mode = ConversationMode::Brainstorming;
        }

        if is_followup_turn
            && !matches!(prior_memory.conversation_mode, ConversationMode::Committing)
        {
            let directive = match interpretation.route {
                TurnRoute::ElaborateCurrent => FollowUpDirective::ElaborateCurrent,
                TurnRoute::AlternativeCurrent => FollowUpDirective::AlternativeOption,
                TurnRoute::ConfirmCurrent => FollowUpDirective::ConfirmCurrent,
                TurnRoute::RejectCurrent => FollowUpDirective::RejectCurrent,
                TurnRoute::ShiftToCharacter => FollowUpDirective::ShiftToCharacter,
                TurnRoute::ShiftToEvent => FollowUpDirective::ShiftToEvent,
                TurnRoute::AddToScreenplay => FollowUpDirective::AddToScreenplay,
                TurnRoute::Continue => deps
                    .generation_gateway
                    .interpret_followup(&trimmed_prompt, &prior_memory)
                    .unwrap_or(FollowUpDirective::Unknown),
            };
            let reduced = reduce_message(
                &trimmed_prompt,
                &classification_preview,
                prior_memory.clone(),
                working_memory,
                interpretation,
                NarrativeMsg::Followup(directive),
                deps,
            );
            return execute_transition(deps, &reduced.prompt, &reduced.prior_memory, reduced.transition);
        }

        if prior_memory.current_focus.is_some() && !is_followup_turn {
            working_memory.conversation_mode = ConversationMode::Brainstorming;
            working_memory.current_focus = None;
        }

        if !matches!(interpretation.dialogue_act, DialogueAct::RewriteRequest) {
            let reduced = reduce_message(
                &trimmed_prompt,
                &classification_preview,
                prior_memory.clone(),
                working_memory,
                interpretation,
                NarrativeMsg::Conversational,
                deps,
            );
            return execute_transition(deps, &reduced.prompt, &reduced.prior_memory, reduced.transition);
        }

        if matches!(interpretation.dialogue_act, DialogueAct::RewriteRequest)
            && prior_memory.current_focus.is_some()
        {
            let reduced = reduce_message(
                &trimmed_prompt,
                &classification_preview,
                prior_memory.clone(),
                working_memory,
                interpretation,
                NarrativeMsg::RewriteRequest,
                deps,
            );
            return execute_transition(deps, &reduced.prompt, &reduced.prior_memory, reduced.transition);
        }
        let reduced = reduce_message(
            &trimmed_prompt,
            &classification_preview,
            prior_memory.clone(),
            working_memory,
            interpretation,
            NarrativeMsg::Conversational,
            deps,
        );
        execute_transition(deps, &reduced.prompt, &reduced.prior_memory, reduced.transition)
    }

    fn apply_suggestion_action(
        &self,
        deps: &NarrativeRuntimeDeps<'_>,
        action: NarrativeSuggestionAction,
    ) -> Result<NarrativeTurnOutcome, String> {
        let prior_memory = deps.query_gateway.load_working_memory()?;
        let mut working_memory = prior_memory.clone();
        working_memory.turn_count = working_memory.turn_count.saturating_add(1);
        working_memory.current_thread.turn_count = working_memory.turn_count;
        match action {
            NarrativeSuggestionAction::UseThis
            | NarrativeSuggestionAction::TryAnother
            | NarrativeSuggestionAction::ExpandThis
            | NarrativeSuggestionAction::AddToScreenplay
            | NarrativeSuggestionAction::ParkThread
            | NarrativeSuggestionAction::ResumeThread
            | NarrativeSuggestionAction::CommitSidequest => {
                let reduced = reduce_message(
                    "",
                    &empty_preview(""),
                    prior_memory,
                    working_memory,
                    TurnInterpretation {
                        dialogue_act: DialogueAct::Query,
                        target: InterpretationTarget::CurrentCandidate,
                        route: TurnRoute::Continue,
                        confidence: 1.0,
                    },
                    NarrativeMsg::Suggestion(action),
                    deps,
                );
                execute_transition(
                    deps,
                    &reduced.prompt,
                    &reduced.prior_memory,
                    reduced.transition,
                )
            }
        }
    }
}

fn reduce_message(
    prompt: &str,
    preview: &NarrativeMessagePreview,
    prior_memory: WorkingMemory,
    working_memory: WorkingMemory,
    interpretation: TurnInterpretation,
    msg: NarrativeMsg,
    deps: &NarrativeRuntimeDeps<'_>,
) -> ReducedMessage {
    match msg {
        NarrativeMsg::Suggestion(action) => {
            let intent = match &action {
                NarrativeSuggestionAction::UseThis => AssistantIntent::MutateOntology,
                NarrativeSuggestionAction::TryAnother => AssistantIntent::Guide,
                NarrativeSuggestionAction::ExpandThis => AssistantIntent::Guide,
                NarrativeSuggestionAction::AddToScreenplay => AssistantIntent::MutateDocument,
                NarrativeSuggestionAction::ParkThread => AssistantIntent::Guide,
                NarrativeSuggestionAction::ResumeThread => AssistantIntent::Guide,
                NarrativeSuggestionAction::CommitSidequest => AssistantIntent::MutateOntology,
            };
            let reduced = reduce_suggestion_action(
                action,
                prior_memory,
                working_memory,
                plan_for_topic_shift(intent),
            );
            ReducedMessage {
                prior_memory: reduced.prior_memory,
                prompt: reduced.prompt.to_string(),
                transition: reduced.transition,
            }
        }
        _ => ReducedMessage {
            prior_memory,
            prompt: prompt.to_string(),
            transition: reduce_user_turn(
                prompt,
                preview,
                working_memory,
                interpretation,
                msg,
                deps,
            ),
        },
    }
}

fn reduce_suggestion_action(
    action: NarrativeSuggestionAction,
    prior_memory: WorkingMemory,
    mut working_memory: WorkingMemory,
    plan: CapabilityPlan,
) -> SuggestionTransition {
    match action {
        NarrativeSuggestionAction::UseThis => SuggestionTransition {
            prior_memory,
            prompt: "Use this",
            transition: RuntimeTransition {
                working_memory,
                plan,
                interpretation: TurnInterpretation {
                    dialogue_act: DialogueAct::Commit,
                    target: InterpretationTarget::CurrentCandidate,
                    route: TurnRoute::ConfirmCurrent,
                    confidence: 1.0,
                },
                effect: NarrativeEffect::ConfirmCurrentFocus,
                write_policy: WritePolicy::CandidateOnly,
            },
        },
        NarrativeSuggestionAction::TryAnother => {
            working_memory.conversation_mode = ConversationMode::Brainstorming;
            working_memory.current_focus = None;
            working_memory.current_thread.current_focus = None;
            working_memory.current_thread.status = crate::domain::NarrativeThreadStatus::Active;
            SuggestionTransition {
                prior_memory,
                prompt: "Try another",
                transition: RuntimeTransition {
                    working_memory,
                    plan,
                    interpretation: TurnInterpretation {
                        dialogue_act: DialogueAct::Brainstorm,
                        target: InterpretationTarget::CurrentCandidate,
                        route: TurnRoute::AlternativeCurrent,
                        confidence: 1.0,
                    },
                    effect: NarrativeEffect::SuggestAlternative,
                    write_policy: WritePolicy::NoWrite,
                },
            }
        }
        NarrativeSuggestionAction::ExpandThis => {
            working_memory.conversation_mode = ConversationMode::Refining;
            working_memory.current_thread.status = crate::domain::NarrativeThreadStatus::Active;
            SuggestionTransition {
                prior_memory,
                prompt: "Expand this",
                transition: RuntimeTransition {
                    working_memory,
                    plan,
                    interpretation: TurnInterpretation {
                        dialogue_act: DialogueAct::Query,
                        target: InterpretationTarget::CurrentCandidate,
                        route: TurnRoute::ElaborateCurrent,
                        confidence: 1.0,
                    },
                    effect: NarrativeEffect::ElaborateFocus,
                    write_policy: WritePolicy::NoWrite,
                },
            }
        }
        NarrativeSuggestionAction::AddToScreenplay => {
            working_memory.conversation_mode = ConversationMode::Committing;
            working_memory.current_thread.status = crate::domain::NarrativeThreadStatus::Committed;
            SuggestionTransition {
                prior_memory,
                prompt: "Add to screenplay",
                transition: RuntimeTransition {
                    working_memory,
                    plan,
                    interpretation: TurnInterpretation {
                        dialogue_act: DialogueAct::RewriteRequest,
                        target: InterpretationTarget::Screenplay,
                        route: TurnRoute::AddToScreenplay,
                        confidence: 1.0,
                    },
                    effect: NarrativeEffect::DraftFromFocus,
                    write_policy: WritePolicy::CandidateOnly,
                },
            }
        }
        NarrativeSuggestionAction::ParkThread => SuggestionTransition {
            prior_memory,
            prompt: "Park thread",
            transition: RuntimeTransition {
                working_memory,
                plan,
                interpretation: TurnInterpretation {
                    dialogue_act: DialogueAct::Commit,
                    target: InterpretationTarget::CurrentCandidate,
                    route: TurnRoute::Continue,
                    confidence: 1.0,
                },
                effect: NarrativeEffect::ParkThread,
                write_policy: WritePolicy::NoWrite,
            },
        },
        NarrativeSuggestionAction::ResumeThread => SuggestionTransition {
            prior_memory,
            prompt: "Resume thread",
            transition: RuntimeTransition {
                working_memory,
                plan,
                interpretation: TurnInterpretation {
                    dialogue_act: DialogueAct::Query,
                    target: InterpretationTarget::CurrentCandidate,
                    route: TurnRoute::Continue,
                    confidence: 1.0,
                },
                effect: NarrativeEffect::ResumeThread,
                write_policy: WritePolicy::NoWrite,
            },
        },
        NarrativeSuggestionAction::CommitSidequest => SuggestionTransition {
            prior_memory,
            prompt: "Commit sidequest",
            transition: RuntimeTransition {
                working_memory,
                plan,
                interpretation: TurnInterpretation {
                    dialogue_act: DialogueAct::Commit,
                    target: InterpretationTarget::CurrentCandidate,
                    route: TurnRoute::Continue,
                    confidence: 1.0,
                },
                effect: NarrativeEffect::CommitSidequest,
                write_policy: WritePolicy::CandidateOnly,
            },
        },
    }
}

fn reduce_user_turn(
    prompt: &str,
    preview: &NarrativeMessagePreview,
    mut working_memory: WorkingMemory,
    interpretation: TurnInterpretation,
    msg: NarrativeMsg,
    deps: &NarrativeRuntimeDeps<'_>,
) -> RuntimeTransition {
    match msg {
        NarrativeMsg::Suggestion(_) => unreachable!("suggestions are reduced by reduce_message"),
        NarrativeMsg::Followup(directive) => match directive {
            FollowUpDirective::ElaborateCurrent | FollowUpDirective::Unknown => {
                working_memory.conversation_mode = ConversationMode::Refining;
                RuntimeTransition {
                    working_memory,
                    plan: deps.conversation_support.plan_capabilities(CapabilityPlanningContext {
                        prompt,
                        preview,
                        interpretation: &interpretation,
                        mutation_allowed: false,
                    }),
                    interpretation,
                    effect: NarrativeEffect::ElaborateFocus,
                    write_policy: WritePolicy::NoWrite,
                }
            }
            FollowUpDirective::AlternativeOption | FollowUpDirective::RejectCurrent => {
                working_memory.conversation_mode = ConversationMode::Brainstorming;
                working_memory.current_focus = None;
                RuntimeTransition {
                    working_memory,
                    plan: deps.conversation_support.plan_capabilities(CapabilityPlanningContext {
                        prompt,
                        preview,
                        interpretation: &interpretation,
                        mutation_allowed: false,
                    }),
                    interpretation,
                    effect: NarrativeEffect::SuggestAlternative,
                    write_policy: WritePolicy::NoWrite,
                }
            }
            FollowUpDirective::ConfirmCurrent => RuntimeTransition {
                working_memory,
                plan: deps.conversation_support.plan_capabilities(CapabilityPlanningContext {
                    prompt,
                    preview,
                    interpretation: &interpretation,
                    mutation_allowed: false,
                }),
                interpretation,
                effect: NarrativeEffect::RespondInContext,
                write_policy: WritePolicy::NoWrite,
            },
            FollowUpDirective::ShiftToCharacter => {
                working_memory.conversation_mode = ConversationMode::Brainstorming;
                working_memory.conversation_topic = ConversationTopic::Character;
                RuntimeTransition {
                    working_memory,
                    plan: plan_for_topic_shift(AssistantIntent::Guide),
                    interpretation,
                    effect: NarrativeEffect::BrainstormTopic,
                    write_policy: WritePolicy::NoWrite,
                }
            }
            FollowUpDirective::ShiftToEvent => {
                working_memory.conversation_mode = ConversationMode::Brainstorming;
                working_memory.conversation_topic = ConversationTopic::Event;
                RuntimeTransition {
                    working_memory,
                    plan: plan_for_topic_shift(AssistantIntent::Guide),
                    interpretation,
                    effect: NarrativeEffect::BrainstormTopic,
                    write_policy: WritePolicy::NoWrite,
                }
            }
            FollowUpDirective::AddToScreenplay => RuntimeTransition {
                working_memory,
                plan: plan_for_topic_shift(AssistantIntent::Guide),
                interpretation,
                effect: NarrativeEffect::RespondInContext,
                write_policy: WritePolicy::NoWrite,
            },
        },
        NarrativeMsg::Conversational => {
            working_memory.conversation_mode = ConversationMode::Brainstorming;
            RuntimeTransition {
                working_memory,
                plan: deps.conversation_support.plan_capabilities(CapabilityPlanningContext {
                    prompt,
                    preview,
                    interpretation: &interpretation,
                    mutation_allowed: false,
                }),
                interpretation,
                effect: NarrativeEffect::RespondInContext,
                write_policy: WritePolicy::NoWrite,
            }
        }
        NarrativeMsg::RewriteRequest => RuntimeTransition {
            working_memory,
            plan: plan_for_topic_shift(AssistantIntent::Guide),
            interpretation,
            effect: NarrativeEffect::DraftFromFocus,
            write_policy: WritePolicy::NoWrite,
        },
    }
}

fn execute_transition(
    deps: &NarrativeRuntimeDeps<'_>,
    prompt: &str,
    prior_memory: &WorkingMemory,
    transition: RuntimeTransition,
) -> Result<NarrativeTurnOutcome, String> {
    let response = match transition.effect {
        NarrativeEffect::RespondInContext => {
            deps.generation_gateway.respond_in_context(prompt, prior_memory)?
        }
        NarrativeEffect::ElaborateFocus => {
            deps.generation_gateway.elaborate_focus(prompt, prior_memory)?
        }
        NarrativeEffect::SuggestAlternative => {
            deps.generation_gateway.suggest_alternative(prompt, prior_memory)?
        }
        NarrativeEffect::BrainstormTopic => {
            deps.generation_gateway.brainstorm_topic(prompt, &transition.working_memory)?
        }
        NarrativeEffect::DraftFromFocus => {
            deps.generation_gateway.draft_from_focus(prompt, prior_memory)?
        }
        NarrativeEffect::ConfirmCurrentFocus => {
            let Some(focus) = prior_memory.current_focus.as_ref() else {
                return Err("no active focus to confirm".to_string());
            };

            let _ = deps.mutation_gateway.confirm_current_focus(
                prior_memory.conversation_topic.clone(),
                &focus.summary,
            )?;

            let mut committed_memory = transition.working_memory;
            committed_memory.conversation_mode = ConversationMode::Committing;
            committed_memory.recent_corrections.clear();
            committed_memory.open_questions.clear();

            return respond_from_specialized_path(
                deps,
                prompt,
                committed_memory,
                transition.plan,
                AssistantResponse::FinalReply {
                    title: "Story Direction".to_string(),
                    body: format!("I’ve recorded this as the current direction:\n{}", focus.summary),
                    focus_summary: Some(focus.summary.clone()),
                },
                &transition.interpretation,
                transition.write_policy,
            );
        }
        NarrativeEffect::ParkThread => {
            let mut parked_memory = transition.working_memory;
            parked_memory.conversation_mode = ConversationMode::Brainstorming;
            if parked_memory.current_thread.current_focus.is_some() {
                let mut parked = parked_memory.current_thread.clone();
                parked.status = crate::domain::NarrativeThreadStatus::Parked;
                parked.scope = crate::domain::ThreadScope::Sidequest;
                parked_memory.sidequests.push(parked);
            }
            parked_memory.current_focus = None;
            parked_memory.current_thread = fresh_current_thread(
                parked_memory.turn_count,
                ConversationTopic::General,
            );
            return respond_from_specialized_path(
                deps,
                prompt,
                parked_memory,
                transition.plan,
                AssistantResponse::FinalReply {
                    title: "Thread parked".to_string(),
                    body: "I’ve set this thread aside for later. You can resume it whenever you want."
                        .to_string(),
                    focus_summary: None,
                },
                &transition.interpretation,
                transition.write_policy,
            );
        }
        NarrativeEffect::ResumeThread => {
            let mut resumed_memory = transition.working_memory;
            let Some(mut resumed) = resumed_memory.sidequests.pop() else {
                return Err("no parked sidequest to resume".to_string());
            };
            resumed_memory.return_thread = Some(resumed_memory.current_thread.clone());
            resumed.status = crate::domain::NarrativeThreadStatus::Active;
            resumed.scope = crate::domain::ThreadScope::Sidequest;
            resumed.return_to_thread_id = resumed_memory
                .return_thread
                .as_ref()
                .map(|thread| thread.id.clone());
            resumed_memory.conversation_mode = ConversationMode::Refining;
            resumed_memory.current_thread = resumed.clone();
            resumed_memory.conversation_topic = resumed.topic.clone();
            resumed_memory.current_focus = resumed.current_focus.clone();
            resumed_memory.open_questions = resumed.open_questions.clone();
            return respond_from_specialized_path(
                deps,
                prompt,
                resumed_memory,
                transition.plan,
                AssistantResponse::FinalReply {
                    title: "Thread resumed".to_string(),
                    body: "We’re back on this thread. Keep developing it or choose the next move."
                        .to_string(),
                    focus_summary: prior_memory
                        .current_thread
                        .current_focus
                        .as_ref()
                        .map(|focus| focus.summary.clone()),
                },
                &transition.interpretation,
                transition.write_policy,
            );
        }
        NarrativeEffect::CommitSidequest => {
            let mut sidequest_memory = transition.working_memory;
            if !matches!(
                sidequest_memory.current_thread.scope,
                crate::domain::ThreadScope::Sidequest
            ) {
                return Err("no active sidequest to commit".to_string());
            }
            let sidequest = sidequest_memory.current_thread.clone();
            if let Some(focus) = sidequest.current_focus.as_ref() {
                let _ = deps
                    .mutation_gateway
                    .confirm_current_focus(sidequest.topic.clone(), &focus.summary)?;
            }
            sidequest_memory.conversation_mode = ConversationMode::Brainstorming;
            if let Some(return_thread) = sidequest_memory.return_thread.take() {
                sidequest_memory.current_thread = return_thread.clone();
                sidequest_memory.conversation_topic = return_thread.topic.clone();
                sidequest_memory.current_focus = return_thread.current_focus.clone();
                sidequest_memory.open_questions = return_thread.open_questions.clone();
            }
            return respond_from_specialized_path(
                deps,
                prompt,
                sidequest_memory,
                transition.plan,
                AssistantResponse::FinalReply {
                    title: "Sidequest committed".to_string(),
                    body: "I’ve committed the most recent sidequest into the story state."
                        .to_string(),
                    focus_summary: None,
                },
                &transition.interpretation,
                transition.write_policy,
            );
        }
    };

    respond_from_specialized_path(
        deps,
        prompt,
        transition.working_memory,
        transition.plan,
        response,
        &transition.interpretation,
        transition.write_policy,
    )
}

fn respond_from_specialized_path(
    deps: &NarrativeRuntimeDeps<'_>,
    prompt: &str,
    mut working_memory: WorkingMemory,
    plan: CapabilityPlan,
    response: AssistantResponse,
    interpretation: &TurnInterpretation,
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
    sync_thread_from_legacy_fields(&mut working_memory);
    let _ = deps.mutation_gateway.save_working_memory(working_memory.clone());
    let intent = plan.intent.clone();
    let evaluation =
        deps.narrative_engine
            .evaluate(&build_interaction_state(
                &working_memory,
                interpretation.clone(),
            ));
    let suggested_actions = suggested_actions_for_evaluation(
        &evaluation,
        &working_memory.conversation_topic,
        &working_memory.current_focus,
        &working_memory.current_thread.status,
    );

    Ok(NarrativeTurnOutcome {
        intent,
        capabilities: plan.capabilities,
        write_policy,
        interpretation_target: interpretation.target.clone(),
        interpretation_route: interpretation.route.clone(),
        interpretation_confidence: interpretation.confidence,
        reply_title: title,
        reply_body: body,
        narrative_mode: evaluation.mode.clone(),
        thread_status: evaluation_thread_status(&working_memory),
        active_beat: evaluation.beat.clone(),
        evaluation_nudge: evaluation.nudge.clone(),
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
            CapabilityPlan { intent, capabilities }
        }
        AssistantIntent::MutateDocument => {
            capabilities.push(AssistantCapability::ProposeDocumentChange);
            CapabilityPlan { intent, capabilities }
        }
        _ => CapabilityPlan { intent, capabilities },
    }
}

fn suggested_actions_for_evaluation(
    evaluation: &crate::domain::EvaluationResult,
    topic: &ConversationTopic,
    current_focus: &Option<crate::domain::FocusItem>,
    current_thread_status: &crate::domain::NarrativeThreadStatus,
) -> Vec<NarrativeSuggestedAction> {
    if evaluation.actions.is_empty()
        || (current_focus.is_none()
        && !matches!(
            current_thread_status,
            crate::domain::NarrativeThreadStatus::Parked
        ))
    {
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

    let screenplay_label = "Add to screenplay";
    let actions = evaluation.actions.clone();
    let primary_action = actions
        .first()
        .cloned()
        .unwrap_or(NarrativeSuggestionAction::UseThis);

    actions
        .iter()
        .cloned()
        .map(|action| {
            let label = match action {
                NarrativeSuggestionAction::UseThis => use_label,
                NarrativeSuggestionAction::TryAnother => alternative_label,
                NarrativeSuggestionAction::ExpandThis => expand_label,
                NarrativeSuggestionAction::AddToScreenplay => screenplay_label,
                NarrativeSuggestionAction::ParkThread => "Park as sidequest",
                NarrativeSuggestionAction::ResumeThread => "Resume sidequest",
                NarrativeSuggestionAction::CommitSidequest => "Commit sidequest",
            };
            NarrativeSuggestedAction {
                primary: action == primary_action,
                action,
                label: label.to_string(),
            }
        })
        .collect()
}

fn build_interaction_state(
    memory: &WorkingMemory,
    interpretation: TurnInterpretation,
) -> InteractionState {
    let current_candidate = memory
        .current_thread
        .current_focus
        .as_ref()
        .map(|focus| focus.summary.clone());
    let current_mode = match memory.conversation_mode {
        ConversationMode::Brainstorming => {
            if current_candidate.is_some() {
                NarrativeMode::Converging
            } else {
                NarrativeMode::Brainstorming
            }
        }
        ConversationMode::Refining => NarrativeMode::Elaborating,
        ConversationMode::Committing => NarrativeMode::Committing,
    };
    InteractionState {
        current_candidate,
        current_mode,
        thread_status: memory.current_thread.thread_status.clone(),
        current_thread_scope: memory.current_thread.scope.clone(),
        has_return_thread: memory.return_thread.is_some(),
        turn_count: memory.current_thread.turn_count.max(memory.turn_count),
        last_interpretation_target: interpretation.target,
        last_turn_route: interpretation.route,
        last_interpretation_confidence: interpretation.confidence,
        open_sidequests: memory
            .sidequests
            .iter()
            .filter(|thread| matches!(thread.status, crate::domain::NarrativeThreadStatus::Active))
            .count(),
    }
}

fn evaluation_thread_status(memory: &WorkingMemory) -> crate::domain::ThreadStatus {
    memory.current_thread.thread_status.clone()
}

fn sync_thread_from_legacy_fields(memory: &mut WorkingMemory) {
    memory.current_thread.topic = memory.conversation_topic.clone();
    memory.current_thread.current_focus = memory.current_focus.clone();
    memory.current_thread.open_questions = memory.open_questions.clone();
    memory.current_thread.turn_count = memory.turn_count;
    if !matches!(
        memory.current_thread.status,
        crate::domain::NarrativeThreadStatus::Parked
    ) {
        memory.current_thread.status = match memory.conversation_mode {
            ConversationMode::Committing => crate::domain::NarrativeThreadStatus::Committed,
            _ => crate::domain::NarrativeThreadStatus::Active,
        };
    }
    memory.current_thread.thread_status = derive_thread_status(
        &match memory.conversation_mode {
            ConversationMode::Brainstorming => {
                if memory.current_thread.current_focus.is_some() {
                    NarrativeMode::Converging
                } else {
                    NarrativeMode::Brainstorming
                }
            }
            ConversationMode::Refining => NarrativeMode::Elaborating,
            ConversationMode::Committing => NarrativeMode::Committing,
        },
        memory.current_thread.current_focus.is_some(),
        memory.current_thread.turn_count.max(memory.turn_count),
        &TurnRoute::Continue,
    );

    if memory.current_thread.goal.trim().is_empty() || memory.current_thread.goal == "Shape the story" {
        memory.current_thread.goal = memory
            .current_focus
            .as_ref()
            .map(|focus| focus.summary.clone())
            .unwrap_or_else(|| match memory.conversation_topic {
                ConversationTopic::Setting => "Develop the setting".to_string(),
                ConversationTopic::Character => "Develop the character".to_string(),
                ConversationTopic::Event => "Develop the event".to_string(),
                ConversationTopic::Relationship => "Develop the relationship".to_string(),
                ConversationTopic::General => "Shape the story".to_string(),
            });
    }
}

fn fresh_current_thread(
    turn_count: u32,
    topic: ConversationTopic,
) -> crate::domain::NarrativeThread {
    crate::domain::NarrativeThread {
        id: uuid::Uuid::new_v4().to_string(),
        goal: match topic {
            ConversationTopic::Setting => "Develop the setting".to_string(),
            ConversationTopic::Character => "Develop the character".to_string(),
            ConversationTopic::Event => "Develop the event".to_string(),
            ConversationTopic::Relationship => "Develop the relationship".to_string(),
            ConversationTopic::General => "Shape the story".to_string(),
        },
        status: crate::domain::NarrativeThreadStatus::Active,
        thread_status: crate::domain::ThreadStatus::Active,
        scope: crate::domain::ThreadScope::Main,
        return_to_thread_id: None,
        topic,
        current_focus: None,
        open_questions: Vec::new(),
        turn_count,
    }
}
