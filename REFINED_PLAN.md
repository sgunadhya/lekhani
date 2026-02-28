# Refined Plan: Document, Ontology, MCP, and Lint

## Goal

Build Lekhani as a screenplay document app where:

- the screenplay is the writer-facing artifact
- the ontology is the machine-facing story model
- the Narrative assistant uses MCP tools to populate and reconcile structured state
- lint emerges from the document/ontology/link system

## Architecture Summary

### 1. Document Layer

The document layer owns:

- Fountain text
- screenplay title and metadata
- scene segmentation
- document save/open/import/export

This is what the writer feels they are authoring.

### 2. Ontology Layer

The ontology layer owns:

- characters
- events
- relationships
- motivations
- factions
- temporal ordering
- causal links

This is what the system reasons over.

### 3. Link Layer

The link layer connects document and ontology.

Examples:

- scene -> event
- scene -> character
- dialogue/action span -> ontology entity
- ontology entity -> supporting document evidence

Each link should carry:

- provenance
- confidence
- sync state

Suggested sync states:

- `linked`
- `suggested`
- `conflicted`
- `orphaned`

## Authoring Model

### Narrative Mode

Narrative is the primary assistant surface.

User sends natural-language turns.
The agent:

1. interprets intent
2. reads current state
3. calls MCP tools
4. mutates ontology and, when appropriate, document metadata or screenplay content
5. replies with:
   - what changed
   - what remains unclear
   - best next step

Narrative is ontology-first.

### Edit Mode

Edit is the screenplay writing surface.

User writes Fountain directly.
The system:

1. parses the document
2. extracts candidate structure
3. proposes or applies ontology reconciliation

Edit is document-first.

### Visual Mode

Visual is the inspector.

It should surface:

- timeline
- characters
- relationships
- alignment state
- lint findings

Visual is inspect-first, not a manual ontology editor.

## MCP Direction

The assistant should use MCP tooling rather than free-form state mutation.

Current implementation note:

- the repo now has an internal MCP-compatible tool adapter for Narrative commits
- it is not yet a standalone protocol server
- the next step is to expand that adapter into a fuller `ontology.*`, `document.*`, `sync.*`, and `lint.*` tool surface

### Tool Families

#### `ontology.*`

- `list_characters`
- `get_character`
- `create_character`
- `update_character`
- `list_events`
- `get_event`
- `create_event`
- `update_event`
- `create_relationship`
- `update_relationship`
- `get_timeline`
- `get_gaps`

#### `document.*`

- `get_active_document`
- `save_document`
- `update_title`
- `apply_screenplay_edit`
- `import_fountain`
- `export_fountain`

#### `sync.*`

- `propose_links`
- `list_conflicts`
- `merge_characters`
- `merge_events`
- `resolve_alignment_issue`

#### `lint.*`

- `run_lint`
- `list_findings`
- `resolve_finding`
- `dismiss_finding`

## User-Facing Failure Modes

These should shape the implementation.

### 1. Silent Divergence

The document and ontology stop matching, but the user is not told.

Mitigation:

- explicit link layer
- sync states
- alignment lint
- assistant summaries after committed changes

### 2. Duplicate Entities

The system creates new characters/events when it should update existing ones.

Mitigation:

- matching traits
- duplicate detection
- merge flows
- confidence/provenance

### 3. Hidden or Over-Eager Mutation

The assistant changes important state without enough visibility.

Mitigation:

- clear “what changed” responses
- traceable tool calls
- deliberate screenplay edits

### 4. Weak Trust in the Assistant

The user cannot tell what the system understood.

Mitigation:

- conversational reply
- changed-entity summary
- next-step guidance
- confidence and uncertainty surfaced later

### 5. Ontology Overload

The product starts exposing raw structure instead of helping the writer.

Mitigation:

- ontology remains mostly behind Narrative and Visual
- Edit stays document-focused
- avoid raw ontology editing as the default path

### 6. Slow Narrative Interaction

The assistant feels blocked by parsing or model calls.

Mitigation:

- keep tool operations cheap
- separate preview from commit where needed
- fall back gracefully when model calls fail

### 7. Provider Fragility

Foundation Models or another LLM path refuses or fails.

Mitigation:

- provider abstraction
- deterministic fallback logic
- do not let provider failure corrupt project state

### 8. Lint Without Resolution

The system points out problems but does not help fix them.

Mitigation:

- lint findings should carry suggested actions
- assistant should help resolve findings

## Lint Direction

Lint should become a first-class feature after links and sync states are stable.

### Lint Categories

#### Document Lint

- malformed Fountain structure
- duplicate or weak scene headings
- dialogue formatting anomalies

#### Narrative Lint

- protagonist has no goal
- event has no participants
- character is isolated
- event order is inconsistent

#### Alignment Lint

- ontology character not present in screenplay
- screenplay introduces likely entity not linked to ontology
- event exists in ontology but has no document evidence
- scene implies event not modeled

### Lint Finding Shape

Each finding should have:

- `id`
- `kind`
- `severity`
- `scope`
- `message`
- `evidence`
- `related_entity_ids`
- `related_scene_ids`
- `suggested_actions`
- `status`

## Traits to Introduce

Use traits early and refine later.

### Core Traits

- `DocumentRepository`
- `OntologyRepository`
- `LinkRepository`
- `LintRepository`

### Processing Traits

- `NarrativeAgent`
- `DocumentExtractor`
- `SyncResolver`
- `LintEngine`
- `EntityMatcher`
- `RelationshipInferer`
- `TimelineReasoner`

### Strategy

- start deterministic
- keep advanced reasoning behind traits
- do not make embeddings or CoT the source of truth

## Later Reasoning Extensions

These are promising, but not the foundation:

- embeddings for duplicate detection and retrieval
- temporal/causal ranking for event links
- agent planning traces
- richer timeline reasoning

They should support the explicit model, not replace it.

## Suggested Storage Direction

Keep `.lekhani` as the canonical SQLite project file.

Likely tables:

- `screenplays`
- `document_scenes`
- `ontology_entities`
- `ontology_relationships`
- `document_ontology_links`
- `sync_candidates`
- `lint_findings`
- `schema_migrations`

## Implementation Order

### Phase 1

- stabilize current Narrative and Edit surfaces
- keep `.lekhani` as canonical project format
- preserve document and ontology storage in the same project

### Phase 2

- introduce explicit document/ontology link tables
- track provenance, confidence, and sync state

### Phase 3

- move Narrative commit flow behind MCP-style tool boundaries
- keep assistant tool-driven

### Phase 4

- build extraction from Edit into candidate ontology updates
- support reconciliation and merge flows

### Phase 5

- add deterministic lint engine
- surface findings in Visual
- route resolution through Narrative assistant

## Immediate Next Steps

1. Define the concrete link schema in SQLite.
2. Define the first MCP tool surface for ontology and document operations.
3. Add sync-state objects and alignment status to the backend.
4. Add the first deterministic lint finding model, even before full lint rules exist.
