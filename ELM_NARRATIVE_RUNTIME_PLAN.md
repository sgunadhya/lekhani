# Elm-Style Narrative Runtime Plan

## Goal

Use an Elm-style architecture for the `Narrative` surface so the app owns state transitions and the model remains an effectful helper instead of the control plane.

This is a runtime plan for chat behavior, not a replacement for the existing document / ontology / sync architecture.

## Why

The main failure pattern in Lekhani has been letting freeform chat behave like a mutation protocol.

That caused:

- accidental commits
- stale memory leaking into ordinary chat
- extraction logic contaminating brainstorming
- provider behavior dictating state transitions

The correction is:

- app-owned state transitions
- model-backed generation
- explicit mutation boundary

## Core Rule

Plain chat is non-mutating.

Only explicit suggestion actions or deliberate commit paths should call the mutation gateway.

Examples:

- `Use this`
- `Try another`
- `Expand this`
- `Add to screenplay`

## Elm Mapping

### Model

The runtime should keep a narrow interaction model for the active narrative session:

- `messages`
- `current_topic`
- `current_candidate`
- `current_mode`
- `suggested_actions`
- `working_memory_summary`
- `story_snapshot_summary`

This sits on top of broader canonical state:

- screenplay
- ontology
- links
- working memory

### Agent Stance

`current_mode` should evolve into an explicit `AgentStance`.

This is the thin crossing point between UI intent and model behavior.
It constrains which commands are legal for the current turn.

Recommended first-cut stance set:

- `Idle`
- `Brainstorm`
- `Elaborate`
- `Commit`
- `Retrieve`

Important note:

- defer richer stances like `Critique` until they have a real product use case
- keep the first stance set small so the runtime stays legible

### Narrative Thread

The core unit should move from a flat session stream toward a scoped `NarrativeThread`.

First-cut thread shape:

```text
NarrativeThread {
  id
  goal
  status
  turn_ids
  current_candidate
  open_questions
  suggested_actions
}
```

Where:

- `goal` is the current story objective for the thread
- `status` is one of:
  - `Active`
  - `Parked`
  - `Committed`

This should become the main state container for:

- current thread
- sidequests
- deferred ideas
- thread-local continuity

Important note:

- a thread is not necessarily a fully autonomous agent loop
- it is primarily a scoped state container plus an action space

### View

The Narrative UI is a projection of the model:

- chat transcript
- current thread
- current anchor
- sidequests / deferred ideas
- next move / nudge
- suggestion actions

### Msg

Suggested message types:

- `UserSubmittedTurn`
- `AssistantReturnedReply`
- `SuggestionClicked`
- `WorkingMemoryLoaded`
- `StorySnapshotLoaded`
- `MutationSucceeded`
- `MutationFailed`

Later additions once thread-scoped runtime is in place:

- `ThreadSelected`
- `ThreadParked`
- `ThreadCommitted`
- `ThreadCreatedFromSuggestion`

### Update

The `update` step should be deterministic and app-owned.

It decides:

- whether the turn is ordinary conversation
- whether the runtime should refine the current candidate
- whether a suggestion click should trigger mutation
- how state changes after each effect result

The provider should not directly decide canonical state transitions.

### Cmd

LLM calls and MCP calls are commands/effects:

- classify dialogue act
- generate freeform reply
- brainstorm an option
- elaborate the current candidate
- draft screenplay text
- propose ontology mutation
- propose document patch

The model is a command source, not the reducer.

### Perception Layer

The dialogue classifier should act as the perception layer for each turn.

Instead of returning only a coarse act, it should move toward a structured
turn interpretation such as:

```text
TurnInterpretation {
  act_type
  target
  confidence
}
```

Suggested first-cut `act_type` values:

- `Brainstorm`
- `Refine`
- `Commit`
- `Question`
- `Tangent`

Suggested first-cut `target` values:

- `CurrentCandidate`
- `NewTopic`
- `Screenplay`

Important rule:

- `Update` should route on structured interpretation
- it should not route on raw user text
- user language stays natural, but app logic stays typed

## Layer Split

### Interaction State

Immediate action generation should be driven by a small state surface:

- `current_topic`
- `current_candidate`
- `current_mode`

This is the main driver for suggestion actions.

### Working Memory

Working memory remains important, but it should support interaction state rather than replace it.

Good uses:

- rejected ideas
- open questions
- pinned decisions
- sidequests
- current thread continuity

### Canonical State

Canonical state stays outside the model and outside the provider:

