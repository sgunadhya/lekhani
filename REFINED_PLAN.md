# Refined Architecture: Mathura Struggle Screenplay Organizer

## Core Principles
1. **Screenplay-First**: Fountain screenplay text is the primary artifact.
2. **GOLEM as Semantic Hydration**: GOLEM ontology captures narrative semantics derived from screenplay + user intent.
3. **Dual Synchronization**: Changes in screenplay update GOLEM graph; NLP chat edits can modify both.
4. **Audit Trail**: Every edit (chat or direct) creates a versioned change record.
5. **Transparent Cross-Reference**: Mapping between screenplay elements and GOLEM entities is automatic but hidden.

## Data Model

### 1. Screenplay State
```rust
struct Screenplay {
    id: Uuid,
    title: String,
    fountain_text: String,  // Raw Fountain format
    parsed: ParsedScreenplay,  // fountain-rs AST
    version: u64,
    changes: Vec<ScreenplayChange>,
}

struct ScreenplayChange {
    id: Uuid,
    timestamp: DateTime,
    author: String,  // "user" or "system"
    change_type: ChangeType,  // Insert, Delete, Replace
    range: (usize, usize),  // Character range in fountain_text
    new_text: String,
    old_text: String,
    provenance: Option<InferenceId>,  // Linked LLM inference
}
```

### 2. GOLEM Semantic Graph
- **Characters**: `G1_Character` with features, relationships.
- **Events**: `G5_Narrative_Event` linked to scenes.
- **Relationships**: `G4_Social_Relationship` between characters.
- **Psychological States**: `G3_Psychological_State` attached to characters.
- **Narrative Units**: `G9_Narrative_Unit` mapping to scenes/beats.
- **Inference Provenance**: Track which LLM inference created each entity.

### 3. Cross-Reference Mapping
```rust
struct ScreenplayToGolemMapping {
    scene_to_events: HashMap<SceneId, Vec<EventId>>,
    scene_to_characters: HashMap<SceneId, Vec<CharacterId>>,
    dialogue_to_psychological_state: HashMap<DialogueId, PsychologicalStateId>,
    action_to_event: HashMap<ActionId, EventId>,
    character_mentions: HashMap<CharacterId, Vec<TextPosition>>,
}
```

## System Architecture

### Components:
1. **Screenplay Store**: Manages Fountain text, versioning, change history.
2. **GOLEM Graph**: Semantic database (SQLite) with embeddings.
3. **Parser Pipeline**: Fountain → AST → GOLEM entity extraction.
4. **LLM Integration**: `fm-rs` (Apple) / `llama-cpp-rs` (fallback) for NLP parsing and nudge generation.
5. **Chat Interface**: Natural language input that can:
   - Add/update GOLEM entities
   - Suggest screenplay edits
   - Execute direct screenplay edits
6. **Screenplay Editor**: Fountain-aware text editor with:
   - Syntax highlighting
   - Inline suggestions from GOLEM
   - Real-time parsing
7. **Visualization Engine**: Timeline of events/character arcs derived from GOLEM graph.

## Workflow Examples

### 1. User adds character via chat:
```
User: "Kanishka is a Kushan emperor, ambitious but conflicted about invading Mathura."
```
**System:**
- LLM parses → `Character` with `features: ["ambitious", "conflicted"]`
- Validate: No existing Kanishka? Create new.
- Suggest: "Add introductory scene for Kanishka?" (if user agrees, generate scene text and insert into screenplay)
- Update GOLEM graph, embeddings, cross-reference mapping.

### 2. User edits screenplay directly:
```
User adds scene: "INT. PALACE - DAY\nKanishka stares at map of Mathura, conflicted."
```
**System:**
- Parse scene → detect character "Kanishka", location "palace", psychological state "conflicted"
- Update GOLEM: Link scene to existing Kanishka character, create psychological state.
- Update cross-reference mapping.

### 3. Nudge generation:
- System analyzes GOLEM graph: "Kanishka has relationship with 'Mathura priests' but no events showing this."
- Generate nudge: "Consider adding scene showing Kanishka meeting with Mathura priests."
- If accepted, generate scene template and insert into screenplay.

## Implementation Phases

### Phase 1: Foundation (Week 1-2)
- [ ] Setup Tauri + Leptos project
- [ ] Basic Screenplay struct with versioning
- [ ] Fountain parsing (`fountain-rs`)
- [ ] SQLite schema for GOLEM entities
- [ ] Simple editor UI (textarea with Fountain highlighting)

### Phase 2: LLM Integration (Week 3-4)
- [ ] `fm-rs` integration (Apple)
- [ ] `llama-cpp-rs` fallback
- [ ] NLP parsing: text → GOLEM JSON
- [ ] Validation logic (consistency checking)
- [ ] Basic chat UI

### Phase 3: Cross-Reference & Sync (Week 5-6)
- [ ] Parser pipeline: Fountain AST → GOLEM extraction
- [ ] Cross-reference mapping tables
- [ ] Screenplay edit → GOLEM update
- [ ] GOLEM change → screenplay suggestion
- [ ] Audit trail recording

### Phase 4: Advanced Features (Week 7-8)
- [ ] Embeddings (`fastembed`) for semantic search
- [ ] Nudge generation engine
- [ ] Timeline visualization (canvas)
- [ ] Relationship graph visualization
- [ ] Export/import Fountain files

### Phase 5: Polish (Week 9-10)
- [ ] Performance optimization
- [ ] Cross-platform testing
- [ ] UX refinement
- [ ] Documentation

## Technical Stack
- **Frontend**: Leptos (Rust WebAssembly) + Tauri
- **Backend**: Rust (same binary)
- **Database**: SQLite with `rusqlite`
- **LLM**: `fm-rs` (Apple FoundationModels) primary, `llama-cpp-rs` fallback
- **Embeddings**: `fastembed`
- **Screenplay Parsing**: `fountain-rs`
- **UI Components**: Custom Leptos components, `leptos-use` for utilities

## Key Challenges & Mitigations
1. **LLM Availability**: `fm-rs` requires macOS 26+. Fallback to local llama.cpp model.
2. **Performance**: Parsing large screenplays on every edit. Use incremental parsing, debounce.
3. **Complexity Hiding**: GOLEM ontology is complex. Expose only through simple nudges.
4. **Sync Conflicts**: User edits screenplay while chat suggests changes. Use operational transformation or simple lock.
5. **Memory**: Large embeddings. Use on-disk vector storage, incremental indexing.

## Next Steps
1. **Approve this refined architecture**
2. **Create Tauri+Leptos project**
3. **Implement Phase 1 foundations**
