use crate::api::dto::ScreenplayDto;
use leptos::*;

#[derive(Clone, Copy)]
pub struct DocumentContext {
    pub document: RwSignal<Option<ScreenplayDto>>,
    pub file_path: RwSignal<Option<String>>,
}
