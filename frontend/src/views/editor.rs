use crate::state::document::DocumentContext;
use leptos::*;
use nom::error::VerboseError;

#[component]
pub fn ScreenplayEditor() -> impl IntoView {
    let document = use_context::<DocumentContext>().expect("document context should exist");
    let screenplay_text = move || {
        document
            .document
            .get()
            .map(|doc| doc.fountain_text)
            .unwrap_or_default()
    };

    view! {
        <section class="editor-mode">
            <div class="editor-split">
                <section class="editor-pane">
                    <textarea
                        class="screenplay-textarea"
                        prop:value=screenplay_text
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

                <section class="editor-pane screenplay-preview-pane">
                    <div class="screenplay-preview">
                        {move || {
                            let preview = render_fountain_preview(&screenplay_text());

                            match preview {
                                Some(html) => view! {
                                    <div class="screenplay-preview-content" inner_html=html></div>
                                }
                                    .into_view(),
                                None => view! {
                                    <p class="muted">"Preview will appear as you write Fountain."</p>
                                }
                                    .into_view(),
                            }
                        }}
                    </div>
                </section>
            </div>
        </section>
    }
}

fn render_fountain_preview(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = if input.ends_with('\n') {
        input.to_string()
    } else {
        format!("{input}\n")
    };

    match fountain::parse_document::<VerboseError<&str>>(&normalized) {
        Ok((_remaining, parsed)) => Some(parsed.as_html()),
        Err(_err) => Some(format!(
            "<div class=\"fountain-preview-error\">{}</div>",
            html_escape(trimmed)
        )),
    }
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
