use crate::adapters::tauri::state::AppState;
use crate::application::{AssistantIntentContext, CapabilityPlanningContext};
use crate::domain::{
    AssistantCapability, AssistantIntent, FocusItem, FocusKind, NarrativeChangeKind,
    NarrativeCommitTarget, NarrativeMessagePreview, ToolActionRecord, WorkingMemory,
    SyncActionKind, SyncTargetKind, WritePolicy,
};
use crate::ports::WorkingMemoryRepository;

use super::sync::record_applied_candidate_payload;
use super::{document, ontology, sync, McpToolCall, McpToolResult};

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
    let snapshot = match ontology::execute(state, McpToolCall::GetNarrativeSnapshot)? {
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

    let screenplay = match document::execute(state, McpToolCall::GetActiveScreenplay)? {
        McpToolResult::Screenplay(screenplay) => screenplay,
        _ => return Err("active screenplay tool returned an unexpected result".to_string()),
    };
    let committed = preview_turn(state, &trimmed_prompt)?;
    let intent = state.assistant_intent_classifier.classify(AssistantIntentContext {
        prompt: &trimmed_prompt,
        preview: &committed,
    });
    let mutation_allowed = state.mutation_gate.allow_mutation(AssistantIntentContext {
        prompt: &trimmed_prompt,
        preview: &committed,
    });
    let plan = state.assistant_capability_planner.plan(CapabilityPlanningContext {
        prompt: &trimmed_prompt,
        preview: &committed,
        intent,
        mutation_allowed,
    });
    if matches!(plan.write_policy, WritePolicy::SafeCommit) {
        let sync_run = match sync::execute(
            state,
            McpToolCall::StartNarrativeSyncRun {
                prompt: trimmed_prompt.clone(),
                document_version: screenplay.version,
            },
        )? {
            McpToolResult::SyncRun(sync_run) => sync_run,
            _ => return Err("sync run tool returned an unexpected result".to_string()),
        };

        match committed.suggested_target {
            NarrativeCommitTarget::Character => {
                let character = committed
                    .character
                    .clone()
                    .ok_or_else(|| "unable to hydrate a character preview from this input".to_string())?;
                let change_title = committed
                    .changes
                    .first()
                    .map(|change| change.label.clone())
                    .unwrap_or_else(|| "Narrative character".to_string());
                let change_detail = committed
                    .changes
                    .first()
                    .map(|change| change.detail.clone())
                    .unwrap_or_else(|| "Applied character change from narrative turn".to_string());

                match ontology::execute(
                    state,
                    McpToolCall::UpsertCharacter {
                        character: character.clone(),
                    },
                )? {
                    McpToolResult::Empty => {}
                    _ => return Err("character upsert tool returned an unexpected result".to_string()),
                }

                record_applied_ontology_candidate(
                    state,
                    &sync_run,
                    &trimmed_prompt,
                    SyncTargetKind::Character,
                    SyncActionKind::Create,
                    &change_title,
                    &change_detail,
                    &record_applied_candidate_payload(&Some(character.clone()))?,
                    "narrative_character",
                    Some(character.id.to_string()),
                )?;
            }
            NarrativeCommitTarget::Event => {
                let event = committed
                    .event
                    .clone()
                    .ok_or_else(|| "unable to hydrate an event preview from this input".to_string())?;
                let change_title = committed
                    .changes
                    .first()
                    .map(|change| change.label.clone())
                    .unwrap_or_else(|| "Narrative event".to_string());
                let change_detail = committed
                    .changes
                    .first()
                    .map(|change| change.detail.clone())
                    .unwrap_or_else(|| "Applied event change from narrative turn".to_string());

                match ontology::execute(
                    state,
                    McpToolCall::UpsertEvent {
                        event: event.clone(),
                    },
                )? {
                    McpToolResult::Empty => {}
                    _ => return Err("event upsert tool returned an unexpected result".to_string()),
                }

                record_applied_ontology_candidate(
                    state,
                    &sync_run,
                    &trimmed_prompt,
                    SyncTargetKind::Event,
                    SyncActionKind::Create,
                    &change_title,
                    &change_detail,
                    &record_applied_candidate_payload(&Some(event.clone()))?,
                    "narrative_event",
                    Some(event.id.to_string()),
                )?;
            }
        }

        for relationship in committed.relationships.iter().cloned() {
            let relationship_id = relationship.id.to_string();
            match ontology::execute(
                state,
                McpToolCall::UpsertRelationship {
                    relationship: relationship.clone(),
                },
            )? {
                McpToolResult::Empty => {}
                _ => return Err("relationship upsert tool returned an unexpected result".to_string()),
            }

            record_applied_ontology_candidate(
                state,
                &sync_run,
                &trimmed_prompt,
                SyncTargetKind::Relationship,
                SyncActionKind::Create,
                "Narrative relationship",
                "Applied relationship change from narrative turn",
                &record_applied_candidate_payload(&relationship)?,
                "ontology_relationship",
                Some(relationship_id),
            )?;
        }

        match sync::execute(state, McpToolCall::FinishSyncRun { sync_run })? {
            McpToolResult::Empty => {}
            _ => return Err("finish sync run tool returned an unexpected result".to_string()),
        }
    }

    let working_memory = update_working_memory(state, &plan.intent, &trimmed_prompt, &committed)?;

    let reply_title = reply_title(&plan.intent, &committed);
    let reply_body = summarize_commit(&plan.intent, &committed);

    Ok(NarrativeTurnOutcome {
        intent: plan.intent,
        capabilities: plan.capabilities,
        write_policy: plan.write_policy,
        reply_title,
        reply_body,
        committed,
        working_memory,
    })
}

