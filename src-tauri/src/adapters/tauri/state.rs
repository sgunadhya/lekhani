use crate::adapters::db::SqliteScreenplayRepository;
use crate::application::{
    AssistantCapabilityPlanner, AssistantFallbackResponder, AssistantIntentContext,
    BeliefStateUpdater, CapabilityPlan, CapabilityPlanningContext, DialogueAct,
    DialogueActContext, DialogueStateContext, DialogueStateUpdate,
    MutationGate, NarrativeConversationSupport, NarrativeService, ResponseStateFinalizer,
    ScreenplayService,
};
use crate::domain::{
    AppError, ConversationTopic, NarrativeCharacter, NarrativeEvent, NarrativeMessagePreview,
    NarrativeSnapshot, OntologyEntity, OntologyEntityKind, OntologyRelationship, StorySnapshot,
    WorkingMemory,
};
use crate::ports::{
    CharacterParser, EventParser, MutationGateway, NarrativeGenerationGateway,
    NarrativeProvider, NarrativeRepository, NudgeGenerator, QueryGateway, ScreenplayRepository,
    WorkingMemoryRepository,
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
    pub narrative_provider: Box<dyn NarrativeProvider>,
    pub belief_state_updater: Box<dyn BeliefStateUpdater>,
    pub assistant_capability_planner: Box<dyn AssistantCapabilityPlanner>,
    pub assistant_fallback_responder: Box<dyn AssistantFallbackResponder>,
    pub response_state_finalizer: Box<dyn ResponseStateFinalizer>,
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
        narrative_provider: Box<dyn NarrativeProvider>,
        belief_state_updater: Box<dyn BeliefStateUpdater>,
        assistant_capability_planner: Box<dyn AssistantCapabilityPlanner>,
        assistant_fallback_responder: Box<dyn AssistantFallbackResponder>,
        response_state_finalizer: Box<dyn ResponseStateFinalizer>,
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
            narrative_provider,
            belief_state_updater,
            assistant_capability_planner,
            assistant_fallback_responder,
            response_state_finalizer,
            mutation_gate,
            narrative_repository,
            sqlite_repository,
            llm_backend,
            llm_detail,
        }
    }
}

impl QueryGateway for AppState {
    fn load_working_memory(&self) -> Result<WorkingMemory, String> {
        self.get_working_memory()
    }

    fn load_narrative_snapshot(&self) -> Result<NarrativeSnapshot, String> {
        self.get_snapshot()
    }

    fn load_story_snapshot(&self) -> Result<StorySnapshot, String> {
        let screenplay = self
            .screenplay_service
            .get_active_screenplay()
            .map_err(|err| err.to_string())?;

        Ok(StorySnapshot {
            screenplay_title: screenplay.title.clone(),
            fountain_text: screenplay.fountain_text.clone(),
            narrative: self.get_snapshot()?,
            working_memory: self.get_working_memory()?,
        })
    }
}

