use crate::domain::{
    AppError, ConflictKind, LintFinding, SyncCandidate, SyncRun, SyncSourceKind,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncSource {
    NarrativeTurn { turn_id: String, prompt: String },
    EditorChange { scene_id: Option<String>, diff_id: String },
    OntologySuggestion { candidate_id: String },
    LintResolution { finding_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncRunOutcome {
    Completed,
    Failed { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResolutionDecision {
    AutoApply,
    QueueAsSuggestion,
    MarkConflicted(ConflictKind),
    Reject(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppliedEffect {
    pub applied_ids: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResolutionContext {
    pub document_version: Option<u64>,
    pub ontology_version: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LintContext {
    pub document_version: Option<u64>,
    pub ontology_version: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncSummary {
    pub applied_count: usize,
    pub suggested_count: usize,
    pub conflicted_count: usize,
}

pub trait SyncCoordinator: Send + Sync {
    fn begin_run(&self, source: SyncSource) -> Result<SyncRun, AppError>;
    fn derive_candidates(&self, run: &SyncRun) -> Result<Vec<SyncCandidate>, AppError>;
    fn finalize_run(&self, run_id: String, outcome: SyncRunOutcome) -> Result<(), AppError>;
}

pub trait CandidateResolver: Send + Sync {
    fn evaluate(
        &self,
        candidate: &SyncCandidate,
        ctx: &ResolutionContext,
    ) -> Result<ResolutionDecision, AppError>;

    fn apply(
        &self,
        candidate: &SyncCandidate,
        ctx: &ResolutionContext,
    ) -> Result<AppliedEffect, AppError>;
}

pub trait DocumentExtractor: Send + Sync {
    fn derive_from_document(&self, source_ref: &str) -> Result<Vec<SyncCandidate>, AppError>;
}

pub trait SyncResolver: Send + Sync {
    fn resolve_candidate(
        &self,
        candidate: &SyncCandidate,
        ctx: &ResolutionContext,
    ) -> Result<ResolutionDecision, AppError>;
}

pub trait EntityMatcher: Send + Sync {
    fn match_character(&self, text: &str) -> Result<Option<String>, AppError>;
    fn match_event(&self, text: &str) -> Result<Option<String>, AppError>;
}

pub trait TimelineReasoner: Send + Sync {
    fn evaluate_timeline(&self, ontology_version: Option<u64>) -> Result<SyncSummary, AppError>;
}

pub trait LintEngine: Send + Sync {
    fn run_after_commit(&self, ctx: &LintContext) -> Result<Vec<LintFinding>, AppError>;
}

impl From<&SyncSource> for SyncSourceKind {
    fn from(value: &SyncSource) -> Self {
        match value {
            SyncSource::NarrativeTurn { .. } => Self::NarrativeChat,
            SyncSource::EditorChange { .. } => Self::ScreenplayExtraction,
            SyncSource::OntologySuggestion { .. } => Self::OntologySuggestion,
            SyncSource::LintResolution { .. } => Self::LintResolution,
        }
    }
}
