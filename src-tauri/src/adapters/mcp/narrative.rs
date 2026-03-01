use crate::adapters::tauri::state::AppState;
use crate::application::{
    AssistantIntentContext, CapabilityPlanningContext, DialogueAct, DialogueActContext,
    DialogueStateContext,
};
use crate::domain::{
    AssistantCapability, AssistantIntent, ConversationMode, ConversationTopic,
    NarrativeCommitTarget, NarrativeMessagePreview, OntologyEntity, OntologyEntityKind,
    WorkingMemory, WritePolicy,
};
use crate::ports::{
    AssistantResponse, FollowUpDirective, WorkingMemoryRepository,
};

use super::{execute_tool, McpToolCall, McpToolResult};

pub struct NarrativeTurnOutcome {
    pub intent: AssistantIntent,
    pub capabilities: Vec<AssistantCapability>,
    pub write_policy: WritePolicy,
    pub reply_title: String,
    pub reply_body: String,
    pub committed: NarrativeMessagePreview,
    pub working_memory: WorkingMemory,
}

pub fn preview_turn(state: &AppState, prompt: &str) -> Result<NarrativeMessagePreview, String> {
    let snapshot = match execute_tool(state, McpToolCall::GetNarrativeSnapshot)? {
        McpToolResult::Snapshot(snapshot) => snapshot,
        _ => return Err("narrative snapshot tool returned an unexpected result".to_string()),
    };

    state.narrative_service.preview_message(prompt, &snapshot)
}

