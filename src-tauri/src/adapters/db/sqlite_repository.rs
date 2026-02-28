use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::{
    AppError, NarrativeCharacter, NarrativeEvent, NarrativeMetrics, NarrativeSnapshot,
    OntologyEntity, OntologyEntityKind, OntologyGraph, OntologyRelationship,
    OntologyRelationshipKind, Screenplay, ScreenplayChange,
};
use crate::ports::{NarrativeRepository, ScreenplayRepository};
use uuid::Uuid;

pub struct SqliteScreenplayRepository {
    path: Mutex<PathBuf>,
}

const MIGRATIONS: [(&str, &str); 2] = [
    (
        "0001_init.sql",
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/migrations/0001_init.sql")),
    ),
    (
        "0002_normalize_screenplay_changes.sql",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0002_normalize_screenplay_changes.sql"
        )),
    ),
];

impl SqliteScreenplayRepository {
    pub fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        Self::initialize_path(path)?;
        Ok(Self {
            path: Mutex::new(path.to_path_buf()),
        })
    }

    pub fn current_path(&self) -> Result<PathBuf, AppError> {
        self.path
            .lock()
            .map(|path| path.clone())
            .map_err(|_| AppError::StatePoisoned("sqlite path lock poisoned"))
    }

    pub fn switch_path(&self, path: &Path) -> Result<(), AppError> {
        Self::initialize_path(path)
            .map_err(|_| AppError::StatePoisoned("failed to initialize selected project database"))?;
        let mut current_path = self
            .path
            .lock()
            .map_err(|_| AppError::StatePoisoned("sqlite path lock poisoned"))?;
        *current_path = path.to_path_buf();
        Ok(())
    }

    fn connection(&self) -> Result<Connection, AppError> {
        let path = self.current_path()?;
        let connection = Connection::open(&path)
            .map_err(|_| AppError::StatePoisoned("failed to open sqlite database"))?;
        Self::apply_migrations(&connection)
            .map_err(|_| AppError::StatePoisoned("failed to apply sqlite migrations"))?;
        Self::backfill_screenplay_changes(&connection)
            .map_err(|_| AppError::StatePoisoned("failed to normalize screenplay changes"))?;
        Ok(connection)
    }

    fn initialize_path(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        Self::apply_migrations(&connection)?;
        Self::backfill_screenplay_changes(&connection)?;
        Ok(())
    }

    fn apply_migrations(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version TEXT PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
        )?;

        for (version, sql) in MIGRATIONS {
            let already_applied: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
                [version],
                |row| row.get(0),
            )?;

            if already_applied {
                continue;
            }

            connection.execute_batch(sql)?;
            connection.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![version, Utc::now().to_rfc3339()],
            )?;
        }

        Ok(())
    }

    fn backfill_screenplay_changes(connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
        let mut statement = connection.prepare(
            "SELECT id, changes_json
             FROM screenplays
             WHERE changes_json != '[]' AND changes_json != ''",
        )?;

        let rows = statement.query_map([], |row| {
            let screenplay_id: String = row.get(0)?;
            let changes_json: String = row.get(1)?;
            Ok((screenplay_id, changes_json))
        })?;

        for row in rows {
            let (screenplay_id, changes_json) = row?;
            let existing_count: i64 = connection.query_row(
                "SELECT COUNT(1) FROM screenplay_changes WHERE screenplay_id = ?1",
                [screenplay_id.clone()],
                |db_row| db_row.get(0),
            )?;

            if existing_count > 0 {
                continue;
            }

            let changes: Vec<ScreenplayChange> = serde_json::from_str(&changes_json).unwrap_or_default();
            for change in changes {
                Self::insert_screenplay_change(connection, &screenplay_id, &change)
                    .map_err(|err| -> Box<dyn std::error::Error> { Box::new(err) })?;
            }
        }

        Ok(())
    }

    fn load_screenplay_changes(
        connection: &Connection,
        screenplay_id: &str,
    ) -> Result<Vec<ScreenplayChange>, AppError> {
        let mut statement = connection
            .prepare(
                "SELECT id, timestamp, author, change_type, range_start, range_end, new_text, old_text, provenance_id
                 FROM screenplay_changes
                 WHERE screenplay_id = ?1
                 ORDER BY timestamp ASC",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare screenplay changes query"))?;

        let rows = statement
            .query_map([screenplay_id], |row| {
                let id: String = row.get(0)?;
                let timestamp: String = row.get(1)?;
                let change_type: String = row.get(3)?;
                let provenance_id: Option<String> = row.get(8)?;

                Ok(ScreenplayChange {
                    id: parse_uuid_or_nil(&id),
                    timestamp: DateTime::parse_from_rfc3339(&timestamp)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    author: row.get(2)?,
                    change_type: change_type_from_str(&change_type),
                    range_start: row.get(4)?,
                    range_end: row.get(5)?,
                    new_text: row.get(6)?,
                    old_text: row.get(7)?,
                    provenance: provenance_id.as_deref().map(parse_uuid_or_nil),
                })
            })
            .map_err(|_| AppError::StatePoisoned("failed to query screenplay changes"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read screenplay changes"))
    }

    fn insert_screenplay_change(
        connection: &Connection,
        screenplay_id: &str,
        change: &ScreenplayChange,
    ) -> Result<(), AppError> {
        connection
            .execute(
                "INSERT INTO screenplay_changes
                    (id, screenplay_id, timestamp, author, change_type, range_start, range_end, new_text, old_text, provenance_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    change.id.to_string(),
                    screenplay_id,
                    change.timestamp.to_rfc3339(),
                    change.author,
                    change_type_to_str(&change.change_type),
                    change.range_start,
                    change.range_end,
                    change.new_text,
                    change.old_text,
                    change.provenance.map(|id| id.to_string()),
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to save screenplay change"))?;

        Ok(())
    }

    fn upsert_ontology_entity(
        connection: &Connection,
        entity: &OntologyEntity,
    ) -> Result<(), AppError> {
        connection
            .execute(
                "INSERT INTO ontology_entities (id, entity_kind, label, summary)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                    entity_kind = excluded.entity_kind,
                    label = excluded.label,
                    summary = excluded.summary",
                params![
                    entity.id.to_string(),
                    ontology_entity_kind_to_str(&entity.kind),
                    entity.label,
                    entity.summary,
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to save ontology entity"))?;

        Ok(())
    }

    fn upsert_projection_link(
        connection: &Connection,
        narrative_kind: &str,
        narrative_id: Uuid,
        ontology_entity_id: Uuid,
    ) -> Result<(), AppError> {
        let existing_id = connection
            .query_row(
                "SELECT id
                 FROM narrative_projection_links
                 WHERE narrative_kind = ?1 AND narrative_id = ?2",
                params![narrative_kind, narrative_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| AppError::StatePoisoned("failed to query projection link"))?;
        let projection_id = existing_id.unwrap_or_else(|| Uuid::new_v4().to_string());

        connection
            .execute(
                "INSERT INTO narrative_projection_links
                    (id, narrative_kind, narrative_id, ontology_entity_id, relationship_id)
                 VALUES (?1, ?2, ?3, ?4, NULL)
                 ON CONFLICT(id) DO UPDATE SET
                    narrative_kind = excluded.narrative_kind,
                    narrative_id = excluded.narrative_id,
                    ontology_entity_id = excluded.ontology_entity_id,
                    relationship_id = excluded.relationship_id",
                params![
                    projection_id,
                    narrative_kind,
                    narrative_id.to_string(),
                    ontology_entity_id.to_string(),
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to save projection link"))?;

        Ok(())
    }

    fn load_characters(connection: &Connection) -> Result<Vec<NarrativeCharacter>, AppError> {
        let mut statement = connection
            .prepare(
                "SELECT id, ontology_entity_id, name, summary, tags_json
                 FROM narrative_characters
                 ORDER BY name ASC",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare character query"))?;

        let rows = statement
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let ontology_entity_id: Option<String> = row.get(1)?;
                let tags_json: String = row.get(4)?;

                Ok(NarrativeCharacter {
                    id: parse_uuid_or_nil(&id),
                    ontology_entity_id: ontology_entity_id.as_deref().map(parse_uuid_or_nil),
                    name: row.get(2)?,
                    summary: row.get(3)?,
                    tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                })
            })
            .map_err(|_| AppError::StatePoisoned("failed to query characters"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read character rows"))
    }

    fn load_events(connection: &Connection) -> Result<Vec<NarrativeEvent>, AppError> {
        let mut statement = connection
            .prepare(
                "SELECT id, ontology_entity_id, title, summary
                 FROM narrative_events
                 ORDER BY title ASC",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare event query"))?;

        let rows = statement
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let ontology_entity_id: Option<String> = row.get(1)?;

                Ok(NarrativeEvent {
                    id: parse_uuid_or_nil(&id),
                    ontology_entity_id: ontology_entity_id.as_deref().map(parse_uuid_or_nil),
                    title: row.get(2)?,
                    summary: row.get(3)?,
                    participants: Vec::new(),
                })
            })
            .map_err(|_| AppError::StatePoisoned("failed to query events"))?;

        let mut events = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read event rows"))?;

        for event in &mut events {
            let mut participant_statement = connection
                .prepare(
                    "SELECT narrative_character_id
                     FROM narrative_event_participants
                     WHERE narrative_event_id = ?1",
                )
                .map_err(|_| AppError::StatePoisoned("failed to prepare participant query"))?;

            let participants = participant_statement
                .query_map([event.id.to_string()], |row| row.get::<_, String>(0))
                .map_err(|_| AppError::StatePoisoned("failed to query event participants"))?;

            event.participants = participants
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| AppError::StatePoisoned("failed to read participant rows"))?
                .into_iter()
                .map(|id| parse_uuid_or_nil(&id))
                .collect();
        }

        Ok(events)
    }

    fn load_ontology_graph(connection: &Connection) -> Result<OntologyGraph, AppError> {
        let mut entity_statement = connection
            .prepare(
                "SELECT id, entity_kind, label, summary
                 FROM ontology_entities
                 ORDER BY label ASC",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare ontology entity query"))?;

        let entities = entity_statement
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let kind: String = row.get(1)?;

                Ok(OntologyEntity {
                    id: parse_uuid_or_nil(&id),
                    kind: ontology_entity_kind_from_str(&kind),
                    label: row.get(2)?,
                    summary: row.get(3)?,
                })
            })
            .map_err(|_| AppError::StatePoisoned("failed to query ontology entities"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read ontology entity rows"))?;

        let mut relationship_statement = connection
            .prepare(
                "SELECT id, source_entity_id, target_entity_id, relationship_kind, summary
                 FROM ontology_relationships",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare ontology relationship query"))?;

        let relationships = relationship_statement
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let source_id: String = row.get(1)?;
                let target_id: String = row.get(2)?;
                let kind: String = row.get(3)?;

                Ok(OntologyRelationship {
                    id: parse_uuid_or_nil(&id),
                    source_id: parse_uuid_or_nil(&source_id),
                    target_id: parse_uuid_or_nil(&target_id),
                    kind: ontology_relationship_kind_from_str(&kind),
                    summary: row.get(4)?,
                })
            })
            .map_err(|_| AppError::StatePoisoned("failed to query ontology relationships"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read ontology relationship rows"))?;

        Ok(OntologyGraph {
            entities,
            relationships,
        })
    }

    fn load_projection_relationships(
        connection: &Connection,
    ) -> Result<Vec<OntologyRelationship>, AppError> {
        let mut statement = connection
            .prepare(
                "SELECT id, narrative_kind, narrative_id, ontology_entity_id
                 FROM narrative_projection_links",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare projection link query"))?;

        let rows = statement
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let narrative_kind: String = row.get(1)?;
                let narrative_id: String = row.get(2)?;
                let ontology_entity_id: String = row.get(3)?;
                let summary = match narrative_kind.as_str() {
                    "character" => "Narrative character projects to ontology character node",
                    "event" => "Narrative event projects to ontology event node",
                    _ => "Narrative item projects to ontology node",
                };

                Ok(OntologyRelationship {
                    id: parse_uuid_or_nil(&id),
                    source_id: parse_uuid_or_nil(&narrative_id),
                    target_id: parse_uuid_or_nil(&ontology_entity_id),
                    kind: OntologyRelationshipKind::NarrativeProjection,
                    summary: summary.to_string(),
                })
            })
            .map_err(|_| AppError::StatePoisoned("failed to query projection links"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read projection link rows"))
    }
}