fn update_working_memory(
    state: &AppState,
    intent: &AssistantIntent,
    prompt: &str,
    preview: &NarrativeMessagePreview,
) -> Result<WorkingMemory, String> {
    let Some(repository) = state.sqlite_repository.as_ref() else {
        return Ok(build_memory_snapshot(
            WorkingMemory::default(),
            intent,
            prompt,
            preview,
        ));
    };

    let memory = repository
        .load_working_memory("current-project", "main")
        .map_err(|err| err.to_string())?;
    let memory = build_memory_snapshot(memory, intent, prompt, preview);
    repository
        .save_working_memory(memory)
        .map_err(|err| err.to_string())
}

fn build_memory_snapshot(
    mut memory: WorkingMemory,
    intent: &AssistantIntent,
    prompt: &str,
    preview: &NarrativeMessagePreview,
) -> WorkingMemory {
    let focus_summary = preview
        .changes
        .first()
        .map(|change| change.label.clone())
        .unwrap_or_else(|| prompt.trim().to_string());
    let focus_kind = match intent {
        AssistantIntent::Query => FocusKind::OpenQuestion,
        AssistantIntent::Guide => FocusKind::Structure,
        _ => match preview.suggested_target {
            NarrativeCommitTarget::Character => FocusKind::Character,
            NarrativeCommitTarget::Event => FocusKind::Event,
        },
    };

    memory.current_focus = Some(FocusItem {
        kind: focus_kind,
        summary: focus_summary.clone(),
        related_refs: preview
            .changes
            .iter()
            .map(|change| change.label.clone())
            .collect(),
    });

    memory.pinned_decisions = if matches!(intent, AssistantIntent::MutateOntology) {
        preview
            .changes
            .iter()
            .enumerate()
            .map(|(index, change)| crate::domain::PinnedDecision {
                id: format!("decision-{}", index + 1),
                summary: format!("{}: {}", summarize_change_kind(&change.kind), change.label),
                related_refs: vec![change.label.clone()],
                source_ref: Some(prompt.to_string()),
            })
            .collect()
    } else {
        memory.pinned_decisions
    };

    memory.open_questions = if matches!(intent, AssistantIntent::Query | AssistantIntent::Clarify) {
        vec![crate::domain::OpenQuestion {
            id: "current-question".to_string(),
            question: if matches!(intent, AssistantIntent::Clarify) {
                "What should we define first: a character, an opening event, or the central conflict?"
                    .to_string()
            } else {
                prompt.to_string()
            },
            related_refs: preview
                .changes
                .iter()
                .map(|change| change.label.clone())
                .collect(),
            priority: 1,
        }]
    } else {
        memory.open_questions
    };

    memory.last_tool_actions = vec![ToolActionRecord {
        tool_name: match intent {
            AssistantIntent::MutateOntology => "ontology.apply_preview".to_string(),
            AssistantIntent::MutateDocument => "document.apply_change".to_string(),
            AssistantIntent::ProposeSync => "sync.propose_candidate".to_string(),
            AssistantIntent::ResolveLint => "lint.resolve".to_string(),
            AssistantIntent::Query => "ontology.query".to_string(),
            AssistantIntent::Guide => "assistant.guide".to_string(),
            AssistantIntent::Clarify => "assistant.clarify".to_string(),
        },
        summary: focus_summary,
        related_refs: preview
            .changes
            .iter()
            .map(|change| change.label.clone())
            .collect(),
    }];
    memory.updated_at = chrono::Utc::now();
    memory
}

fn summarize_change_kind(kind: &NarrativeChangeKind) -> &'static str {
    match kind {
        NarrativeChangeKind::AddCharacter => "Add character",
        NarrativeChangeKind::UpdateCharacter => "Update character",
        NarrativeChangeKind::AddEvent => "Add event",
        NarrativeChangeKind::UpdateEvent => "Update event",
        NarrativeChangeKind::AddRelationship => "Add relationship",
        NarrativeChangeKind::UpdateRelationship => "Update relationship",
    }
}

