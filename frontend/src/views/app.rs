use crate::views::shell::WorkspaceShell;
use leptos::*;
use leptos_meta::*;

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Meta name="viewport" content="width=device-width, initial-scale=1.0"/>
        <Title text="Lekhani - Screenplay Writing Tool"/>

        <WorkspaceShell/>
    }
}
