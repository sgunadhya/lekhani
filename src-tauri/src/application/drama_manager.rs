use crate::application::TurnRoute;
use crate::domain::{
    BeatId, EvaluationResult, InteractionState, NarrativeMode, NarrativeSuggestionAction,
    ThreadScope, ThreadStatus,
};

pub trait NarrativeEngine: Send + Sync {
    fn evaluate(&self, state: &InteractionState) -> EvaluationResult;
}

pub struct DeterministicNarrativeEngine;

impl NarrativeEngine for DeterministicNarrativeEngine {
    fn evaluate(&self, state: &InteractionState) -> EvaluationResult {
        if let Some(result) = candidate_ready(state) {
            return result;
        }

        if let Some(result) = ready_to_commit(state) {
            return result;
        }

        if let Some(result) = sidequest_closeable(state) {
            return result;
        }

        if let Some(result) = sidequest_opened(state) {
            return result;
        }

        if let Some(result) = drift_detected(state) {
            return result;
        }

        if let Some(result) = stalled(state) {
            return result;
        }

        EvaluationResult {
            mode: state.current_mode.clone(),
            beat: None,
            actions: default_actions(state),
            nudge: None,
        }
    }
}

fn candidate_ready(state: &InteractionState) -> Option<EvaluationResult> {
    if !matches!(state.current_mode, NarrativeMode::Brainstorming)
        || state.current_candidate.is_none()
        || state.turn_count < 2
        || state.last_interpretation_confidence < 0.35
        || matches!(
            state.last_turn_route,
            TurnRoute::ConfirmCurrent | TurnRoute::AddToScreenplay
        )
    {
        return None;
    }

    Some(EvaluationResult {
        mode: NarrativeMode::Converging,
        beat: Some(BeatId::CandidateReady),
        actions: vec![
            NarrativeSuggestionAction::UseThis,
            NarrativeSuggestionAction::TryAnother,
            NarrativeSuggestionAction::ExpandThis,
            NarrativeSuggestionAction::ParkThread,
        ],
        nudge: Some("This idea is taking shape. Decide whether to keep it, deepen it, or try another angle.".to_string()),
    })
}

fn ready_to_commit(state: &InteractionState) -> Option<EvaluationResult> {
    if state.current_candidate.is_none()
        || !matches!(
            state.last_turn_route,
            TurnRoute::ConfirmCurrent | TurnRoute::AddToScreenplay
        )
        || state.last_interpretation_confidence < 0.5
    {
        return None;
    }

    let actions = if matches!(state.current_mode, NarrativeMode::TunnelingSidequest) {
        vec![
            NarrativeSuggestionAction::CommitSidequest,
            NarrativeSuggestionAction::ResumeThread,
        ]
    } else {
        vec![
            NarrativeSuggestionAction::UseThis,
            NarrativeSuggestionAction::AddToScreenplay,
            NarrativeSuggestionAction::ExpandThis,
            NarrativeSuggestionAction::ParkThread,
        ]
    };

    Some(EvaluationResult {
        mode: NarrativeMode::Committing,
        beat: Some(BeatId::ReadyToCommit),
        actions,
        nudge: Some("You have a concrete direction. Commit it or draft it into the screenplay.".to_string()),
    })
}

fn drift_detected(state: &InteractionState) -> Option<EvaluationResult> {
    if !matches!(state.thread_status, ThreadStatus::Drifting)
        && !(matches!(state.last_interpretation_target, crate::application::InterpretationTarget::General)
            && matches!(state.last_turn_route, TurnRoute::Continue)
            && state.last_interpretation_confidence < 0.3
            && state.turn_count > 2)
    {
        return None;
    }

    Some(EvaluationResult {
        mode: NarrativeMode::Drifting,
        beat: Some(BeatId::DriftDetected),
        actions: default_actions(state),
        nudge: Some("The thread is drifting. Return to the current idea or deliberately switch focus.".to_string()),
    })
}

fn stalled(state: &InteractionState) -> Option<EvaluationResult> {
    if !matches!(state.thread_status, ThreadStatus::Stalled) {
        return None;
    }

    Some(EvaluationResult {
        mode: NarrativeMode::Brainstorming,
        beat: Some(BeatId::Stalled),
        actions: vec![
            NarrativeSuggestionAction::TryAnother,
            NarrativeSuggestionAction::ExpandThis,
        ],
        nudge: Some("This thread has stalled. Try a new angle or deepen the current one.".to_string()),
    })
}

fn sidequest_opened(state: &InteractionState) -> Option<EvaluationResult> {
    if !matches!(state.current_thread_scope, ThreadScope::Sidequest) || !state.has_return_thread {
        return None;
    }

    Some(EvaluationResult {
        mode: NarrativeMode::TunnelingSidequest,
        beat: Some(BeatId::SidequestOpened),
        actions: vec![
            NarrativeSuggestionAction::ExpandThis,
            NarrativeSuggestionAction::CommitSidequest,
            NarrativeSuggestionAction::ResumeThread,
        ],
        nudge: Some("A side thread is open. Explore it briefly, then decide whether to return or commit it.".to_string()),
    })
}

