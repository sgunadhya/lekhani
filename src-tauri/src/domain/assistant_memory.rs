use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::domain::narrative_engine::{ThreadScope, ThreadStatus};

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
pub enum ConversationMode {
    Brainstorming,
    Refining,
    Committing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConversationTopic {
    Setting,
    Character,
    Event,
    Relationship,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NarrativeThreadStatus {
    Active,
    Parked,
    Committed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NarrativeSuggestionAction {
    UseThis,
    TryAnother,
    ExpandThis,
    AddToScreenplay,
    ParkThread,
    ResumeThread,
    CommitSidequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeSuggestedAction {
    pub action: NarrativeSuggestionAction,
    pub label: String,
    pub primary: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConstraintScope {
    Setting,
    Character,
    Event,
    Relationship,
    Tone,
    Structure,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConstraintOperator {
    Avoid,
    Prefer,
    Require,
    Forbid,
    Correct,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConstraintStatus {
    Active,
    Satisfied,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constraint {
    pub id: String,
    pub scope: ConstraintScope,
    pub operator: ConstraintOperator,
    pub value: String,
    pub source: String,
    pub status: ConstraintStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolActionRecord {
    pub tool_name: String,
    pub summary: String,
    pub related_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Open,
    InProgress,
    Resolved,
    Blocked,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskCategory {
    Structure,
    Character,
    Event,
    Relationship,
    Alignment,
    Lint,
    Drafting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryTask {
    pub id: String,
    pub description: String,
    pub priority: u8, // 1: High, 5: Low
    pub status: TaskStatus,
    pub category: TaskCategory,
    pub related_refs: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeThread {
    pub id: String,
    pub goal: String,
    pub status: NarrativeThreadStatus,
    #[serde(default)]
    pub thread_status: ThreadStatus,
    #[serde(default)]
    pub scope: ThreadScope,
    #[serde(default)]
    pub return_to_thread_id: Option<String>,
    pub topic: ConversationTopic,
    pub current_focus: Option<FocusItem>,
    pub open_questions: Vec<OpenQuestion>,
    pub turn_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemory {
    pub project_id: String,
    pub session_id: String,
    pub turn_count: u32,
    pub conversation_mode: ConversationMode,
    pub conversation_topic: ConversationTopic,
    pub current_focus: Option<FocusItem>,
    pub current_thread: NarrativeThread,
    pub return_thread: Option<NarrativeThread>,
    pub sidequests: Vec<NarrativeThread>,
    pub constraints: Vec<Constraint>,
    pub story_backlog: Vec<StoryTask>,
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
            turn_count: 0,
            conversation_mode: ConversationMode::Brainstorming,
            conversation_topic: ConversationTopic::General,
            current_focus: None,
            current_thread: NarrativeThread {
                id: "main-thread".to_string(),
                goal: "Shape the story".to_string(),
                status: NarrativeThreadStatus::Active,
                thread_status: ThreadStatus::Active,
                scope: ThreadScope::Main,
                return_to_thread_id: None,
                topic: ConversationTopic::General,
                current_focus: None,
                open_questions: Vec::new(),
                turn_count: 0,
            },
            return_thread: None,
            sidequests: Vec::new(),
            constraints: Vec::new(),
            story_backlog: Vec::new(),
            open_questions: Vec::new(),
            pinned_decisions: Vec::new(),
            active_assumptions: Vec::new(),
            recent_corrections: Vec::new(),
            last_tool_actions: Vec::new(),
            updated_at: Utc::now(),
        }
    }
}
