CREATE INDEX IF NOT EXISTS idx_screenplay_changes_timestamp
    ON screenplay_changes(timestamp);

CREATE INDEX IF NOT EXISTS idx_narrative_event_participants_character_id
    ON narrative_event_participants(narrative_character_id);

CREATE INDEX IF NOT EXISTS idx_projection_links_ontology_entity_id
    ON narrative_projection_links(ontology_entity_id);
