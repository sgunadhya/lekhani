use crate::adapters::tauri::state::AppState;
use crate::application::{
    DefaultNarrativeRuntime, NarrativeRuntime, NarrativeRuntimeDeps, NarrativeTurnOutcome,
};
use crate::domain::{NarrativeMessagePreview, NarrativeSuggestionAction};

fn runtime_deps<'a>(state: &'a AppState) -> NarrativeRuntimeDeps<'a> {
    NarrativeRuntimeDeps {
        query_gateway: state,
        mutation_gateway: state,
        generation_gateway: state,
        conversation_support: state,
        narrative_engine: state.narrative_engine.as_ref(),
    }
}

pub fn preview_turn(state: &AppState, prompt: &str) -> Result<NarrativeMessagePreview, String> {
    DefaultNarrativeRuntime.preview_turn(&runtime_deps(state), prompt)
}

pub fn submit_turn(state: &AppState, prompt: &str) -> Result<NarrativeTurnOutcome, String> {
    DefaultNarrativeRuntime.submit_turn(&runtime_deps(state), prompt)
}

pub fn apply_suggestion_action(
    state: &AppState,
    action: NarrativeSuggestionAction,
) -> Result<NarrativeTurnOutcome, String> {
    DefaultNarrativeRuntime.apply_suggestion_action(&runtime_deps(state), action)
}