impl ScreenplayRepository for SqliteScreenplayRepository {
    fn list(&self) -> Result<Vec<Screenplay>, AppError> {
        let connection = self.connection()?;
        type ScreenplayRow = (String, String, String, u64, String, String);
        let mut statement = connection
            .prepare(
                "SELECT id, title, fountain_text, version, changes_json, created_at, updated_at
                 FROM screenplays
                 ORDER BY updated_at DESC",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare screenplay query"))?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u64>(3)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|_| AppError::StatePoisoned("failed to query screenplays"))?;

        let rows = rows
            .collect::<Result<Vec<ScreenplayRow>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read screenplay rows"))?;
        drop(statement);

        rows.into_iter()
            .map(|(id, title, fountain_text, version, created_at, updated_at)| {
                let changes = Self::load_screenplay_changes(&connection, &id)?;

                Ok(Screenplay {
                    id: Uuid::parse_str(&id).unwrap_or_else(|_| Uuid::nil()),
                    title,
                    fountain_text,
                    version,
                    changes,
                    created_at: DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    updated_at: DateTime::parse_from_rfc3339(&updated_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })
            .collect()
    }

    fn save(&self, screenplay: Screenplay) -> Result<(), AppError> {
        let connection = self.connection()?;
        let changes_json = serde_json::to_string(&screenplay.changes)
            .map_err(|_| AppError::StatePoisoned("failed to serialize screenplay changes"))?;
        let screenplay_id = screenplay.id.to_string();

        connection
            .execute(
                "INSERT INTO screenplays
                    (id, title, fountain_text, version, changes_json, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    fountain_text = excluded.fountain_text,
                    version = excluded.version,
                    changes_json = excluded.changes_json,
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at",
                params![
                    screenplay.id.to_string(),
                    screenplay.title,
                    screenplay.fountain_text,
                    screenplay.version,
                    changes_json,
                    screenplay.created_at.to_rfc3339(),
                    screenplay.updated_at.to_rfc3339(),
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to save screenplay"))?;

        connection
            .execute(
                "DELETE FROM screenplay_changes WHERE screenplay_id = ?1",
                [screenplay_id.clone()],
            )
            .map_err(|_| AppError::StatePoisoned("failed to replace screenplay changes"))?;

        for change in &screenplay.changes {
            Self::insert_screenplay_change(&connection, &screenplay_id, change)?;
        }

        Ok(())
    }
}

impl NarrativeRepository for SqliteScreenplayRepository {
    fn save_character(&self, character: NarrativeCharacter) -> Result<NarrativeCharacter, AppError> {
        let connection = self.connection()?;
        let ontology_entity_id = character.ontology_entity_id.unwrap_or_else(Uuid::new_v4);
        let character = NarrativeCharacter {
            ontology_entity_id: Some(ontology_entity_id),
            ..character
        };
        let tags_json = serde_json::to_string(&character.tags)
            .map_err(|_| AppError::StatePoisoned("failed to serialize character tags"))?;
        let ontology_entity = OntologyEntity {
            id: ontology_entity_id,
            kind: OntologyEntityKind::Character,
            label: character.name.clone(),
            summary: character.summary.clone(),
        };

        Self::upsert_ontology_entity(&connection, &ontology_entity)?;
        connection
            .execute(
                "INSERT INTO narrative_characters (id, ontology_entity_id, name, summary, tags_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                    ontology_entity_id = excluded.ontology_entity_id,
                    name = excluded.name,
                    summary = excluded.summary,
                    tags_json = excluded.tags_json",
                params![
                    character.id.to_string(),
                    ontology_entity_id.to_string(),
                    character.name,
                    character.summary,
                    tags_json,
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to save narrative character"))?;

        Self::upsert_projection_link(&connection, "character", character.id, ontology_entity_id)?;

        Ok(character)
    }

    fn save_event(&self, event: NarrativeEvent) -> Result<NarrativeEvent, AppError> {
        let connection = self.connection()?;
        let ontology_entity_id = event.ontology_entity_id.unwrap_or_else(Uuid::new_v4);
        let event = NarrativeEvent {
            ontology_entity_id: Some(ontology_entity_id),
            ..event
        };
        let ontology_entity = OntologyEntity {
            id: ontology_entity_id,
            kind: OntologyEntityKind::Event,
            label: event.title.clone(),
            summary: event.summary.clone(),
        };

        Self::upsert_ontology_entity(&connection, &ontology_entity)?;
        connection
            .execute(
                "INSERT INTO narrative_events (id, ontology_entity_id, title, summary)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                    ontology_entity_id = excluded.ontology_entity_id,
                    title = excluded.title,
                    summary = excluded.summary",
                params![
                    event.id.to_string(),
                    ontology_entity_id.to_string(),
                    event.title,
                    event.summary,
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to save narrative event"))?;

        Self::upsert_projection_link(&connection, "event", event.id, ontology_entity_id)?;

        connection
            .execute(
                "DELETE FROM narrative_event_participants WHERE narrative_event_id = ?1",
                [event.id.to_string()],
            )
            .map_err(|_| AppError::StatePoisoned("failed to clear event participants"))?;
        connection
            .execute(
                "DELETE FROM ontology_relationships
                 WHERE target_entity_id = ?1 AND relationship_kind = ?2",
                params![
                    ontology_entity_id.to_string(),
                    ontology_relationship_kind_to_str(&OntologyRelationshipKind::ParticipantInEvent)
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to clear participant relationships"))?;

        for participant_id in &event.participants {
            connection
                .execute(
                    "INSERT INTO narrative_event_participants (narrative_event_id, narrative_character_id)
                     VALUES (?1, ?2)",
                    params![event.id.to_string(), participant_id.to_string()],
                )
                .map_err(|_| AppError::StatePoisoned("failed to save event participant"))?;

            let character_ontology_id: Option<String> = connection
                .query_row(
                    "SELECT ontology_entity_id
                     FROM narrative_characters
                     WHERE id = ?1",
                    [participant_id.to_string()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|_| AppError::StatePoisoned("failed to query participant ontology node"))?
                .flatten();

            if let Some(character_ontology_id) = character_ontology_id {
                connection
                    .execute(
                        "INSERT INTO ontology_relationships
                            (id, source_entity_id, target_entity_id, relationship_kind, summary)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![
                            Uuid::new_v4().to_string(),
                            character_ontology_id,
                            ontology_entity_id.to_string(),
                            ontology_relationship_kind_to_str(
                                &OntologyRelationshipKind::ParticipantInEvent
                            ),
                            "Character participates in event",
                        ],
                    )
                    .map_err(|_| {
                        AppError::StatePoisoned("failed to save participant relationship")
                    })?;
            }
        }

        Ok(event)
    }

    fn save_relationship(
        &self,
        relationship: OntologyRelationship,
    ) -> Result<OntologyRelationship, AppError> {
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO ontology_relationships
                    (id, source_entity_id, target_entity_id, relationship_kind, summary)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                    source_entity_id = excluded.source_entity_id,
                    target_entity_id = excluded.target_entity_id,
                    relationship_kind = excluded.relationship_kind,
                    summary = excluded.summary",
                params![
                    relationship.id.to_string(),
                    relationship.source_id.to_string(),
                    relationship.target_id.to_string(),
                    ontology_relationship_kind_to_str(&relationship.kind),
                    relationship.summary,
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to save ontology relationship"))?;

        Ok(relationship)
    }

    fn load_snapshot(&self) -> Result<NarrativeSnapshot, AppError> {
        let connection = self.connection()?;
        let characters = Self::load_characters(&connection)?;
        let events = Self::load_events(&connection)?;
        let ontology_graph = Self::load_ontology_graph(&connection)?;
        let projection_relationships = Self::load_projection_relationships(&connection)?;

        Ok(NarrativeSnapshot {
            metrics: NarrativeMetrics {
                scene_count: 0,
                character_count: characters.len(),
                event_count: events.len(),
            },
            characters,
            events,
            projection_relationships,
            ontology_graph,
        })
    }
}

fn parse_uuid_or_nil(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap_or_else(|_| Uuid::nil())
}

fn ontology_entity_kind_to_str(kind: &OntologyEntityKind) -> &'static str {
    match kind {
        OntologyEntityKind::Character => "character",
        OntologyEntityKind::Event => "event",
    }
}

fn ontology_entity_kind_from_str(value: &str) -> OntologyEntityKind {
    match value {
        "event" => OntologyEntityKind::Event,
        _ => OntologyEntityKind::Character,
    }
}

fn ontology_relationship_kind_to_str(kind: &OntologyRelationshipKind) -> &'static str {
    match kind {
        OntologyRelationshipKind::NarrativeProjection => "narrative_projection",
        OntologyRelationshipKind::ParticipantInEvent => "participant_in_event",
        OntologyRelationshipKind::SupportsCharacter => "supports_character",
        OntologyRelationshipKind::OpposesCharacter => "opposes_character",
        OntologyRelationshipKind::AdvisesCharacter => "advises_character",
    }
}

fn ontology_relationship_kind_from_str(value: &str) -> OntologyRelationshipKind {
    match value {
        "participant_in_event" => OntologyRelationshipKind::ParticipantInEvent,
        "supports_character" => OntologyRelationshipKind::SupportsCharacter,
        "opposes_character" => OntologyRelationshipKind::OpposesCharacter,
        "advises_character" => OntologyRelationshipKind::AdvisesCharacter,
        _ => OntologyRelationshipKind::NarrativeProjection,
    }
}

fn change_type_to_str(value: &crate::domain::ChangeType) -> &'static str {
    match value {
        crate::domain::ChangeType::Insert => "insert",
        crate::domain::ChangeType::Delete => "delete",
        crate::domain::ChangeType::Replace => "replace",
    }
}

fn change_type_from_str(value: &str) -> crate::domain::ChangeType {
    match value {
        "delete" => crate::domain::ChangeType::Delete,
        "replace" => crate::domain::ChangeType::Replace,
        _ => crate::domain::ChangeType::Insert,
    }
}
