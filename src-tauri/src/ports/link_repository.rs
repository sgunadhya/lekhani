use crate::domain::{AppError, DocumentOntologyLink};

pub trait LinkRepository: Send + Sync {
    fn upsert_link(&self, link: DocumentOntologyLink) -> Result<DocumentOntologyLink, AppError>;
    fn find_for_document_ref(&self, document_ref: &str)
        -> Result<Vec<DocumentOntologyLink>, AppError>;
    fn find_for_ontology_ref(
        &self,
        ontology_ref: &str,
    ) -> Result<Vec<DocumentOntologyLink>, AppError>;
}
