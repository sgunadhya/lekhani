# Lekhani Agent Direction

## Product Direction

Lekhani is a macOS screenplay writing app with three distinct surfaces:

1. `Narrative`
   - primary assistant surface
   - user talks to the system in natural language
   - assistant uses MCP tools to read and mutate structured story state

2. `Edit`
   - screenplay writing surface
   - Fountain document editor plus rendered preview
   - document changes can produce ontology sync candidates

3. `Visual`
   - read-oriented inspector
   - timeline, characters, relationships, lint, and document/ontology alignment views

The product is not a generic chat app and not a raw ontology editor.
It is a screenplay document tool with a narrative assistant backed by structured story state.

## Core Model

The system has three related layers:

1. `Document`
   - Fountain screenplay text
   - title and document metadata
   - scenes and text positions

2. `Ontology`
   - characters
   - events
   - relationships
   - motivations
   - temporal and causal structure

3. `Links`
   - explicit mappings between document elements and ontology entities
   - provenance, confidence, and sync state

The link layer is mandatory. Without it, document and ontology drift apart.

## Narrative Agent

The Narrative assistant is tool-driven.

The assistant should not mutate project state by emitting free-form JSON alone.
Committed state changes should come from MCP tool calls against the project model.

The intended pattern is:

1. user sends a message in Narrative
2. agent interprets intent
3. agent calls MCP tools
4. ontology/document state changes are persisted
5. assistant replies conversationally with:
   - what changed
   - what is still unclear
   - best next step

The runtime control model should follow a fixed dialogue-state pattern:

1. classify the turn
2. update working memory / belief state
3. derive capability plan and write policy
4. expose only the MCP tools allowed for that turn
5. execute tools
6. generate the reply from plan + observations + state

Avoid adding behavior by scattering phrase-specific heuristics through orchestration code.
If a new behavior is needed, it should attach to one of:

- dialogue act classification
- belief-state update
- capability planning
- tool policy
- response realization

## MCP Tooling Direction

The ontology should be surfaced to the assistant as MCP tools.

Current implementation note:
- there is now an internal MCP-compatible tool adapter in the backend
- Narrative commits should move through that tool boundary instead of mutating repositories directly
- it is not yet a standalone MCP protocol server

Tool families:

1. `ontology.*`
   - list/get/create/update characters
   - list/get/create/update events
   - create/update relationships
   - timeline and gap queries

2. `document.*`
   - get active screenplay
   - save document
   - update title/metadata
   - import/export Fountain
   - apply screenplay edits deliberately

3. `sync.*`
   - create candidate links
   - reconcile document and ontology
   - detect duplicates
   - merge entities

4. `lint.*`
   - run lint checks
   - list active findings
   - resolve or dismiss findings

## User-Facing Failure Modes

From the user standpoint, the main risks are:

1. silent divergence
   - chat changes ontology but the screenplay never reflects them
   - screenplay changes but ontology remains stale

2. duplicate structure
   - assistant creates a second version of an existing character or event

3. hidden edits
   - assistant mutates important state without making the change legible

4. weak trust
   - user cannot tell whether a change was applied, proposed, inferred, or guessed

5. ontology overload
   - product starts surfacing raw structure instead of helping the writer

6. slow interaction
   - chat feels blocked by heavy parsing or model latency

7. brittle provider behavior
   - model safety refusals or provider unavailability break the authoring loop

8. lint without relief
   - system points out problems but does not help resolve them

9. over-eager automation
   - document text is rewritten too aggressively from Narrative input

10. unclear ownership
   - user cannot tell whether Narrative or Edit is the primary source for a given fact

## Product Rules

1. Narrative is ontology-first.
   - it commits intended story structure

2. Edit is document-first.
   - it commits written screenplay expression

3. Visual is inspect-first.
   - it shows derived structure, alignment, and lint

4. Assistant replies must stay legible.
   - state changes should be summarized in user language

5. Document edits should be deliberate.
   - prefer explicit proposals or traceable applied edits

6. Lint should lead to action.
   - every useful lint finding should suggest a resolution path

## Architecture Direction

Use light hexagonal boundaries with traits.

Good trait candidates:

- `DocumentRepository`
- `OntologyRepository`
- `LinkRepository`
- `LintRepository`
- `NarrativeAgent`
- `DocumentExtractor`
- `SyncResolver`
- `LintEngine`
- `EntityMatcher`
- `TimelineReasoner`

Start deterministic where possible.
Add embedding or advanced reasoning later behind traits.

## Terminology

Avoid the word `GOLEM` in product-facing UX and primary code terminology.

Preferred language:

- `ontology`
- `narrative model`
- `story structure`

## Agent Editing Boundaries

Focused work should respect ownership:

- frontend-focused work:
  - may edit `frontend/**`
  - may not edit `src-tauri/**`, `Makefile`, workspace manifests, integration config unless explicitly asked

- backend-focused work:
  - may edit `src-tauri/**`
  - may not edit `frontend/**` unless the task explicitly requires contract updates

- integration-focused work:
  - may edit `src-tauri/tauri.conf.json`, root manifests, `Makefile`, app wiring
  - should avoid product logic changes unless explicitly requested

Default rule:
- if the user asks for focused frontend or backend work, do not “helpfully” rewrite integration at the same time

## Source of Truth Notes

- active frontend source of truth: `frontend/src/**`
- active backend source of truth: `src-tauri/src/**`
- old backup architecture exists only as reference, not as canonical implementation
