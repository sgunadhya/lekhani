# Lekhani: Agent Architecture with FM-RS, LLMs, and GOLEM

## Overview

Lekhani is a screenplay writing tool that uses a multi-agent architecture to bridge natural language understanding with structured narrative representation. The system combines:

1. **Foundation Models** (via `fm-rs`/Apple MLX) for natural language understanding
2. **GOLEM Ontology** for structured narrative representation  
3. **Agent Coordination** to maintain consistency between screenplay text and semantic graph

## Core Architecture

### 1. Language Model Integration

```
┌─────────────────────────────────────────────────────────┐
│                    Model Abstraction Layer               │
├─────────────────────────────────────────────────────────┤
│  Primary: fm-rs (Apple Foundation Models via MLX)       │
│  Fallback: llama-cpp-rs (cross-platform compatibility)  │
└─────────────────────────────────────────────────────────┘
                             │
┌─────────────────────────────────────────────────────────┐
│                  Agent Specialization                   │
├─────────────┬─────────────┬─────────────┬───────────────┤
│ Parser      │ Nudge       │ Relationship│ Consistency   │
│ Agent       │ Generator   │ Inference   │ Checker       │
└─────────────┴─────────────┴─────────────┴───────────────┘
```

**Model Selection Strategy:**
- macOS with Apple Silicon: Use `fm-rs` for native MLX acceleration
- Other platforms: Fall back to `llama-cpp-rs` with quantized models
- Configuration-driven model loading based on system capabilities

### 2. GOLEM-Based Agent System

Each agent specializes in different aspects of narrative understanding:

#### **Parser Agent**
- **Purpose**: Extract GOLEM entities from natural language descriptions
- **Input**: "Kanishka is a Kushan emperor, ambitious but conflicted"
- **Output**: 
  ```rust
  Character {
      id: UUID,
      name: "Kanishka",
      features: [
          Feature { category: Biographical, value: "Kushan emperor" },
          Feature { category: Psychological, value: "ambitious" },
          Feature { category: Psychological, value: "conflicted" }
      ]
  }
  ```
- **Implementation**: Fine-tuned prompts for entity extraction

#### **Nudge Generator Agent**
- **Purpose**: Suggest narrative improvements based on GOLEM graph analysis
- **Input**: Current screenplay state + GOLEM graph
- **Output**: Contextual suggestions like:
  - "Consider adding a scene showing Kanishka's internal conflict"
  - "Character X has no relationships with other major characters"
  - "Timeline shows gap between events A and B"
- **Implementation**: Graph analysis + LLM-based suggestion generation

#### **Relationship Inference Agent**
- **Purpose**: Infer implicit relationships between characters/events
- **Input**: Set of characters with features and co-occurrence in scenes
- **Output**: `SocialRelationship` with type and strength
- **Implementation**: Embedding similarity + rule-based inference

#### **Consistency Checker Agent**
- **Purpose**: Ensure screenplay text and GOLEM graph remain synchronized
- **Input**: Screenplay changes + corresponding GOLEM updates
- **Output**: Consistency warnings or automatic corrections
- **Implementation**: Cross-reference mapping validation

### 3. Agent Communication Protocol

```rust
enum AgentMessage {
    ParseCharacter { description: String },
    ParseEvent { description: String },
    GenerateNudge { context: NudgeContext },
    InferRelationships { characters: Vec<Character> },
    CheckConsistency { 
        screenplay_id: Uuid,
        changes: Vec<ScreenplayChange> 
    }
}

struct AgentResponse {
    request_id: Uuid,
    result: Result<AgentOutput, AgentError>,
    provenance: InferenceProvenance,
    confidence: f32,
}
```

### 4. LLM Prompt Engineering

#### Character Parser Prompt Template:
```
You are a narrative analysis assistant. Extract character information from the description.

Description: "{description}"

Return JSON with:
- name: Character name (extract if present)
- features: List of {category, value} pairs
- psychological_states: List of emotional states
- narrative_roles: List of narrative functions

Categories: Biographical, Psychological, Physical, Social, Motivational
```

#### Nudge Generation Prompt Template:
```
Analyze this narrative context and suggest improvements:

Screenplay: "{screenplay_title}"
Recent Changes: {recent_changes}
Characters: {character_summaries}
Events: {event_summaries}

Generate 1-3 specific, actionable suggestions for the writer.
Focus on character development, plot holes, or pacing issues.
```

### 5. Data Flow

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   User      │────▶   Chat      │────▶   Parser    │
│   Input     │    │   Interface │    │   Agent     │
└─────────────┘    └─────────────┘    └──────┬──────┘
                                              │
┌─────────────┐    ┌─────────────┐    ┌──────▼──────┐
│  Screenplay │◀───▶ Consistency │◀───▶   GOLEM     │
│   Editor    │    │   Checker   │    │   Graph     │
└─────────────┘    └─────────────┘    └──────┬──────┘
                                              │