fn record_applied_ontology_candidate(
    state: &AppState,
    sync_run: &crate::domain::SyncRun,
    prompt: &str,
    target_kind: SyncTargetKind,
    action_kind: SyncActionKind,
    title: &str,
    summary: &str,
    payload_json: &str,
    derived_kind: &str,
    derived_ref: Option<String>,
) -> Result<(), String> {
    match sync::execute(
        state,
        McpToolCall::RecordAppliedOntologyCandidate {
            sync_run_id: sync_run.id,
            prompt: prompt.to_string(),
            target_kind,
            action_kind,
            title: title.to_string(),
            summary: summary.to_string(),
            payload_json: payload_json.to_string(),
            derived_kind: derived_kind.to_string(),
            derived_ref,
        },
    )? {
        McpToolResult::Empty => Ok(()),
        _ => Err("record applied candidate tool returned an unexpected result".to_string()),
    }
}

fn reply_title(intent: &AssistantIntent, preview: &NarrativeMessagePreview) -> String {
    match intent {
        AssistantIntent::Query => "Question received".to_string(),
        AssistantIntent::Guide => "Guidance".to_string(),
        AssistantIntent::Clarify => "Need one concrete anchor".to_string(),
        _ => format!(
            "Applied as {}",
            match preview.suggested_target {
                NarrativeCommitTarget::Character => "character",
                NarrativeCommitTarget::Event => "event",
            }
        ),
    }
}

fn summarize_commit(intent: &AssistantIntent, preview: &NarrativeMessagePreview) -> String {
    if let Some((subject, object, relationship)) = summarize_named_relationship(&preview.prompt) {
        return match intent {
            AssistantIntent::MutateOntology => format!(
                "I’ve noted that {subject} is {object}'s {relationship}. If you want, I can flesh out {object} as a full character next."
            ),
            AssistantIntent::Clarify => format!(
                "I read that as a relationship: {subject} is {object}'s {relationship}. If you want, I can turn {object} into a full character next."
            ),
            _ => format!(
                "I read that as a relationship between {subject} and {object}. If you want, I can flesh out both characters next."
            ),
        };
    }

    if matches!(intent, AssistantIntent::Query) {
        if preview.changes.is_empty() {
            return "I’m treating that as a question, so I did not change the narrative model yet.".to_string();
        }

        return format!(
            "I’m treating that as a question for now. If you want, I can commit these possible changes next: {}",
            preview
                .changes
                .iter()
                .map(|change| change.label.clone())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if matches!(intent, AssistantIntent::Guide) {
        if preview.changes.is_empty() {
            return "I did not infer a direct structural change from that message. I’m keeping the current focus in working memory and waiting for a clearer instruction.".to_string();
        }

        return format!(
            "I found possible structure here, but I’m treating this as guidance rather than a committed change: {}",
            preview
                .changes
                .iter()
                .map(|change| format!("{} ({})", change.label, summarize_change_kind(&change.kind)))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    if matches!(intent, AssistantIntent::Clarify) {
        return "I need one concrete anchor before I commit anything. Start with a character, an opening event, or the central conflict, and I’ll build the structure from there.".to_string();
    }

    if preview.changes.is_empty() {
        return "I did not infer a concrete structural change from that message.".to_string();
    }

    let mut lines = preview
        .changes
        .iter()
        .map(|change| {
            let label = match change.kind {
                NarrativeChangeKind::AddCharacter => "Added character",
                NarrativeChangeKind::UpdateCharacter => "Updated character",
                NarrativeChangeKind::AddEvent => "Added event",
                NarrativeChangeKind::UpdateEvent => "Updated event",
                NarrativeChangeKind::AddRelationship => "Added relationship",
                NarrativeChangeKind::UpdateRelationship => "Updated relationship",
            };
            format!("{label}: {}. {}", change.label, change.detail)
        })
        .collect::<Vec<_>>();

    if !preview.relationships.is_empty() {
        lines.push(format!(
            "Tracked {} relationship{} in the narrative model.",
            preview.relationships.len(),
            if preview.relationships.len() == 1 { "" } else { "s" }
        ));
    }

    lines.join(" ")
}

fn summarize_named_relationship(prompt: &str) -> Option<(String, String, String)> {
    let words = prompt.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() {
        return None;
    }

    let subject = words.first().and_then(|token| title_like_name(token))?;
    let lowered = prompt.to_lowercase();

    if let Some(index) = lowered.find("'s brother") {
        let owner = prompt[..index]
            .trim()
            .split_whitespace()
            .last()
            .and_then(title_like_name)?;
        return Some((subject, owner, "brother".to_string()));
    }

    if let Some(index) = lowered.find("'s sister") {
        let owner = prompt[..index]
            .trim()
            .split_whitespace()
            .last()
            .and_then(title_like_name)?;
        return Some((subject, owner, "sister".to_string()));
    }

    None
}

fn title_like_name(token: &str) -> Option<String> {
    let cleaned = token.trim_matches(|character: char| !character.is_alphanumeric() && character != '\'' && character != '-');
    let starts_uppercase = cleaned
        .chars()
        .next()
        .map(|character| character.is_ascii_uppercase())
        .unwrap_or(false);

    if cleaned.is_empty() || !starts_uppercase {
        None
    } else {
        Some(cleaned.to_string())
    }
}