pub fn submit_turn(state: &AppState, prompt: &str) -> Result<NarrativeTurnOutcome, String> {
    let trimmed_prompt = prompt.trim().to_string();
    if trimmed_prompt.is_empty() {
        return Err("prompt is empty".to_string());
    }

    let mut working_memory = state.get_working_memory()?;
    let classification_preview = empty_preview(&trimmed_prompt);
    let dialogue_act = state.dialogue_act_classifier.classify(DialogueActContext {
        prompt: &trimmed_prompt,
        preview: &classification_preview,
        memory: &working_memory,
    });
    let state_update = state.belief_state_updater.update(DialogueStateContext {
        prompt: &trimmed_prompt,
        preview: &classification_preview,
        memory: &working_memory,
        dialogue_act: &dialogue_act,
    });
    let prior_memory = working_memory.clone();
    working_memory = state_update.working_memory;

    if matches!(dialogue_act, DialogueAct::Confirmation)
        && prior_memory.current_focus.is_some()
    {
        let plan = state.assistant_capability_planner.plan(CapabilityPlanningContext {
            prompt: &trimmed_prompt,
            preview: &classification_preview,
            dialogue_act: &dialogue_act,
            mutation_allowed: true,
        });
        return confirm_current_focus(state, &trimmed_prompt, prior_memory, working_memory, plan);
    }

    if !matches!(
        dialogue_act,
        DialogueAct::Confirmation | DialogueAct::Commit | DialogueAct::RewriteRequest
    ) && prior_memory.current_focus.is_some()
        && !matches!(prior_memory.conversation_mode, ConversationMode::Committing)
    {
        let plan = state.assistant_capability_planner.plan(CapabilityPlanningContext {
            prompt: &trimmed_prompt,
            preview: &classification_preview,
            dialogue_act: &dialogue_act,
            mutation_allowed: false,
        });
        let directive = state
            .assistant_agent
            .interpret_followup(&trimmed_prompt, &prior_memory)
            .unwrap_or(FollowUpDirective::Unknown);

        return match directive {
            FollowUpDirective::ElaborateCurrent | FollowUpDirective::Unknown => {
                working_memory.conversation_mode = ConversationMode::Refining;
                respond_from_specialized_path(
                    state,
                    &trimmed_prompt,
                    working_memory,
                    plan,
                    state
                        .assistant_agent
                        .elaborate_focus(&trimmed_prompt, &prior_memory)?,
                    WritePolicy::NoWrite,
                )
            }
            FollowUpDirective::AlternativeOption | FollowUpDirective::RejectCurrent => {
                working_memory.conversation_mode = ConversationMode::Brainstorming;
                working_memory.current_focus = None;
                respond_from_specialized_path(
                    state,
                    &trimmed_prompt,
                    working_memory,
                    plan,
                    state
                        .assistant_agent
                        .suggest_alternative(&trimmed_prompt, &prior_memory)?,
                    WritePolicy::NoWrite,
                )
            }
            FollowUpDirective::ConfirmCurrent => {
                confirm_current_focus(state, &trimmed_prompt, prior_memory, working_memory, plan)
            }
            FollowUpDirective::ShiftToCharacter => {
                working_memory.conversation_mode = ConversationMode::Brainstorming;
                working_memory.conversation_topic = ConversationTopic::Character;
                let response = state
                    .assistant_agent
                    .brainstorm_topic(&trimmed_prompt, &working_memory)?;
                respond_from_specialized_path(
                    state,
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
                let response = state
                    .assistant_agent
                    .brainstorm_topic(&trimmed_prompt, &working_memory)?;
                respond_from_specialized_path(
                    state,
                    &trimmed_prompt,
                    working_memory,
                    plan_for_topic_shift(AssistantIntent::Guide),
                    response,
                    WritePolicy::NoWrite,
                )
            }
            FollowUpDirective::AddToScreenplay => {
                working_memory.conversation_mode = ConversationMode::Committing;
                respond_from_specialized_path(
                    state,
                    &trimmed_prompt,
                    working_memory,
                    plan_for_topic_shift(AssistantIntent::MutateDocument),
                    state
                        .assistant_agent
                        .draft_from_focus(&trimmed_prompt, &prior_memory)?,
                    WritePolicy::CandidateOnly,
                )
            }
        };
    }

    if !matches!(dialogue_act, DialogueAct::Confirmation | DialogueAct::Commit | DialogueAct::RewriteRequest)
        && prior_memory.current_focus.is_none()
    {
        let plan = state.assistant_capability_planner.plan(CapabilityPlanningContext {
            prompt: &trimmed_prompt,
            preview: &classification_preview,
            dialogue_act: &DialogueAct::Brainstorm,
            mutation_allowed: false,
        });
        working_memory.conversation_mode = ConversationMode::Brainstorming;
        return respond_from_specialized_path(
            state,
            &trimmed_prompt,
            working_memory,
            plan,
            state
                .assistant_agent
                .brainstorm_topic(&trimmed_prompt, &prior_memory)?,
            WritePolicy::NoWrite,
        );
    }

    if matches!(dialogue_act, DialogueAct::RewriteRequest) && prior_memory.current_focus.is_some() {
        working_memory.conversation_mode = ConversationMode::Committing;
        return respond_from_specialized_path(
            state,
            &trimmed_prompt,
            working_memory,
            plan_for_topic_shift(AssistantIntent::MutateDocument),
            state
                .assistant_agent
                .draft_from_focus(&trimmed_prompt, &prior_memory)?,
            WritePolicy::CandidateOnly,
        );
    }

    let initial_preview = preview_turn(state, &trimmed_prompt)?;
    let mutation_allowed = state.mutation_gate.allow_mutation(AssistantIntentContext {
        prompt: &trimmed_prompt,
        preview: &initial_preview,
    });
    let mut plan = state.assistant_capability_planner.plan(CapabilityPlanningContext {
        prompt: &trimmed_prompt,
        preview: &initial_preview,
        dialogue_act: &dialogue_act,
        mutation_allowed,
    });
    if state_update.force_no_write {
        plan.write_policy = WritePolicy::NoWrite;
    }

    if matches!(plan.write_policy, WritePolicy::SafeCommit | WritePolicy::CandidateOnly) {
        if propose_preview(state, &initial_preview)? {
            working_memory = state.response_state_finalizer.finalize(
                working_memory,
                &trimmed_prompt,
                &plan,
                initial_preview
                    .reply_title
                    .as_deref()
                    .unwrap_or("Story Direction"),
                initial_preview
                    .reply_body
                    .as_deref()
                    .unwrap_or("I recorded this as a structured proposal."),
                None,
            );
            if let Some(repository) = state.sqlite_repository.as_ref() {
                let _ = repository.save_working_memory(working_memory.clone());
            }

            return Ok(NarrativeTurnOutcome {
                intent: plan.intent,
                capabilities: plan.capabilities,
                write_policy: WritePolicy::CandidateOnly,
                reply_title: initial_preview
                    .reply_title
                    .clone()
                    .unwrap_or_else(|| "Story Direction".to_string()),
                reply_body: initial_preview.reply_body.clone().unwrap_or_else(|| {
                    "I recorded this as a structured proposal.".to_string()
                }),
                committed: initial_preview,
                working_memory,
            });
        }
    }

    let fallback = state.assistant_fallback_responder.respond(
        &trimmed_prompt,
        &initial_preview,
        &working_memory,
        &plan,
    );
    respond_from_specialized_path(
        state,
        &trimmed_prompt,
        working_memory,
        plan,
        fallback,
        WritePolicy::NoWrite,
    )
}

fn confirm_current_focus(
    state: &AppState,
    prompt: &str,
    prior_memory: WorkingMemory,
    mut working_memory: WorkingMemory,
    plan: crate::application::CapabilityPlan,
) -> Result<NarrativeTurnOutcome, String> {
    let Some(focus) = prior_memory.current_focus.as_ref() else {
        return Err("no active focus to confirm".to_string());
    };

    if matches!(prior_memory.conversation_topic, ConversationTopic::Setting) {
        let entity = OntologyEntity {
            id: uuid::Uuid::new_v4(),
            kind: OntologyEntityKind::Setting,
            label: focus.summary.clone(),
            summary: focus.summary.clone(),
        };
        let _ = execute_tool(
            state,
            McpToolCall::ProposeOntologyCommit {
                sync_run_id: None,
                title: "Setting proposal".to_string(),
                summary: focus.summary.clone(),
                entity: Some(entity),
                relationship: None,
            },
        )?;
    }

    working_memory.conversation_mode = ConversationMode::Committing;
    working_memory.recent_corrections.clear();
    working_memory.open_questions.clear();
    if let Some(repository) = state.sqlite_repository.as_ref() {
        let _ = repository.save_working_memory(working_memory.clone());
    }

    Ok(NarrativeTurnOutcome {
        intent: AssistantIntent::MutateOntology,
        capabilities: plan.capabilities,
        write_policy: WritePolicy::CandidateOnly,
        reply_title: "Story Direction".to_string(),
        reply_body: format!("I’ve recorded this as the current direction:\n{}", focus.summary),
        committed: empty_preview(prompt),
        working_memory,
    })
}

fn respond_from_specialized_path(
    state: &AppState,
    prompt: &str,
    mut working_memory: WorkingMemory,
    plan: crate::application::CapabilityPlan,
    response: AssistantResponse,
    write_policy: WritePolicy,
) -> Result<NarrativeTurnOutcome, String> {
    let AssistantResponse::FinalReply {
        title,
        body,
        focus_summary,
        ..
    } = response
    else {
        return Err("specialized path returned tool calls unexpectedly".to_string());
    };

    working_memory = state.response_state_finalizer.finalize(
        working_memory,
        prompt,
        &plan,
        &title,
        &body,
        focus_summary.as_deref(),
    );
    if let Some(repository) = state.sqlite_repository.as_ref() {
        let _ = repository.save_working_memory(working_memory.clone());
    }

    Ok(NarrativeTurnOutcome {
        intent: plan.intent,
        capabilities: plan.capabilities,
        write_policy,
        reply_title: title,
        reply_body: body,
        committed: empty_preview(prompt),
        working_memory,
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

fn propose_preview(state: &AppState, preview: &NarrativeMessagePreview) -> Result<bool, String> {
    if let Some(character) = preview.character.as_ref() {
        execute_tool(
            state,
            McpToolCall::ProposeOntologyCommit {
                sync_run_id: None,
                title: format!("Character proposal: {}", character.name),
                summary: character.summary.clone(),
                entity: Some(OntologyEntity {
                    id: uuid::Uuid::new_v4(),
                    kind: OntologyEntityKind::Character,
                    label: character.name.clone(),
                    summary: character.summary.clone(),
                }),
                relationship: None,
            },
        )?;
        return Ok(true);
    }

    if let Some(event) = preview.event.as_ref() {
        execute_tool(
            state,
            McpToolCall::ProposeOntologyCommit {
                sync_run_id: None,
                title: format!("Event proposal: {}", event.title),
                summary: event.summary.clone(),
                entity: Some(OntologyEntity {
                    id: uuid::Uuid::new_v4(),
                    kind: OntologyEntityKind::Event,
                    label: event.title.clone(),
                    summary: event.summary.clone(),
                }),
                relationship: None,
            },
        )?;
        return Ok(true);
    }

    if let Some(relationship) = preview.relationships.first().cloned() {
        execute_tool(
            state,
            McpToolCall::ProposeOntologyCommit {
                sync_run_id: None,
                title: "Relationship proposal".to_string(),
                summary: relationship.summary.clone(),
                entity: None,
                relationship: Some(relationship),
            },
        )?;
        return Ok(true);
    }

    Ok(false)
}

fn plan_for_topic_shift(intent: AssistantIntent) -> crate::application::CapabilityPlan {
    let mut capabilities = vec![AssistantCapability::UnderstandTurn];
    match intent {
        AssistantIntent::MutateDocument => {
            capabilities.push(AssistantCapability::ProposeDocumentChange);
            crate::application::CapabilityPlan {
                intent,
                capabilities,
                write_policy: WritePolicy::CandidateOnly,
            }
        }
        _ => {
            capabilities.push(AssistantCapability::GuideNextStep);
            crate::application::CapabilityPlan {
                intent,
                capabilities,
                write_policy: WritePolicy::NoWrite,
            }
        }
    }
}
