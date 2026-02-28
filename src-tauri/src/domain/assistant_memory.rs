use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssistantIntent {
    Query,
    Guide,
    Clarify,
    MutateOntology,
    MutateDocument,
    ProposeSync,
    ResolveLint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssistantCapability {
    UnderstandTurn,
    InspectProjectState,
    ExtractStructure,
    CommitStructure,
    ProposeDocumentChange,
    InspectAlignment,
    ResolveAmbiguity,
    GuideNextStep,
    ResolveLint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum WritePolicy {
    NoWrite,
    CandidateOnly,
    SafeCommit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FocusKind {
    Character,
    Event,
    Relationship,
    Scene,
    Structure,
    LintResolution,
    OpenQuestion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusItem {
    pub kind: FocusKind,
    pub summary: String,
    pub related_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenQuestion {
    pub id: String,
    pub question: String,
    pub related_refs: Vec<String>,
    pub priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedDecision {
    pub id: String,
    pub summary: String,
    pub related_refs: Vec<String>,
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveAssumption {
    pub id: String,
    pub summary: String,
    pub confidence: f32,
    pub related_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentCorrection {
    pub id: String,
    pub summary: String,
    pub corrected_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolActionRecord {
    pub tool_name: String,
    pub summary: String,
    pub related_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemory {
    pub project_id: String,
    pub session_id: String,
    pub current_focus: Option<FocusItem>,
    pub open_questions: Vec<OpenQuestion>,
    pub pinned_decisions: Vec<PinnedDecision>,
    pub active_assumptions: Vec<ActiveAssumption>,
    pub recent_corrections: Vec<RecentCorrection>,
    pub last_tool_actions: Vec<ToolActionRecord>,
    pub updated_at: DateTime<Utc>,
}

impl Default for WorkingMemory {
    fn default() -> Self {
        Self {
            project_id: "current-project".to_string(),
            session_id: "main".to_string(),
            current_focus: None,
            open_questions: Vec::new(),
            pinned_decisions: Vec::new(),
            active_assumptions: Vec::new(),
            recent_corrections: Vec::new(),
            last_tool_actions: Vec::new(),
            updated_at: Utc::now(),
        }
    }
}