fn sidequest_closeable(state: &InteractionState) -> Option<EvaluationResult> {
    if !matches!(state.current_thread_scope, ThreadScope::Sidequest)
        || !state.has_return_thread
        || !matches!(
            state.last_turn_route,
            TurnRoute::ConfirmCurrent | TurnRoute::AddToScreenplay
        )
    {
        return None;
    }

    Some(EvaluationResult {
        mode: NarrativeMode::Converging,
        beat: Some(BeatId::SidequestCloseable),
        actions: vec![
            NarrativeSuggestionAction::CommitSidequest,
            NarrativeSuggestionAction::ResumeThread,
        ],
        nudge: Some("This side thread looks ready to resolve. Commit it or set it aside.".to_string()),
    })
}

fn default_actions(state: &InteractionState) -> Vec<NarrativeSuggestionAction> {
    match state.current_thread_scope {
        ThreadScope::Sidequest => {
            let mut actions = if matches!(state.last_turn_route, TurnRoute::ConfirmCurrent) {
                vec![
                    NarrativeSuggestionAction::CommitSidequest,
                    NarrativeSuggestionAction::ExpandThis,
                ]
            } else {
                vec![
                    NarrativeSuggestionAction::ExpandThis,
                    NarrativeSuggestionAction::CommitSidequest,
                ]
            };
            if state.has_return_thread {
                actions.push(NarrativeSuggestionAction::ResumeThread);
            }
            actions
        }
        ThreadScope::Main => {
            let mut actions = if state.current_candidate.is_some() {
                let mut actions = vec![
                    NarrativeSuggestionAction::UseThis,
                    NarrativeSuggestionAction::TryAnother,
                    NarrativeSuggestionAction::ExpandThis,
                    NarrativeSuggestionAction::AddToScreenplay,
                    NarrativeSuggestionAction::ParkThread,
                ];

                if matches!(state.last_turn_route, TurnRoute::ConfirmCurrent) {
                    actions.swap(0, 3);
                    actions.swap(1, 3);
                }

                actions
            } else {
                Vec::new()
            };

            if state.open_sidequests > 0 {
                actions.push(NarrativeSuggestionAction::ResumeThread);
            }

            actions
        }
    }
}

pub fn derive_thread_status(
    current_mode: &NarrativeMode,
    has_candidate: bool,
    turn_count: u32,
    last_turn_route: &TurnRoute,
) -> ThreadStatus {
    if matches!(current_mode, NarrativeMode::Drifting) {
        return ThreadStatus::Drifting;
    }

    if has_candidate
        && matches!(last_turn_route, TurnRoute::ConfirmCurrent | TurnRoute::AddToScreenplay)
    {
        return ThreadStatus::Converging;
    }

    if !has_candidate && turn_count >= 4 {
        return ThreadStatus::Stalled;
    }

    ThreadStatus::Active
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_state() -> InteractionState {
        InteractionState {
            current_candidate: Some("Ancient Mathura".to_string()),
            current_mode: NarrativeMode::Brainstorming,
            thread_status: ThreadStatus::Active,
            current_thread_scope: ThreadScope::Main,
            has_return_thread: false,
            turn_count: 2,
            last_interpretation_target: crate::application::InterpretationTarget::General,
            last_turn_route: crate::application::TurnRoute::Continue,
            last_interpretation_confidence: 1.0,
            open_sidequests: 0,
        }
    }

    #[test]
    fn candidate_ready_surfaces_commit_actions() {
        let engine = DeterministicNarrativeEngine;
        let result = engine.evaluate(&base_state());
        assert_eq!(result.beat, Some(BeatId::CandidateReady));
        assert!(result.actions.contains(&NarrativeSuggestionAction::UseThis));
    }

    #[test]
    fn ready_to_commit_has_add_to_screenplay() {
        let engine = DeterministicNarrativeEngine;
        let mut state = base_state();
        state.last_turn_route = TurnRoute::ConfirmCurrent;
        let result = engine.evaluate(&state);
        assert_eq!(result.beat, Some(BeatId::ReadyToCommit));
        assert!(result
            .actions
            .contains(&NarrativeSuggestionAction::AddToScreenplay));
    }

    #[test]
    fn sidequest_opened_surfaces_resume_and_commit() {
        let engine = DeterministicNarrativeEngine;
        let mut state = base_state();
        state.current_thread_scope = ThreadScope::Sidequest;
        state.has_return_thread = true;
        let result = engine.evaluate(&state);
        assert_eq!(result.beat, Some(BeatId::SidequestOpened));
        assert!(result.actions.contains(&NarrativeSuggestionAction::ResumeThread));
        assert!(result.actions.contains(&NarrativeSuggestionAction::CommitSidequest));
    }
}
