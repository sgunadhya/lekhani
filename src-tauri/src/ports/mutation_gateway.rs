use crate::domain::{ConversationTopic, OntologyEntity, WorkingMemory};

pub trait MutationGateway: Send + Sync {
    fn save_working_memory(&self, memory: WorkingMemory) -> Result<(), String>;
    fn propose_ontology_entity(
        &self,
        title: String,
        summary: String,
        entity: OntologyEntity,
    ) -> Result<(), String>;
    fn confirm_current_focus(
        &self,
        topic: ConversationTopic,
        focus_summary: &str,
    ) -> Result<bool, String>;
}
