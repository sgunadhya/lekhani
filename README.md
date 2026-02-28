# Lekhani

Lekhani is a Tauri + Leptos desktop app for screenplay writing and narrative setup.

It treats the screenplay as a document, stores projects in `.lekhani` files backed by SQLite, supports Fountain import/export, and builds a structured narrative/ontology model from natural-language input.

## Current Shape

- `Narrative` mode:
  - one writing surface for natural-language setup
  - live inferred preview in a side pane
  - single commit action
- `Edit` mode:
  - plain Fountain editor
- `Visual` mode:
  - one scrollable inspector for derived narrative state

The backend currently hydrates:
- characters
- events
- character-to-character relationships
- event participation links
- ontology projection links

## Project Format

- Primary format: `.lekhani`
  - SQLite-backed project file
- Interchange format: `.fountain`
  - import/export only

## Stack

- Frontend: Leptos
- Desktop shell: Tauri 2
- Persistence: SQLite via `rusqlite`
- Local macOS LLM path: `fm-rs`
- Fallback hydration path: local heuristic engine

## Repository Layout

- [`frontend`](/Users/sushantsrivastava/code/Screenplays/MathuraStruggle/frontend): Leptos UI
- [`src-tauri`](/Users/sushantsrivastava/code/Screenplays/MathuraStruggle/src-tauri): Tauri app and backend
- [`src-tauri/migrations`](/Users/sushantsrivastava/code/Screenplays/MathuraStruggle/src-tauri/migrations): SQLite migrations
- [`AGENTS.md`](/Users/sushantsrivastava/code/Screenplays/MathuraStruggle/AGENTS.md): project-specific agent guardrails

Backend modules are split into:
- `domain`
- `application`
- `ports`
- `adapters`

## Getting Started

Requirements:

- Rust toolchain
- `cargo tauri`
- `trunk`
- macOS if you want to use the `fm-rs` Foundation Models path

Common commands:

```bash
make dev
make build
make launch
make quick-test
```

## Development Notes

- `Narrative` mode is the primary authoring surface.
- `Visual` mode is a read-oriented inspector over derived model state.
- The UI intentionally avoids heavy manual ontology editors; the assistant is the main mutation path.
- If Foundation Models is unavailable or rejects a request, the app falls back to the local heuristic hydrator.

## Status

Implemented:

- working Leptos + Tauri desktop integration
- `.lekhani` document workflow
- Fountain import/export
- SQLite persistence with migrations
- narrative message preview/commit flow
- relationship hydration
- simplified document-centered UI

Still rough:

- multi-entity intent handling beyond one primary inferred target
- richer relationship types
- screenplay patch proposals instead of appending narrative notes
- provider configuration for non-Apple LLM backends