┌─────────────┐    ┌─────────────┐    ┌──────▼──────┐
│   Timeline  │◀────   Nudge     │◀──── Relationship│
│   View      │    │   Generator │    │   Inference │
└─────────────┘    └─────────────┘    └─────────────┘
```

### 6. Implementation Status

**Current State (Simplified MVP):**
- ✅ Tauri desktop application with window
- ✅ Basic backend command handlers (`get_time`)
- ✅ Static HTML frontend with Tauri API testing
- ✅ Makefile for development workflow
- ✅ Project renamed to "Lekhani"

**Original Architecture Components (Temporarily Simplified):**
- ✅ Basic Tauri application structure
- 🔲 GOLEM ontology data structures (implemented but not integrated)
- 🔲 SQLite database schema (implemented but not integrated)
- 🔲 Tauri command handlers (stubs exist but simplified)
- ⚙️ Leptos frontend integration (in progress, Trunk build issues)
- 🔲 `fm-rs` dependency configuration (removed for now)
- 🔲 Agent message passing system
- 🔲 LLM model integration (`fm-rs` or `llama-cpp-rs`)
- 🔲 Agent specialization implementations
- 🔲 Cross-reference mapping system
- 🔲 Timeline visualization

**Note:** The current implementation is a minimal viable product to establish the development workflow. The full agent architecture will be incrementally integrated once the foundation is stable.

### 7. Configuration

**Current (Simplified):**
```toml
# Cargo.toml dependencies
[dependencies]
tauri = { version = "2.10.0", features = ["wry", "custom-protocol"] }
tauri-plugin-log = "2"
log = "0.4"
chrono = { version = "0.4", features = ["serde"] }

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

**Planned (Full Architecture):**
```toml
# Cargo.toml dependencies
[dependencies]
fm-rs = { version = "0.1", features = ["derive"], optional = true }
llama-cpp-rs = { version = "0.1", optional = true }
fountain = "0.1"
rusqlite = { version = "0.31", features = ["bundled"] }

[features]
apple-mlx = ["fm-rs"]
llama-fallback = ["llama-cpp-rs"]
```

### 8. Performance Considerations

1. **Model Loading**: Lazy loading of LLM models to reduce startup time
2. **Caching**: Cache common parsing results to reduce LLM calls
3. **Batching**: Batch similar agent requests when possible
4. **Fallback Strategy**: Graceful degradation when primary model unavailable

### 9. Testing Strategy

- Unit tests for each agent's core logic
- Integration tests for agent communication
- Mock LLM responses for reproducible testing
- Property-based testing for GOLEM graph consistency

### 10. Future Extensions

1. **Multi-model Ensemble**: Combine outputs from different models
2. **Fine-tuning**: Domain-specific fine-tuning on screenplay corpus
3. **Real-time Collaboration**: Multiple agents working simultaneously
4. **Export Formats**: Additional narrative analysis exports

## Development Commands

```bash
# Start development server
make dev

# Build for production
make build

# Clean build artifacts
make clean

# Kill running processes
make kill

# Format Rust code
make fmt

# Lint Rust code
make lint

# Run tests
make test

# Watch for changes and rebuild
make watch

# Setup development environment
make setup

# Build Leptos frontend only
make frontend-build

# Serve Leptos frontend only
make frontend-serve

# Quick build test
make quick-test

# Check dependencies
make check

# Show help
make help
```

## Agent Ownership Boundaries

When working in this repository, agents must respect ownership boundaries unless the user explicitly asks for integration work.

### Frontend-only work

- Allowed: `frontend/**`
- Allowed: Leptos components, styles, frontend state, frontend data handling
- Not allowed: `src-tauri/**`, `src-tauri/tauri.conf.json`, `Makefile`, workspace manifests, Trunk/Tauri integration

### Backend-only work

- Allowed: `src-tauri/**`
- Allowed: Tauri commands, Rust backend logic, persistence, model integration
- Not allowed: `frontend/**`, frontend HTML/CSS, Trunk config, integration wiring unless explicitly requested

### Integration-only work

- Allowed: `src-tauri/tauri.conf.json`, `Makefile`, root `Cargo.toml`, frontend build wiring, Tauri/Trunk orchestration
- Not allowed: product logic changes inside frontend components or backend domain logic unless explicitly requested

### Default rule

- If the user asks for a focused frontend or backend change, do not touch integration files.
- If integration must change to complete the task, stop and state that clearly before editing integration files.

## Leptos Component Source Of Truth

- The active Leptos component source of truth is `frontend/src/lib.rs`.
- Current visible startup rendering comes from the `App` and `HomePage` components in that file.
- Agents must not assume a separate frontend backup file exists unless they verify it on disk first.

## Verified Backup Files

- Verified backup present: `src-tauri/src/lib.rs.backup`
- This backup is a Tauri backend backup, not a Leptos component backup.
- No separate backup file containing Leptos components was found in this repository as of 2026-02-28.

## References

- GOLEM Ontology: "The GOLEM Ontology for Narrative and Fiction"
- Tauri: https://tauri.app/
- Leptos: https://leptos.dev/
- FM-RS: https://github.com/apple/fm-rs
- llama.cpp: https://github.com/ggerganov/llama.cpp

*Last updated: 2026-02-28*
