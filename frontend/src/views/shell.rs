use crate::api::tauri;
use crate::state::document::DocumentContext;
use crate::state::app_mode::AppMode;
use crate::state::narrative::{create_nudge_resource, NarrativeChatContext};
use crate::views::chat::ChatInterface;
use crate::views::editor::ScreenplayEditor;
use crate::views::timeline::TimelineView;
use leptos::*;

#[component]
pub fn WorkspaceShell() -> impl IntoView {
    let (mode, set_mode) = create_signal(AppMode::Narrative);
    let document = create_rw_signal(None);
    let file_path = create_rw_signal(None);
    let (nudge_nonce, set_nudge_nonce) = create_signal(0_u64);
    let nudge = create_nudge_resource(nudge_nonce);
    let active_project = create_local_resource(|| (), |_| async move { tauri::get_current_project().await });
    let narrative_chat = NarrativeChatContext::new();

    let document_context = DocumentContext {
        document,
        file_path,
    };

    provide_context(document_context);
    provide_context(narrative_chat);

    create_effect(move |_| {
        if let Some(Ok(project)) = active_project.get() {
            document.set(Some(project.screenplay));
            file_path.set(project.file_path);
        }
    });

    spawn_local({
        let active_project = active_project;
        async move {
            let _ = tauri::listen_for_project_opened(move || {
                _ = active_project.refetch();
            })
            .await;
        }
    });

    let refresh_nudge = move |_| set_nudge_nonce.update(|value| *value += 1);

    let open_document = move || {
        let document = document;
        let file_path = document_context.file_path;
        spawn_local(async move {
            if let Ok(Some(document_file)) = tauri::open_project_document().await {
                document.set(Some(document_file.screenplay));
                file_path.set(document_file.file_path);
            }
        });
    };

    let import_fountain = move || {
        let document = document;
        spawn_local(async move {
            if let Ok(Some(imported_screenplay)) = tauri::import_fountain_document().await {
                document.set(Some(imported_screenplay));
            }
        });
    };

    let save_project = move |force_picker: bool| {
        let document = document;
        let file_path = document_context.file_path;
        spawn_local(async move {
            let Some(current_document) = document.get_untracked() else {
                return;
            };

            let persisted = match tauri::save_screenplay(current_document.clone()).await {
                Ok(saved) => saved,
                Err(_) => return,
            };
            document.set(Some(persisted.clone()));

            if force_picker || file_path.get_untracked().is_none() {
                if let Ok(Some(saved_file)) =
                    tauri::save_project_document_as(persisted, None).await
                {
                    document.set(Some(saved_file.screenplay));
                    file_path.set(saved_file.file_path);
                }
            }
        });
    };

    let export_fountain = move || {
        let document = document;
        spawn_local(async move {
            let Some(current_document) = document.get_untracked() else {
                return;
            };

            let _ = tauri::export_fountain_document(current_document).await;
        });
    };

    spawn_local({
        let open_document = open_document;
        async move {
            let _ = tauri::listen_for_menu_action("menu-open-project", move || {
                open_document();
            })
            .await;
        }
    });

    spawn_local({
        let save_project = save_project;
        async move {
            let _ = tauri::listen_for_menu_action("menu-save-project", move || {
                save_project(false);
            })
            .await;
        }
    });

    spawn_local({
        let save_project = save_project;
        async move {
            let _ = tauri::listen_for_menu_action("menu-save-project-as", move || {
                save_project(true);
            })
            .await;
        }
    });

    spawn_local({
        let import_fountain = import_fountain;
        async move {
            let _ = tauri::listen_for_menu_action("menu-import-fountain", move || {
                import_fountain();
            })
            .await;
        }
    });

    spawn_local({
        let export_fountain = export_fountain;
        async move {
            let _ = tauri::listen_for_menu_action("menu-export-fountain", move || {
                export_fountain();
            })
            .await;
        }
    });

    spawn_local({
        let active_project = active_project;
        async move {
            let _ = tauri::listen_for_menu_action("menu-reload-project", move || {
                spawn_local(async move {
                    _ = active_project.refetch();
                });
            })
            .await;
        }
    });

    view! {
        <div class="workspace-shell">
            <header class="workspace-header">
                <div class="workspace-title-group">
                    <label class="workspace-label" for="document-title">"Document"</label>
                    <input
                        id="document-title"
                        class="document-title-input"
                        prop:value=move || {
                            document
                                .get()
                                .map(|doc| doc.title)
                                .unwrap_or_else(|| "Loading screenplay...".to_string())
                        }
                        on:input=move |ev| {
                            let next_title = event_target_value(&ev);
                            document.update(|current| {
                                if let Some(current) = current {
                                    current.title = next_title.clone();
                                }
                            });
                        }
                    />
                </div>

                <div class="mode-tabs">
                    <button
                        class:mode-tab=true
                        class:mode-tab-active=move || mode.get() == AppMode::Narrative
                        on:click=move |_| set_mode.set(AppMode::Narrative)
                    >
                        "Narrative"
                    </button>
                    <button
                        class:mode-tab=true
                        class:mode-tab-active=move || mode.get() == AppMode::Edit
                        on:click=move |_| set_mode.set(AppMode::Edit)
                    >
                        "Edit"
                    </button>
                    <button
                        class:mode-tab=true
                        class:mode-tab-active=move || mode.get() == AppMode::Visual
                        on:click=move |_| set_mode.set(AppMode::Visual)
                    >
                        "Visual"
                    </button>
                </div>

            </header>

            <div
                class="workspace-body"
                class:workspace-body-single=move || mode.get() == AppMode::Narrative
                class:workspace-body-wide=move || mode.get() != AppMode::Visual
            >
                <main class="workspace-main">
                    {move || match mode.get() {
                        AppMode::Narrative => view! { <ChatInterface/> }.into_view(),
                        AppMode::Edit => view! { <ScreenplayEditor/> }.into_view(),
                        AppMode::Visual => view! { <TimelineView/> }.into_view(),
                    }}
                </main>

                <Show when=move || mode.get() == AppMode::Visual>
                    <aside class="workspace-rail">
                        <section class="rail-section">
                            <span class="eyebrow">"Project"</span>
                            <p>{move || document_context.file_path.get().unwrap_or_else(|| "Unsaved .lekhani project".to_string())}</p>
                        </section>

                        <section class="rail-section">
                            <span class="eyebrow">"Current Nudge"</span>
                            {move || match nudge.get() {
                                None => view! { <p>"Loading nudge..."</p> }.into_view(),
                                Some(Ok(nudge)) => view! { <p>{nudge.message}</p> }.into_view(),
                                Some(Err(err)) => view! { <p class="error">{err}</p> }.into_view(),
                            }}
                        </section>

                        <section class="rail-section">
                            <span class="eyebrow">"Setup Focus"</span>
                            <ul class="focus-list">
                                <li>"Define the lead and their core conflict"</li>
                                <li>"Clarify the opening event"</li>
                                <li>"Link narrative setup back to Fountain scenes"</li>
                            </ul>
                            <div class="rail-actions">
                                <button class="secondary-button" on:click=refresh_nudge>"Refresh Nudge"</button>
                            </div>
                        </section>
                    </aside>
                </Show>
            </div>
        </div>
    }
}
