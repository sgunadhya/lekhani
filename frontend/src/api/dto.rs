use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChangeTypeDto {
    Insert,
    Delete,
    Replace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenplayChangeDto {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub author: String,
    pub change_type: ChangeTypeDto,
    pub range_start: usize,
    pub range_end: usize,
    pub new_text: String,
    pub old_text: String,
    pub provenance: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenplayDto {
    pub id: Uuid,
    pub title: String,
    pub fountain_text: String,
    pub version: u64,
    pub changes: Vec<ScreenplayChangeDto>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveScreenplayRequest {
    pub screenplay: ScreenplayDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentFileDto {
    pub screenplay: ScreenplayDto,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveDocumentRequest {
    pub screenplay: ScreenplayDto,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseDescriptionRequest {
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NarrativeCommitTargetDto {
    Character,
    Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NarrativeChangeKindDto {
    AddCharacter,
    UpdateCharacter,
    AddEvent,
    UpdateEvent,
    AddRelationship,
    UpdateRelationship,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeChangeSummaryDto {
    pub kind: NarrativeChangeKindDto,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewNarrativeInputDto {
    pub prompt: String,
    pub suggested_target: NarrativeCommitTargetDto,
    pub character: Option<NarrativeCharacterDto>,
    pub event: Option<NarrativeEventDto>,
    pub relationships: Vec<OntologyRelationshipDto>,
    pub changes: Vec<NarrativeChangeSummaryDto>,
    pub reply_title: Option<String>,
    pub reply_body: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitNarrativeInputRequest {
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssistantIntentDto {
    Query,
    Guide,
    Clarify,
    MutateOntology,
    MutateDocument,
    ProposeSync,
    ResolveLint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AssistantCapabilityDto {
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
pub enum WritePolicyDto {
    NoWrite,
    CandidateOnly,
    SafeCommit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConversationModeDto {
    Brainstorming,
    Refining,
    Committing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConversationTopicDto {
    Setting,
    Character,
    Event,
    Relationship,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FocusKindDto {
    Character,
    Event,
    Relationship,
    Scene,
    Structure,
    LintResolution,
    OpenQuestion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusItemDto {
    pub kind: FocusKindDto,
    pub summary: String,
    pub related_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenQuestionDto {
    pub id: String,
    pub question: String,
    pub related_refs: Vec<String>,
    pub priority: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinnedDecisionDto {
    pub id: String,
    pub summary: String,
    pub related_refs: Vec<String>,
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveAssumptionDto {
    pub id: String,
    pub summary: String,
    pub confidence: f32,
    pub related_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentCorrectionDto {
    pub id: String,
    pub summary: String,
    pub corrected_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConstraintScopeDto {
    Setting,
    Character,
    Event,
    Relationship,
    Tone,
    Structure,
    General,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConstraintOperatorDto {
    Avoid,
    Prefer,
    Require,
    Forbid,
    Correct,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConstraintStatusDto {
    Active,
    Satisfied,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintDto {
    pub id: String,
    pub scope: ConstraintScopeDto,
    pub operator: ConstraintOperatorDto,
    pub value: String,
    pub source: String,
    pub status: ConstraintStatusDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatusDto {
    Open,
    InProgress,
    Resolved,
    Blocked,
    Dismissed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskCategoryDto {
    Structure,
    Character,
    Event,
    Relationship,
    Alignment,
    Lint,
    Drafting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryTaskDto {
    pub id: String,
    pub description: String,
    pub priority: u8,
    pub status: TaskStatusDto,
    pub category: TaskCategoryDto,
    pub related_refs: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolActionRecordDto {
    pub tool_name: String,
    pub summary: String,
    pub related_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemoryDto {
    pub project_id: String,
    pub session_id: String,
    pub conversation_mode: ConversationModeDto,
    pub conversation_topic: ConversationTopicDto,
    pub current_focus: Option<FocusItemDto>,
    pub constraints: Vec<ConstraintDto>,
    pub story_backlog: Vec<StoryTaskDto>,
    pub open_questions: Vec<OpenQuestionDto>,
    pub pinned_decisions: Vec<PinnedDecisionDto>,
    pub active_assumptions: Vec<ActiveAssumptionDto>,
    pub recent_corrections: Vec<RecentCorrectionDto>,
    pub last_tool_actions: Vec<ToolActionRecordDto>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantTurnDto {
    pub intent: AssistantIntentDto,
    pub capabilities: Vec<AssistantCapabilityDto>,
    pub write_policy: WritePolicyDto,
    pub reply_title: String,
    pub reply_body: String,
    pub committed: PreviewNarrativeInputDto,
    pub working_memory: WorkingMemoryDto,
    pub suggested_actions: Vec<NarrativeSuggestedActionViewDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NarrativeSuggestionActionDto {
    UseThis,
    TryAnother,
    ExpandThis,
    AddToScreenplay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeSuggestedActionViewDto {
    pub action: NarrativeSuggestionActionDto,
    pub label: String,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncSourceKindDto {
    NarrativeChat,
    ScreenplayExtraction,
    OntologySuggestion,
    LintResolution,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncRunStatusDto {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRunDto {
    pub id: Uuid,
    pub source_kind: SyncSourceKindDto,
    pub source_ref: Option<String>,
    pub document_version: Option<u64>,
    pub ontology_version: Option<u64>,
    pub status: SyncRunStatusDto,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncTargetLayerDto {
    Document,
    Ontology,
    Link,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncTargetKindDto {
    Character,
    Event,
    Relationship,
    ScreenplayPatch,
    DocumentMetadata,
    Link,
    LintFinding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncActionKindDto {
    Create,
    Update,
    Merge,
    Relink,
    Patch,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CandidateStatusDto {
    Draft,
    Ready,
    Applied,
    Rejected,
    Superseded,
    Expired,
    Conflicted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCandidateDto {
    pub id: Uuid,
    pub sync_run_id: Uuid,
    pub source_kind: SyncSourceKindDto,
    pub source_ref: Option<String>,
    pub target_layer: SyncTargetLayerDto,
    pub target_kind: SyncTargetKindDto,
    pub action_kind: SyncActionKindDto,
    pub status: CandidateStatusDto,
    pub confidence: Option<f32>,
    pub title: String,
    pub summary: String,
    pub payload_json: String,
    pub evidence_json: Option<String>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceRecordDto {
    pub id: Uuid,
    pub sync_run_id: Uuid,
    pub source_kind: SyncSourceKindDto,
    pub source_ref: Option<String>,
    pub derived_kind: String,
    pub derived_ref: String,
    pub confidence: Option<f32>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDebugDto {
    pub runs: Vec<SyncRunDto>,
    pub pending_candidates: Vec<SyncCandidateDto>,
    pub recent_provenance: Vec<ProvenanceRecordDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStatusDto {
    pub backend: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeCharacterDto {
    pub id: Uuid,
    pub ontology_entity_id: Option<Uuid>,
    pub name: String,
    pub summary: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeEventDto {
    pub id: Uuid,
    pub ontology_entity_id: Option<Uuid>,
    pub title: String,
    pub summary: String,
    pub participants: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeNudgeDto {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NarrativeMetricsDto {
    pub scene_count: usize,
    pub character_count: usize,
    pub event_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NarrativeSnapshotDto {
    pub characters: Vec<NarrativeCharacterDto>,
    pub events: Vec<NarrativeEventDto>,
    pub projection_relationships: Vec<OntologyRelationshipDto>,
    pub ontology_graph: OntologyGraphDto,
    pub metrics: NarrativeMetricsDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OntologyEntityKindDto {
    Character,
    Event,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyEntityDto {
    pub id: Uuid,
    pub kind: OntologyEntityKindDto,
    pub label: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OntologyRelationshipKindDto {
    NarrativeProjection,
    ParticipantInEvent,
    SupportsCharacter,
    OpposesCharacter,
    AdvisesCharacter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OntologyRelationshipDto {
    pub id: Uuid,
    pub source_id: Uuid,
    pub target_id: Uuid,
    pub kind: OntologyRelationshipKindDto,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OntologyGraphDto {
    pub entities: Vec<OntologyEntityDto>,
    pub relationships: Vec<OntologyRelationshipDto>,
}
