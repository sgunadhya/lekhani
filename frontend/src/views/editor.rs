use crate::state::document::DocumentContext;
use leptos::*;

#[component]
pub fn ScreenplayEditor() -> impl IntoView {
    let document = use_context::<DocumentContext>().expect("document context should exist");

    view! {
        <section class="editor-mode">
            <div class="mode-header">
                <span class="eyebrow">"Edit Mode"</span>
                <h2>"Screenplay Editor"</h2>
                <p>"Edit the active screenplay directly in Fountain format. Keep formatting simple and let the narrative assistant guide missing setup."</p>
            </div>
            <textarea
                class="screenplay-textarea"
                prop:value=move || {
                    document
                        .document
                        .get()
                        .map(|doc| doc.fountain_text)
                        .unwrap_or_default()
                }
                on:input=move |ev| {
                    let next_text = event_target_value(&ev);
                    document.document.update(|current| {
                        if let Some(current) = current {
                            current.fountain_text = next_text.clone();
                        }
                    });
                }
                placeholder="Type your Fountain screenplay here..."
                rows=30
                cols=80
            />
        </section>
    }
}