- screenplay
- ontology
- links
- provenance
- sync candidates

Only mutation gateway actions should change it.

## Suggestion Action Pattern

Suggestion actions should follow a Codex-like pattern:

- typed actions are app-defined
- labels are contextual
- only a few actions are visible at once

Good typed actions:

- `ConfirmCurrent`
- `AlternativeCurrent`
- `ExpandCurrent`
- `AddCurrentToScreenplay`
- later:
  - `DeriveCharacterFromCurrent`
  - `DeriveEventFromCurrent`

The UI labels can vary by topic:

- `Use this setting`
- `Try another`
- `Deepen this`
- `Add this to screenplay`

The pattern is:

`state -> allowed typed actions -> user-facing labels`

not:

`raw chat -> arbitrary new buttons`

This action enum is the agent's effective action space.

Load-bearing rule:

- the agent can plan and reason freely
- it can only act through typed, app-defined actions

Every increase in agent capability should show up as:

- a new typed action

not as:

- freeform model output mutating state directly

## Query and Mutation Gateways

This runtime works best with separate read/write boundaries:

### `QueryGateway`

Read-only access to:

- working memory
- story snapshot
- screenplay
- open threads
- lint findings

### `MutationGateway`

Write-only access to:

- ontology proposals
- screenplay patch proposals
- commit current candidate
- defer candidate

The runtime should read through `QueryGateway` and write only through `MutationGateway`.

## Tool Registry

The runtime should expose a small typed tool registry to the provider/runtime layer.

Suggested categories:

### `QueryTool`

- `working_memory`
- `story_snapshot`
- `screenplay_section`

### `MutationTool`

- `propose_patch`
- `commit_candidate`
- `defer_candidate`

### `GenerationTool`

- `brainstorm`
- `elaborate`
- `draft_scene`
- later:
  - `critique`

Important rule:

- the model/runtime can only call tools from this registry
- the registry stays app-owned and typed
- this is the mutation boundary, even as autonomy increases

## Narrative Engine Role

The narrative engine should help with:

- brainstorming
- elaboration
- in-context response generation
- retrieval-backed continuity
- structured extraction when explicitly requested

It should not own:

- canonical state transitions
- session state
- commit policy

That means:

- frontstage: the engine behaves like a narrative guide / game master
- backstage: Lekhani owns the document, ontology, links, and mutation policy

## Ink / Narrative Game Inspiration

Useful concepts to borrow:

- `current thread`
- `main quest`
- `sidequests`
- `next move`
- `commit this thread`
- `set aside`

These are UX framings on top of the same backend:

- ontology
- screenplay
- working memory
- mutation/query gateways

The app does not need to become Ink.
It can borrow the thread and move vocabulary while keeping its own canonical state model.

## Drama Manager Layer

The Narrative Engine can be made more robust by borrowing the `Drama Manager`
pattern from interactive narrative systems such as *Façade*.

The useful translation for Lekhani is:

- watch the interaction state
- decide what should happen next
- surface the right next move without exposing the machinery

The important unit here is a `beat`:

- a discrete, app-authored dramatic interaction unit
- evaluated against current interaction state
- selected deterministically

### Role in the Elm Loop

The Narrative Engine should not be the reducer.

Instead:

```text
NarrativeEngine.evaluate : InteractionState -> EvaluationResult
```

Where:

```text
EvaluationResult {
  mode
  beat
  actions
  nudge
}
```

`Update` remains the place that decides what to do with the result.
The engine only evaluates and recommends.

This keeps the engine substitutable:

- deterministic today
- possibly richer later
- still isolated from `Update`

### InteractionState

The engine should evaluate only a narrow interaction surface.

Suggested first-cut shape:

```text
InteractionState {
  current_topic
  current_candidate
  current_mode
  thread_status
  turn_count
  last_dialogue_act
  open_sidequests
  working_memory_summary
}
```

Important rule:

- the evaluator should not inspect raw chat text
- the evaluator should not read canonical screenplay or ontology state directly
- if canonical context is needed, it should be distilled into summary fields before evaluation

### NarrativeMode

This works well as an explicit stance machine:

- `Idle`
- `Brainstorming`
- `Converging`
- `Elaborating`
- `Committing`
- `TunnelingSidequest`
- `Drifting`

Mode transitions should remain app-owned and deterministic:

```text
(current_mode, last_dialogue_act, thread_status) -> next_mode
```

The provider should not own these transitions.

### Beat Library

A beat is a named evaluable unit:

```text
Beat {
  id
  label
  precondition
  priority
  effect
}
```

Where `effect` can be:

- `SurfaceActions`
- `EmitNudge`
- `SuggestModeTransition`
- `OpenTunnel`
- `CloseTunnel`

Example first-cut beats:

- `CandidateReady`
- `DriftDetected`
- `ReadyToCommit`
- `Stalled`
- `SidequestOpened`
- `SidequestCloseable`

Important rule:

- start with a very small beat library
- keep each precondition pure and testable
- do not over-author the library on the first pass

### Evaluation Algorithm

The first implementation can stay simple and deterministic:

```text
evaluate(state):
  eligible_beats = beat_library
    .filter(precondition)
    .sort_by(priority desc)

  top_beat = eligible_beats.first()

  return EvaluationResult {
    mode
    beat
    actions
    nudge
  }
```

No model is needed in the evaluation loop.

### LLM Position

The LLM should execute what the engine selected, not select beats itself.

Pattern:

- engine surfaces actions
- user clicks or types into the current thread
- `Update` issues a command
- provider generates or elaborates content
- reducer updates state
- engine evaluates again

This means:

- the LLM can generate
- the app still controls flow

### Dialogue Act as Perception

Dialogue act classification remains the perception layer.

It should produce a structured interpretation such as:

```text
TurnInterpretation {
  act_type
  target
  confidence
}
```

The evaluator then reads:

- `last_dialogue_act`
- thread state
- stance

This keeps the thin crossing point intact:

- user language stays freeform
- app logic stays typed

### Ink Tunnel Pattern

This is a strong fit for sidequests.

When the user produces a tangent while a main thread is active:

- the engine may open a tunnel
- a sidequest becomes the active thread
- the main thread is suspended, not lost

Then when the sidequest resolves:

- the engine can close the tunnel
- control returns to the original thread

This is better than letting side ideas pollute one flat session stream.

### Why This Helps

This layer gives the runtime:

- a deterministic suggestion surface
- a loggable reason for each suggestion set
- testable preconditions
- authorial control over flow
- a place to encode narrative guidance without adding phrase heuristics

Most importantly:

- the provider does not invent actions
- the beat library remains app-owned

## Quest Framing

`Quest` is useful as product language layered on top of thread state.

Recommended split:

- backend/runtime term: `NarrativeThread`
- UX term: `Main quest` / `Sidequest`

This lets the product say:

- `Main quest: set the stage`
- `Sidequest: decide what breaks the monk's meditation`

without forcing the backend to adopt a heavier planning primitive too early.

Longer term, a thread may grow into a richer quest-like structure with:

- objective
- current step
- completed steps
- open questions
- parked ideas
- committed outcomes

But that should come after the first thread-scoped runtime is stable.

## Incremental Implementation Plan

### Step 1

Lock the current contract:

- plain chat is non-mutating
- suggestion actions are the mutation boundary

### Step 2

Introduce a session-shaped interaction model for Narrative:

- current topic
- current candidate
- current mode
- messages
- suggestions

Then evolve it into thread-scoped interaction state:

- `NarrativeThread`
- `AgentStance`
- thread-local current candidate
- thread-local open questions

### Step 3

Drive suggestion generation from interaction state, not raw chat text.

### Step 4

Keep provider output constrained to:

- dialogue act classification
- follow-up interpretation
- candidate elaboration
- contextual reply generation

Then move toward structured turn interpretation:

- `act_type`
- `target`
- `confidence`

### Step 5

Add retrieval only for committed or high-value story state after the user commits through an action like `Use this`.

### Step 6

Add the typed tool registry and keep all higher-autonomy behavior inside it.

### Step 7

Add the deterministic drama-manager evaluator:

- `InteractionState`
- `NarrativeMode`
- `Beat`
- `EvaluationResult`
- first small beat library

Keep it pure and provider-independent.

## Non-Goals

This plan does not require:

- auto-committing ordinary chat
- exposing raw ontology operations in Narrative
- replacing canonical state with LLM memory
- allowing the provider to own state transitions

## Success Criteria

This architecture is working when:

- plain chat never mutates state accidentally
- suggestions feel contextual and reliable
- providers are swappable
- stale memory does not hijack ordinary chat
- the runtime can be reasoned about as:

`Model -> Update -> Cmd -> View`

instead of:

`provider magic -> hidden state change`

And when:

- thread state is scoped cleanly
- stance constrains legal commands
- the action space remains small and typed
- the evaluator can explain which beat fired and why
