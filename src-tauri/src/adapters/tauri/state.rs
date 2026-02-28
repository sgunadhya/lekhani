use crate::adapters::db::SqliteScreenplayRepository;
use crate::application::{
    AssistantCapabilityPlanner, AssistantIntentClassifier, MutationGate, NarrativeService,
    ScreenplayService,
};
use crate::domain::{NarrativeCharacter, NarrativeEvent, NarrativeSnapshot, OntologyRelationship};
use crate::ports::{
    CharacterParser, EventParser, NarrativeRepository, NudgeGenerator, ScreenplayRepository,
};
use std::path::Path;
use std::sync::Arc;

pub struct AppState {
    pub screenplay_service: ScreenplayService<Box<dyn ScreenplayRepository>>,
    pub narrative_service: NarrativeService<
        Box<dyn CharacterParser>,
        Box<dyn EventParser>,
        Box<dyn NudgeGenerator>,
    >,
    pub assistant_intent_classifier: Box<dyn AssistantIntentClassifier>,
    pub assistant_capability_planner: Box<dyn AssistantCapabilityPlanner>,
    pub mutation_gate: Box<dyn MutationGate>,
    pub narrative_repository: Box<dyn NarrativeRepository>,
    pub sqlite_repository: Option<Arc<SqliteScreenplayRepository>>,
    pub llm_backend: String,
    pub llm_detail: String,
}

impl AppState {
    pub fn new(
        screenplay_repository: Box<dyn ScreenplayRepository>,
        narrative_repository: Box<dyn NarrativeRepository>,
        sqlite_repository: Option<Arc<SqliteScreenplayRepository>>,
        llm_backend: String,
        llm_detail: String,
        assistant_intent_classifier: Box<dyn AssistantIntentClassifier>,
        assistant_capability_planner: Box<dyn AssistantCapabilityPlanner>,
        mutation_gate: Box<dyn MutationGate>,
        character_parser: Box<dyn CharacterParser>,
        event_parser: Box<dyn EventParser>,
        nudge_generator: Box<dyn NudgeGenerator>,
    ) -> Self {
        Self {
            screenplay_service: ScreenplayService::new(screenplay_repository),
            narrative_service: NarrativeService::new(
                character_parser,
                event_parser,
                nudge_generator,
            ),
            assistant_intent_classifier,
            assistant_capability_planner,
            mutation_gate,
            narrative_repository,
            sqlite_repository,
            llm_backend,
            llm_detail,
        }
    }
}

impl AppState {
    pub fn store_character(&self, character: NarrativeCharacter) -> Result<(), String> {
        self.narrative_repository
            .save_character(character)
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    pub fn store_event(&self, event: NarrativeEvent) -> Result<(), String> {
        self.narrative_repository
            .save_event(event)
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    pub fn get_snapshot(&self) -> Result<NarrativeSnapshot, String> {
        let mut snapshot = self
            .narrative_repository
            .load_snapshot()
            .map_err(|err| err.to_string())?;
        let screenplay = self
            .screenplay_service
            .get_active_screenplay()
            .map_err(|err| err.to_string())?;

        snapshot.metrics.scene_count = screenplay_scene_count(&screenplay.fountain_text);
        snapshot.metrics.character_count = snapshot.characters.len();
        snapshot.metrics.event_count = snapshot.events.len();

        Ok(snapshot)
    }

    pub fn store_relationship(&self, relationship: OntologyRelationship) -> Result<(), String> {
        self.narrative_repository
            .save_relationship(relationship)
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    pub fn current_project_path(&self) -> Result<Option<String>, String> {
        self.sqlite_repository
            .as_ref()
            .map(|repository| repository.current_path().map(|path| path.to_string_lossy().to_string()))
            .transpose()
            .map_err(|err| err.to_string())
    }

    pub fn switch_project_path(&self, path: &Path) -> Result<(), String> {
        self.sqlite_repository
            .as_ref()
            .ok_or_else(|| "sqlite project storage is not available".to_string())?
            .switch_path(path)
            .map_err(|err| err.to_string())
    }

    pub fn clone_project_to(&self, path: &Path) -> Result<(), String> {
        let repository = self
            .sqlite_repository
            .as_ref()
            .ok_or_else(|| "sqlite project storage is not available".to_string())?;
        let source = repository.current_path().map_err(|err| err.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        std::fs::copy(&source, path).map_err(|err| err.to_string())?;
        repository.switch_path(path).map_err(|err| err.to_string())
    }
}

fn screenplay_scene_count(screenplay_text: &str) -> usize {
    screenplay_text
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            line.starts_with("INT.")
                || line.starts_with("EXT.")
                || line.starts_with("INT/EXT.")
                || line.starts_with("I/E.")
        })
        .count()
}
