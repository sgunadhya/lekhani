use crate::state::screenplays::create_screenplays_resource;
use leptos::*;

#[component]
pub fn HomePage() -> impl IntoView {
    let screenplays = create_screenplays_resource();

    view! {
        <section class="narrative-mode">
            <div class="narrative-overview">
                <span class="eyebrow">"Narrative Setup Assistant"</span>
                <h2>"Build the story model before you polish the script."</h2>
                <p>"Use the assistant to define characters, events, intent, and structure. The screenplay stays in Fountain, but the narrative model drives nudges and derived views."</p>
            </div>
            <div class="screenplay-summary">
                <h3>"Current Screenplays"</h3>
                {move || match screenplays.get() {
                    None => view! { <p>"Loading..."</p> }.into_view(),
                    Some(Ok(sps)) => view! {
                        <ul class="screenplay-list">
                            {sps.iter().map(|sp| view! {
                                <li class="screenplay-list-item" data-id=sp.id.to_string()>
                                    <span class="screenplay-bullet"></span>
                                    <span>{&sp.title}</span>
                                </li>
                            }).collect_view()}
                        </ul>
                    }.into_view(),
                    Some(Err(e)) => view! { <p class="error">{"Error: "}{e}</p> }.into_view(),
                }}
            </div>
        </section>
    }
}
