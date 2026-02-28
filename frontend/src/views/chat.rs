use crate::state::document::DocumentContext;
use crate::state::narrative::{
    create_character_action, create_event_action, create_nudge_resource,
};
use leptos::*;

#[component]
pub fn ChatInterface() -> impl IntoView {
    let document = use_context::<DocumentContext>().expect("document context should exist");
    let (prompt, set_prompt) = create_signal(String::new());
    let (nudge_nonce, set_nudge_nonce) = create_signal(0_u64);
    let nudge = create_nudge_resource(nudge_nonce);
    let character_action = create_character_action();
    let event_action = create_event_action();

    let submit_character = move |_| {
        let current_prompt = prompt.get_untracked();
        if !current_prompt.trim().is_empty() {
            character_action.dispatch(current_prompt);
            set_nudge_nonce.update(|value| *value += 1);
        }
    };

    let submit_event = move |_| {
        let current_prompt = prompt.get_untracked();
        if !current_prompt.trim().is_empty() {
            event_action.dispatch(current_prompt);
            set_nudge_nonce.update(|value| *value += 1);
        }
    };

    let apply_prompt_as_title = move |_| {
        let current_prompt = prompt.get_untracked();
        if !current_prompt.trim().is_empty() {
            document.document.update(|current| {
                if let Some(current) = current {
                    current.title = current_prompt.clone();
                }
            });
        }
    };

    let append_prompt_to_draft = move |_| {
        let current_prompt = prompt.get_untracked();
        if !current_prompt.trim().is_empty() {
            document.document.update(|current| {
                if let Some(current) = current {
                    if !current.fountain_text.trim().is_empty() {
                        current.fountain_text.push_str("\n\n");
                    }
                    current
                        .fountain_text
                        .push_str(&format!("[[Narrative note: {}]]", current_prompt.trim()));
                }
            });
        }
    };

    view! {
        <section class="chat-mode">
            <div class="mode-header">
                <span class="eyebrow">"Narrative Mode"</span>
                <h2>"Narrative Setup Assistant"</h2>
                <p>"Describe the story in plain language. The assistant parses character and event input, nudges the setup forward, and can later revise the screenplay itself."</p>
            </div>

            <div class="chat-layout">
                <div class="assistant-panel">
                    <label class="chat-label" for="narrative-input">"What should the system understand next?"</label>
                    <textarea
                        id="narrative-input"
                        class="assistant-input"
                        prop:value=prompt
                        on:input=move |ev| set_prompt.set(event_target_value(&ev))
                        placeholder="Example: The lead is a conflicted prince trying to avoid a war he is expected to start."
                        rows=8
                    />
                    <div class="assistant-actions">
                        <button class="primary-button" on:click=submit_character>"Parse Character"</button>
                        <button class="secondary-button" on:click=submit_event>"Parse Event"</button>
                        <button class="secondary-button" on:click=append_prompt_to_draft>"Append To Draft"</button>
                        <button class="secondary-button" on:click=apply_prompt_as_title>"Use As Title"</button>
                    </div>
                </div>

                <div class="assistant-results">
                    <div class="result-card">
                        <h3>"Current Nudge"</h3>
                        {move || match nudge.get() {
                            None => view! { <p>"Generating nudge..."</p> }.into_view(),
                            Some(Ok(nudge)) => view! { <p>{nudge.message}</p> }.into_view(),
                            Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                        }}
                    </div>

                    <div class="result-card">
                        <h3>"Latest Character Parse"</h3>
                        {move || match character_action.value().get() {
                            None => view! { <p>"No character parsed yet."</p> }.into_view(),
                            Some(Ok(character)) => view! {
                                <div class="parsed-block">
                                    <strong>{character.name}</strong>
                                    <p>{character.summary}</p>
                                    <p class="muted">{character.tags.join(", ")}</p>
                                </div>
                            }.into_view(),
                            Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                        }}
                    </div>

                    <div class="result-card">
                        <h3>"Latest Event Parse"</h3>
                        {move || match event_action.value().get() {
                            None => view! { <p>"No event parsed yet."</p> }.into_view(),
                            Some(Ok(event)) => view! {
                                <div class="parsed-block">
                                    <strong>{event.title}</strong>
                                    <p>{event.summary}</p>
                                    <p class="muted">
                                        {format!("Participants tracked: {}", event.participants.len())}
                                    </p>
                                </div>
                            }.into_view(),
                            Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                        }}
                    </div>
                </div>
            </div>
        </section>
    }
}
