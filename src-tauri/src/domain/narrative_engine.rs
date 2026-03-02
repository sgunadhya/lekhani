use crate::application::{InterpretationTarget, TurnRoute};
use crate::domain::NarrativeSuggestionAction;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NarrativeMode {
    Idle,
    Brainstorming,
    Converging,
    Elaborating,
    Committing,
    TunnelingSidequest,
    Drifting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadStatus {
    Active,
    Drifting,
    Converging,
    Stalled,
}

impl Default for ThreadStatus {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadScope {
    Main,
    Sidequest,
}

impl Default for ThreadScope {
    fn default() -> Self {
        Self::Main
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BeatId {
    CandidateReady,
    DriftDetected,
    ReadyToCommit,
    Stalled,
    SidequestOpened,
    SidequestCloseable,
}

#[derive(Debug, Clone)]
pub struct InteractionState {
    pub current_candidate: Option<String>,
    pub current_mode: NarrativeMode,
    pub thread_status: ThreadStatus,
    pub current_thread_scope: ThreadScope,
    pub has_return_thread: bool,
    pub turn_count: u32,
    pub last_interpretation_target: InterpretationTarget,
    pub last_turn_route: TurnRoute,
    pub last_interpretation_confidence: f32,
    pub open_sidequests: usize,
}

#[derive(Debug, Clone)]
pub struct EvaluationResult {
    pub mode: NarrativeMode,
    pub beat: Option<BeatId>,
    pub actions: Vec<NarrativeSuggestionAction>,
    pub nudge: Option<String>,
}