impl MutationGateway for AppState {
    fn save_working_memory(&self, memory: WorkingMemory) -> Result<(), String> {
        let Some(repository) = self.sqlite_repository.as_ref() else {
            return Ok(());
        };

        repository
            .save_working_memory(memory)
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    fn propose_ontology_entity(
        &self,
        title: String,
        summary: String,
        entity: OntologyEntity,
    ) -> Result<(), String> {
        let _ = crate::adapters::mcp::execute_tool(
            self,
            crate::adapters::mcp::McpToolCall::ProposeOntologyCommit {
                sync_run_id: None,
                title,
                summary,
                entity: Some(entity),
                relationship: None,
            },
        )?;

        Ok(())
    }

    fn confirm_current_focus(
        &self,
        topic: ConversationTopic,
        focus_summary: &str,
    ) -> Result<bool, String> {
        if !matches!(topic, ConversationTopic::Setting) {
            return Ok(false);
        }

        self.propose_ontology_entity(
            "Setting proposal".to_string(),
            focus_summary.to_string(),
            OntologyEntity {
                id: uuid::Uuid::new_v4(),
                kind: OntologyEntityKind::Setting,
                label: focus_summary.to_string(),
                summary: focus_summary.to_string(),
            },
        )?;

        Ok(true)
    }

    fn propose_preview(&self, preview: &NarrativeMessagePreview) -> Result<bool, String> {
        if let Some(character) = preview.character.as_ref() {
            self.propose_ontology_entity(
                format!("Character proposal: {}", character.name),
                character.summary.clone(),
                OntologyEntity {
                    id: uuid::Uuid::new_v4(),
                    kind: OntologyEntityKind::Character,
                    label: character.name.clone(),
                    summary: character.summary.clone(),
                },
            )?;
            return Ok(true);
        }

        if let Some(event) = preview.event.as_ref() {
            self.propose_ontology_entity(
                format!("Event proposal: {}", event.title),
                event.summary.clone(),
                OntologyEntity {
                    id: uuid::Uuid::new_v4(),
                    kind: OntologyEntityKind::Event,
                    label: event.title.clone(),
                    summary: event.summary.clone(),
                },
            )?;
            return Ok(true);
        }

        Ok(false)
    }
}

impl NarrativeConversationSupport for AppState {
    fn classify_dialogue_act(&self, context: DialogueActContext<'_>) -> DialogueAct {
        self.narrative_provider.classify_dialogue_act(context)
    }

    fn update_belief_state(&self, context: DialogueStateContext<'_>) -> DialogueStateUpdate {
        self.belief_state_updater.update(context)
    }

    fn plan_capabilities(&self, context: CapabilityPlanningContext<'_>) -> CapabilityPlan {
        self.assistant_capability_planner.plan(context)
    }

    fn allow_mutation(&self, context: AssistantIntentContext<'_>) -> bool {
        self.mutation_gate.allow_mutation(context)
    }

    fn fallback_response(
        &self,
        prompt: &str,
        preview: &crate::domain::NarrativeMessagePreview,
        memory: &WorkingMemory,
        plan: &CapabilityPlan,
    ) -> crate::ports::AssistantResponse {
        self.assistant_fallback_responder
            .respond(prompt, preview, memory, plan)
    }

    fn finalize_response_state(
        &self,
        memory: WorkingMemory,
        prompt: &str,
        plan: &CapabilityPlan,
        title: &str,
        body: &str,
        focus_summary: Option<&str>,
    ) -> WorkingMemory {
        self.response_state_finalizer
            .finalize(memory, prompt, plan, title, body, focus_summary)
    }
}

impl NarrativeGenerationGateway for AppState {
    fn preview_message(
        &self,
        prompt: &str,
        snapshot: &NarrativeSnapshot,
    ) -> Result<crate::domain::NarrativeMessagePreview, String> {
        self.narrative_service.preview_message(prompt, snapshot)
    }

    fn interpret_followup(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<crate::ports::FollowUpDirective, String> {
        self.narrative_provider.interpret_followup(prompt, memory)
    }

    fn elaborate_focus(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<crate::ports::AssistantResponse, String> {
        self.narrative_provider.elaborate_focus(prompt, memory)
    }

    fn suggest_alternative(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<crate::ports::AssistantResponse, String> {
        self.narrative_provider.suggest_alternative(prompt, memory)
    }

    fn brainstorm_topic(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<crate::ports::AssistantResponse, String> {
        self.narrative_provider.brainstorm_topic(prompt, memory)
    }

    fn respond_in_context(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<crate::ports::AssistantResponse, String> {
        self.narrative_provider.respond_in_context(prompt, memory)
    }

    fn draft_from_focus(
        &self,
        prompt: &str,
        memory: &WorkingMemory,
    ) -> Result<crate::ports::AssistantResponse, String> {
        self.narrative_provider.draft_from_focus(prompt, memory)
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

    pub fn get_working_memory(&self) -> Result<WorkingMemory, String> {
        let Some(repository) = self.sqlite_repository.as_ref() else {
            return Ok(WorkingMemory::default());
        };

        repository
            .load_working_memory("current-project", "main")
            .map_err(|err: AppError| err.to_string())
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
