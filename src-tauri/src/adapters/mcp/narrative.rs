use crate::adapters::tauri::state::AppState;
use crate::application::{AssistantIntentContext, CapabilityPlanningContext};
use crate::domain::{
    AssistantCapability, AssistantIntent, NarrativeMessagePreview, SyncRun, WorkingMemory,
    WritePolicy,
};
use crate::ports::{AssistantResponse, AssistantToolCall, WorkingMemoryRepository};

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
    
    // 1. Initial grounding check via existing control model
    let initial_preview = preview_turn(state, &trimmed_prompt)?;
    let intent = state.assistant_intent_classifier.classify(AssistantIntentContext {
        prompt: &trimmed_prompt,
        preview: &initial_preview,
    });
    let mutation_allowed = state.mutation_gate.allow_mutation(AssistantIntentContext {
        prompt: &trimmed_prompt,
        preview: &initial_preview,
    });
    let plan = state.assistant_capability_planner.plan(CapabilityPlanningContext {
        prompt: &trimmed_prompt,
        preview: &initial_preview,
        intent,
        mutation_allowed,
    });

    let screenplay = state
        .screenplay_service
        .get_active_screenplay()
        .map_err(|err| err.to_string())?;
    let mut sync_run = start_sync_run(
        state,
        &trimmed_prompt,
        screenplay.version,
        plan.write_policy.clone(),
    )?;

    let mut observations: Vec<(McpToolCall, McpToolResult)> = Vec::new();
    let mut loop_count = 0;
    let mut proposal_recorded = false;
    const MAX_LOOPS: usize = 3;

    loop {
        loop_count += 1;
        
        let response = state.assistant_agent.process_turn(
            &trimmed_prompt,
            &working_memory,
            &observations,
        );

        let response = match response {
            Ok(res) => res,
            Err(_) => {
                let fallback = state.assistant_fallback_responder.respond(
                    &trimmed_prompt,
                    &initial_preview,
                    &working_memory,
                    &plan,
                );
                finish_sync_run(state, &mut sync_run)?;
                return match fallback {
                    AssistantResponse::FinalReply { title, body, .. } => Ok(NarrativeTurnOutcome {
                        intent: plan.intent,
                        capabilities: plan.capabilities.clone(),
                        write_policy: WritePolicy::NoWrite,
                        reply_title: title,
                        reply_body: body,
                        committed: initial_preview,
                        working_memory,
                    }),
                    AssistantResponse::ToolCalls(_) => Err("fallback responder returned tool calls unexpectedly".to_string()),
                };
            }
        };

        match response {
            AssistantResponse::ToolCalls(calls) => {
                if loop_count >= MAX_LOOPS {
                    finish_sync_run(state, &mut sync_run)?;
                    return Ok(NarrativeTurnOutcome {
                        intent: plan.intent,
                        capabilities: plan.capabilities.clone(),
                        write_policy: if proposal_recorded { WritePolicy::CandidateOnly } else { WritePolicy::NoWrite },
                        reply_title: "Architect's Notes".to_string(),
                        reply_body: "I've processed your input and updated my internal focus. Let's keep building the world.".to_string(),
                        committed: initial_preview,
                        working_memory,
                    });
                }

                for tool_call in calls {
                    if !tool_allowed(&tool_call, plan.write_policy.clone()) {
                        observations.push((tool_call.call, McpToolResult::Empty));
                        continue;
                    }

                    if matches!(tool_call.call, McpToolCall::ProposeOntologyCommit { .. }) && !mutation_allowed {
                        observations.push((tool_call.call, McpToolResult::Empty));
                        continue;
                    }

                    let call = attach_sync_run(tool_call.call, sync_run.as_ref().map(|run| run.id));
                    let result = execute_tool(state, call.clone())?;

                    if matches!(call, McpToolCall::ProposeOntologyCommit { .. }) {
                        proposal_recorded = true;
                    }

                    observations.push((call, result));
                }
            }
            AssistantResponse::FinalReply { intent: _, title, body } => {
                working_memory.updated_at = chrono::Utc::now();
                if let Some(repository) = state.sqlite_repository.as_ref() {
                    let _ = repository.save_working_memory(working_memory.clone());
                }
                finish_sync_run(state, &mut sync_run)?;

                return Ok(NarrativeTurnOutcome {
                    intent: plan.intent,
                    capabilities: plan.capabilities,
                    write_policy: if proposal_recorded { WritePolicy::CandidateOnly } else { WritePolicy::NoWrite }, 
                    reply_title: title,
                    reply_body: body,
                    committed: initial_preview,
                    working_memory,
                });
            }
        }
    }
}

fn tool_allowed(tool_call: &AssistantToolCall, write_policy: WritePolicy) -> bool {
    match (&tool_call.call, write_policy) {
        (
            McpToolCall::GetNarrativeSnapshot
            | McpToolCall::GetActiveScreenplay
            | McpToolCall::GetWorkingMemory,
            _,
        ) => true,
        (McpToolCall::AddStoryTask { .. } | McpToolCall::ResolveStoryTask { .. }, WritePolicy::CandidateOnly | WritePolicy::SafeCommit) => true,
        (McpToolCall::ProposeOntologyCommit { .. }, WritePolicy::CandidateOnly | WritePolicy::SafeCommit) => true,
        _ => false,
    }
}

fn attach_sync_run(call: McpToolCall, sync_run_id: Option<uuid::Uuid>) -> McpToolCall {
    match call {
        McpToolCall::ProposeOntologyCommit {
            title,
            summary,
            entity,
            relationship,
            ..
        } => McpToolCall::ProposeOntologyCommit {
            sync_run_id,
            title,
            summary,
            entity,
            relationship,
        },
        other => other,
    }
}

fn start_sync_run(
    state: &AppState,
    prompt: &str,
    document_version: u64,
    write_policy: WritePolicy,
) -> Result<Option<SyncRun>, String> {
    if matches!(write_policy, WritePolicy::NoWrite) {
        return Ok(None);
    }

    match execute_tool(
        state,
        McpToolCall::StartNarrativeSyncRun {
            prompt: prompt.to_string(),
            document_version,
        },
    )? {
        McpToolResult::SyncRun(run) => Ok(Some(run)),
        _ => Err("sync start tool returned an unexpected result".to_string()),
    }
}

fn finish_sync_run(state: &AppState, sync_run: &mut Option<SyncRun>) -> Result<(), String> {
    let Some(run) = sync_run.take() else {
        return Ok(());
    };

    let _ = execute_tool(state, McpToolCall::FinishSyncRun { sync_run: run })?;
    Ok(())
}
