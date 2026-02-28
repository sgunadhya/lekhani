PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS screenplays (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    fountain_text TEXT NOT NULL,
    version INTEGER NOT NULL,
    changes_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS screenplay_changes (
    id TEXT PRIMARY KEY,
    screenplay_id TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    author TEXT NOT NULL,
    change_type TEXT NOT NULL,
    range_start INTEGER NOT NULL,
    range_end INTEGER NOT NULL,
    new_text TEXT NOT NULL,
    old_text TEXT NOT NULL,
    provenance_id TEXT,
    FOREIGN KEY (screenplay_id) REFERENCES screenplays(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS ontology_entities (
    id TEXT PRIMARY KEY,
    entity_kind TEXT NOT NULL,
    label TEXT NOT NULL,
    summary TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS ontology_relationships (
    id TEXT PRIMARY KEY,
    source_entity_id TEXT NOT NULL,
    target_entity_id TEXT NOT NULL,
    relationship_kind TEXT NOT NULL,
    summary TEXT NOT NULL,
    FOREIGN KEY (source_entity_id) REFERENCES ontology_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (target_entity_id) REFERENCES ontology_entities(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS narrative_characters (
    id TEXT PRIMARY KEY,
    ontology_entity_id TEXT,
    name TEXT NOT NULL,
    summary TEXT NOT NULL,
    tags_json TEXT NOT NULL,
    FOREIGN KEY (ontology_entity_id) REFERENCES ontology_entities(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS narrative_events (
    id TEXT PRIMARY KEY,
    ontology_entity_id TEXT,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    FOREIGN KEY (ontology_entity_id) REFERENCES ontology_entities(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS narrative_event_participants (
    narrative_event_id TEXT NOT NULL,
    narrative_character_id TEXT NOT NULL,
    PRIMARY KEY (narrative_event_id, narrative_character_id),
    FOREIGN KEY (narrative_event_id) REFERENCES narrative_events(id) ON DELETE CASCADE,
    FOREIGN KEY (narrative_character_id) REFERENCES narrative_characters(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS narrative_projection_links (
    id TEXT PRIMARY KEY,
    narrative_kind TEXT NOT NULL,
    narrative_id TEXT NOT NULL,
    ontology_entity_id TEXT NOT NULL,
    relationship_id TEXT,
    FOREIGN KEY (ontology_entity_id) REFERENCES ontology_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (relationship_id) REFERENCES ontology_relationships(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_screenplay_changes_screenplay_id
    ON screenplay_changes(screenplay_id);

CREATE INDEX IF NOT EXISTS idx_ontology_relationships_source_entity_id
    ON ontology_relationships(source_entity_id);

CREATE INDEX IF NOT EXISTS idx_ontology_relationships_target_entity_id
    ON ontology_relationships(target_entity_id);

CREATE INDEX IF NOT EXISTS idx_narrative_projection_links_narrative_ref
    ON narrative_projection_links(narrative_kind, narrative_id);
