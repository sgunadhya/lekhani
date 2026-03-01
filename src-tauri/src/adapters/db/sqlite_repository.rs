use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::{
    AppError, AssistantIntent, CandidateStatus, DocumentOntologyLink, FocusItem, LintFinding,
    LintScope, LintSeverity, LintStatus, NarrativeCharacter, NarrativeEvent, NarrativeMetrics,
    NarrativeSnapshot,
    OntologyEntity, OntologyEntityKind, OntologyGraph, OntologyRelationship,
    OntologyRelationshipKind, ProvenanceRecord, Screenplay, ScreenplayChange, SyncActionKind,
    SyncCandidate, SyncConflict, SyncRun, SyncRunStatus, SyncSourceKind, SyncTargetKind,
    SyncTargetLayer, WorkingMemory,
};
use crate::ports::{
    CandidateRepository, ConflictRepository, LinkRepository, LintRepository,
    NarrativeRepository, ProvenanceRepository, ScreenplayRepository, SyncRunRepository,
    WorkingMemoryRepository,
};
use uuid::Uuid;

pub struct SqliteScreenplayRepository {
    path: Mutex<PathBuf>,
}

const MIGRATIONS: [(&str, &str); 5] = [
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
    (
        "0003_sync_control.sql",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0003_sync_control.sql"
        )),
    ),
    (
        "0004_assistant_memory.sql",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0004_assistant_memory.sql"
        )),
    ),
    (
        "0005_story_backlog.sql",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/migrations/0005_story_backlog.sql"
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
        Self::ensure_assistant_memory_columns(&connection)
            .map_err(|_| AppError::StatePoisoned("failed to update assistant memory schema"))?;
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
        Self::ensure_assistant_memory_columns(&connection)?;
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

    fn ensure_assistant_memory_columns(
        connection: &Connection,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut statement = connection.prepare("PRAGMA table_info(assistant_working_memory)")?;
        let columns = statement
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;

        if !columns.iter().any(|column| column == "constraints_json") {
            connection.execute_batch(
                "ALTER TABLE assistant_working_memory
                 ADD COLUMN constraints_json TEXT NOT NULL DEFAULT '[]';",
            )?;
        }

        if !columns.iter().any(|column| column == "conversation_mode") {
            connection.execute_batch(
                "ALTER TABLE assistant_working_memory
                 ADD COLUMN conversation_mode TEXT NOT NULL DEFAULT 'Brainstorming';",
            )?;
        }
        if !columns.iter().any(|column| column == "conversation_topic") {
            connection.execute_batch(
                "ALTER TABLE assistant_working_memory
                 ADD COLUMN conversation_topic TEXT NOT NULL DEFAULT 'General';",
            )?;
        }

        Ok(())
    }

    pub fn list_recent_sync_runs(&self, limit: usize) -> Result<Vec<SyncRun>, AppError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, source_kind, source_ref, document_version, ontology_version, status, created_at, completed_at
                 FROM sync_runs
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare recent sync run query"))?;

        let rows = statement
            .query_map([limit as i64], |row| {
                let id: String = row.get(0)?;
                let source_kind: String = row.get(1)?;
                let created_at: String = row.get(6)?;
                let completed_at: Option<String> = row.get(7)?;

                Ok(SyncRun {
                    id: parse_uuid_or_nil(&id),
                    source_kind: sync_source_kind_from_str(&source_kind),
                    source_ref: row.get(2)?,
                    document_version: row.get(3)?,
                    ontology_version: row.get(4)?,
                    status: sync_run_status_from_str(&row.get::<_, String>(5)?),
                    created_at: parse_datetime_or_now(&created_at),
                    completed_at: completed_at.as_deref().map(parse_datetime_or_now),
                })
            })
            .map_err(|_| AppError::StatePoisoned("failed to query recent sync runs"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read recent sync runs"))
    }

    pub fn list_recent_provenance(&self, limit: usize) -> Result<Vec<ProvenanceRecord>, AppError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, sync_run_id, source_kind, source_ref, derived_kind, derived_ref, confidence, notes, created_at
                 FROM provenance_records
                 ORDER BY created_at DESC
                 LIMIT ?1",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare recent provenance query"))?;

        let rows = statement
            .query_map([limit as i64], |row| {
                let id: String = row.get(0)?;
                let sync_run_id: String = row.get(1)?;
                let source_kind: String = row.get(2)?;
                let created_at: String = row.get(8)?;

                Ok(ProvenanceRecord {
                    id: parse_uuid_or_nil(&id),
                    sync_run_id: parse_uuid_or_nil(&sync_run_id),
                    source_kind: sync_source_kind_from_str(&source_kind),
                    source_ref: row.get(3)?,
                    derived_kind: row.get(4)?,
                    derived_ref: row.get(5)?,
                    confidence: row.get(6)?,
                    notes: row.get(7)?,
                    created_at: parse_datetime_or_now(&created_at),
                })
            })
            .map_err(|_| AppError::StatePoisoned("failed to query recent provenance"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read recent provenance"))
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

    fn save_entity(&self, entity: OntologyEntity) -> Result<OntologyEntity, AppError> {
        let connection = self.connection()?;
        Self::upsert_ontology_entity(&connection, &entity)?;
        Ok(entity)
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

    fn clear_all(&self) -> Result<(), AppError> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction()
            .map_err(|_| AppError::StatePoisoned("failed to start narrative reset transaction"))?;

        transaction
            .execute("DELETE FROM narrative_event_participants", [])
            .map_err(|_| AppError::StatePoisoned("failed to clear event participants"))?;
        transaction
            .execute("DELETE FROM narrative_projection_links", [])
            .map_err(|_| AppError::StatePoisoned("failed to clear projection links"))?;
        transaction
            .execute("DELETE FROM ontology_relationships", [])
            .map_err(|_| AppError::StatePoisoned("failed to clear ontology relationships"))?;
        transaction
            .execute("DELETE FROM narrative_events", [])
            .map_err(|_| AppError::StatePoisoned("failed to clear narrative events"))?;
        transaction
            .execute("DELETE FROM narrative_characters", [])
            .map_err(|_| AppError::StatePoisoned("failed to clear narrative characters"))?;
        transaction
            .execute("DELETE FROM ontology_entities", [])
            .map_err(|_| AppError::StatePoisoned("failed to clear ontology entities"))?;

        transaction
            .commit()
            .map_err(|_| AppError::StatePoisoned("failed to commit narrative reset"))?;

        Ok(())
    }
}

impl SyncRunRepository for SqliteScreenplayRepository {
    fn create_run(&self, run: SyncRun) -> Result<SyncRun, AppError> {
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO sync_runs
                    (id, source_kind, source_ref, document_version, ontology_version, status, created_at, completed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    run.id.to_string(),
                    sync_source_kind_to_str(&run.source_kind),
                    run.source_ref,
                    run.document_version,
                    run.ontology_version,
                    sync_run_status_to_str(&run.status),
                    run.created_at.to_rfc3339(),
                    run.completed_at.map(|value| value.to_rfc3339()),
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to create sync run"))?;

        Ok(run)
    }

    fn update_run(&self, run: SyncRun) -> Result<SyncRun, AppError> {
        let connection = self.connection()?;
        connection
            .execute(
                "UPDATE sync_runs
                 SET source_kind = ?2,
                     source_ref = ?3,
                     document_version = ?4,
                     ontology_version = ?5,
                     status = ?6,
                     created_at = ?7,
                     completed_at = ?8
                 WHERE id = ?1",
                params![
                    run.id.to_string(),
                    sync_source_kind_to_str(&run.source_kind),
                    run.source_ref,
                    run.document_version,
                    run.ontology_version,
                    sync_run_status_to_str(&run.status),
                    run.created_at.to_rfc3339(),
                    run.completed_at.map(|value| value.to_rfc3339()),
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to update sync run"))?;

        Ok(run)
    }

    fn get_run(&self, run_id: &str) -> Result<Option<SyncRun>, AppError> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT id, source_kind, source_ref, document_version, ontology_version, status, created_at, completed_at
                 FROM sync_runs
                 WHERE id = ?1",
                [run_id],
                |row| {
                    let id: String = row.get(0)?;
                    let source_kind: String = row.get(1)?;
                    let created_at: String = row.get(6)?;
                    let completed_at: Option<String> = row.get(7)?;

                    Ok(SyncRun {
                        id: parse_uuid_or_nil(&id),
                        source_kind: sync_source_kind_from_str(&source_kind),
                        source_ref: row.get(2)?,
                        document_version: row.get(3)?,
                        ontology_version: row.get(4)?,
                        status: sync_run_status_from_str(&row.get::<_, String>(5)?),
                        created_at: parse_datetime_or_now(&created_at),
                        completed_at: completed_at.as_deref().map(parse_datetime_or_now),
                    })
                },
            )
            .optional()
            .map_err(|_| AppError::StatePoisoned("failed to query sync run"))
    }
}

impl CandidateRepository for SqliteScreenplayRepository {
    fn create_candidate(&self, candidate: SyncCandidate) -> Result<SyncCandidate, AppError> {
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO sync_candidates
                    (id, sync_run_id, source_kind, source_ref, target_layer, target_kind, action_kind, status, confidence, title, summary, payload_json, evidence_json, created_at, resolved_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    candidate.id.to_string(),
                    candidate.sync_run_id.to_string(),
                    sync_source_kind_to_str(&candidate.source_kind),
                    candidate.source_ref,
                    sync_target_layer_to_str(&candidate.target_layer),
                    sync_target_kind_to_str(&candidate.target_kind),
                    sync_action_kind_to_str(&candidate.action_kind),
                    candidate_status_to_str(&candidate.status),
                    candidate.confidence,
                    candidate.title,
                    candidate.summary,
                    candidate.payload_json,
                    candidate.evidence_json,
                    candidate.created_at.to_rfc3339(),
                    candidate.resolved_at.map(|value| value.to_rfc3339()),
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to create sync candidate"))?;

        Ok(candidate)
    }

    fn update_candidate(&self, candidate: SyncCandidate) -> Result<SyncCandidate, AppError> {
        let connection = self.connection()?;
        connection
            .execute(
                "UPDATE sync_candidates
                 SET sync_run_id = ?2,
                     source_kind = ?3,
                     source_ref = ?4,
                     target_layer = ?5,
                     target_kind = ?6,
                     action_kind = ?7,
                     status = ?8,
                     confidence = ?9,
                     title = ?10,
                     summary = ?11,
                     payload_json = ?12,
                     evidence_json = ?13,
                     created_at = ?14,
                     resolved_at = ?15
                 WHERE id = ?1",
                params![
                    candidate.id.to_string(),
                    candidate.sync_run_id.to_string(),
                    sync_source_kind_to_str(&candidate.source_kind),
                    candidate.source_ref,
                    sync_target_layer_to_str(&candidate.target_layer),
                    sync_target_kind_to_str(&candidate.target_kind),
                    sync_action_kind_to_str(&candidate.action_kind),
                    candidate_status_to_str(&candidate.status),
                    candidate.confidence,
                    candidate.title,
                    candidate.summary,
                    candidate.payload_json,
                    candidate.evidence_json,
                    candidate.created_at.to_rfc3339(),
                    candidate.resolved_at.map(|value| value.to_rfc3339()),
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to update sync candidate"))?;

        Ok(candidate)
    }

    fn list_pending_candidates(&self) -> Result<Vec<SyncCandidate>, AppError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, sync_run_id, source_kind, source_ref, target_layer, target_kind, action_kind, status, confidence, title, summary, payload_json, evidence_json, created_at, resolved_at
                 FROM sync_candidates
                 WHERE status IN ('draft', 'ready', 'conflicted')
                 ORDER BY created_at ASC",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare sync candidate query"))?;

        let rows = statement
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let sync_run_id: String = row.get(1)?;
                let source_kind: String = row.get(2)?;
                let target_layer: String = row.get(4)?;
                let target_kind: String = row.get(5)?;
                let action_kind: String = row.get(6)?;
                let status: String = row.get(7)?;
                let created_at: String = row.get(13)?;
                let resolved_at: Option<String> = row.get(14)?;

                Ok(SyncCandidate {
                    id: parse_uuid_or_nil(&id),
                    sync_run_id: parse_uuid_or_nil(&sync_run_id),
                    source_kind: sync_source_kind_from_str(&source_kind),
                    source_ref: row.get(3)?,
                    target_layer: sync_target_layer_from_str(&target_layer),
                    target_kind: sync_target_kind_from_str(&target_kind),
                    action_kind: sync_action_kind_from_str(&action_kind),
                    status: candidate_status_from_str(&status),
                    confidence: row.get(8)?,
                    title: row.get(9)?,
                    summary: row.get(10)?,
                    payload_json: row.get(11)?,
                    evidence_json: row.get(12)?,
                    created_at: parse_datetime_or_now(&created_at),
                    resolved_at: resolved_at.as_deref().map(parse_datetime_or_now),
                })
            })
            .map_err(|_| AppError::StatePoisoned("failed to query sync candidates"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read sync candidate rows"))
    }
}

impl ConflictRepository for SqliteScreenplayRepository {
    fn create_conflict(&self, conflict: SyncConflict) -> Result<SyncConflict, AppError> {
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO sync_conflicts
                    (id, candidate_id, conflict_kind, summary, details_json, status, created_at, resolved_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    conflict.id.to_string(),
                    conflict.candidate_id.to_string(),
                    conflict_kind_to_str(&conflict.conflict_kind),
                    conflict.summary,
                    conflict.details_json,
                    candidate_status_to_str(&conflict.status),
                    conflict.created_at.to_rfc3339(),
                    conflict.resolved_at.map(|value| value.to_rfc3339()),
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to create sync conflict"))?;

        Ok(conflict)
    }

    fn list_open_conflicts(&self) -> Result<Vec<SyncConflict>, AppError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, candidate_id, conflict_kind, summary, details_json, status, created_at, resolved_at
                 FROM sync_conflicts
                 WHERE status = 'conflicted'
                 ORDER BY created_at ASC",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare sync conflict query"))?;

        let rows = statement
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let candidate_id: String = row.get(1)?;
                let conflict_kind: String = row.get(2)?;
                let status: String = row.get(5)?;
                let created_at: String = row.get(6)?;
                let resolved_at: Option<String> = row.get(7)?;

                Ok(SyncConflict {
                    id: parse_uuid_or_nil(&id),
                    candidate_id: parse_uuid_or_nil(&candidate_id),
                    conflict_kind: conflict_kind_from_str(&conflict_kind),
                    summary: row.get(3)?,
                    details_json: row.get(4)?,
                    status: candidate_status_from_str(&status),
                    created_at: parse_datetime_or_now(&created_at),
                    resolved_at: resolved_at.as_deref().map(parse_datetime_or_now),
                })
            })
            .map_err(|_| AppError::StatePoisoned("failed to query sync conflicts"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read sync conflict rows"))
    }
}

impl ProvenanceRepository for SqliteScreenplayRepository {
    fn create_record(&self, record: ProvenanceRecord) -> Result<ProvenanceRecord, AppError> {
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO provenance_records
                    (id, sync_run_id, source_kind, source_ref, derived_kind, derived_ref, confidence, notes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    record.id.to_string(),
                    record.sync_run_id.to_string(),
                    sync_source_kind_to_str(&record.source_kind),
                    record.source_ref,
                    record.derived_kind,
                    record.derived_ref,
                    record.confidence,
                    record.notes,
                    record.created_at.to_rfc3339(),
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to create provenance record"))?;

        Ok(record)
    }

    fn list_for_run(&self, run_id: &str) -> Result<Vec<ProvenanceRecord>, AppError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, sync_run_id, source_kind, source_ref, derived_kind, derived_ref, confidence, notes, created_at
                 FROM provenance_records
                 WHERE sync_run_id = ?1
                 ORDER BY created_at ASC",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare provenance query"))?;

        let rows = statement
            .query_map([run_id], |row| {
                let id: String = row.get(0)?;
                let sync_run_id: String = row.get(1)?;
                let source_kind: String = row.get(2)?;
                let created_at: String = row.get(8)?;

                Ok(ProvenanceRecord {
                    id: parse_uuid_or_nil(&id),
                    sync_run_id: parse_uuid_or_nil(&sync_run_id),
                    source_kind: sync_source_kind_from_str(&source_kind),
                    source_ref: row.get(3)?,
                    derived_kind: row.get(4)?,
                    derived_ref: row.get(5)?,
                    confidence: row.get(6)?,
                    notes: row.get(7)?,
                    created_at: parse_datetime_or_now(&created_at),
                })
            })
            .map_err(|_| AppError::StatePoisoned("failed to query provenance records"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read provenance rows"))
    }
}

impl LinkRepository for SqliteScreenplayRepository {
    fn upsert_link(&self, link: DocumentOntologyLink) -> Result<DocumentOntologyLink, AppError> {
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO document_ontology_links
                    (id, document_ref, ontology_ref, link_kind, confidence, status, provenance_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                    document_ref = excluded.document_ref,
                    ontology_ref = excluded.ontology_ref,
                    link_kind = excluded.link_kind,
                    confidence = excluded.confidence,
                    status = excluded.status,
                    provenance_id = excluded.provenance_id,
                    created_at = excluded.created_at,
                    updated_at = excluded.updated_at",
                params![
                    link.id.to_string(),
                    link.document_ref,
                    link.ontology_ref,
                    link.link_kind,
                    link.confidence,
                    link_status_to_str(&link.status),
                    link.provenance_id.map(|value| value.to_string()),
                    link.created_at.to_rfc3339(),
                    link.updated_at.to_rfc3339(),
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to upsert document ontology link"))?;

        Ok(link)
    }

    fn find_for_document_ref(
        &self,
        document_ref: &str,
    ) -> Result<Vec<DocumentOntologyLink>, AppError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, document_ref, ontology_ref, link_kind, confidence, status, provenance_id, created_at, updated_at
                 FROM document_ontology_links
                 WHERE document_ref = ?1
                 ORDER BY updated_at DESC",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare document link query"))?;

        let rows = statement
            .query_map([document_ref], map_document_ontology_link_row)
            .map_err(|_| AppError::StatePoisoned("failed to query document links"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read document link rows"))
    }

    fn find_for_ontology_ref(
        &self,
        ontology_ref: &str,
    ) -> Result<Vec<DocumentOntologyLink>, AppError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, document_ref, ontology_ref, link_kind, confidence, status, provenance_id, created_at, updated_at
                 FROM document_ontology_links
                 WHERE ontology_ref = ?1
                 ORDER BY updated_at DESC",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare ontology link query"))?;

        let rows = statement
            .query_map([ontology_ref], map_document_ontology_link_row)
            .map_err(|_| AppError::StatePoisoned("failed to query ontology links"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read ontology link rows"))
    }
}

impl LintRepository for SqliteScreenplayRepository {
    fn upsert_finding(&self, finding: LintFinding) -> Result<LintFinding, AppError> {
        let connection = self.connection()?;
        connection
            .execute(
                "INSERT INTO lint_findings
                    (id, scope, severity, kind, message, evidence_json, status, created_at, resolved_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                    scope = excluded.scope,
                    severity = excluded.severity,
                    kind = excluded.kind,
                    message = excluded.message,
                    evidence_json = excluded.evidence_json,
                    status = excluded.status,
                    created_at = excluded.created_at,
                    resolved_at = excluded.resolved_at",
                params![
                    finding.id.to_string(),
                    lint_scope_to_str(&finding.scope),
                    lint_severity_to_str(&finding.severity),
                    finding.kind,
                    finding.message,
                    finding.evidence_json,
                    lint_status_to_str(&finding.status),
                    finding.created_at.to_rfc3339(),
                    finding.resolved_at.map(|value| value.to_rfc3339()),
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to upsert lint finding"))?;

        Ok(finding)
    }

    fn list_open_findings(&self) -> Result<Vec<LintFinding>, AppError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, scope, severity, kind, message, evidence_json, status, created_at, resolved_at
                 FROM lint_findings
                 WHERE status = 'open'
                 ORDER BY created_at DESC",
            )
            .map_err(|_| AppError::StatePoisoned("failed to prepare lint finding query"))?;

        let rows = statement
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let scope: String = row.get(1)?;
                let severity: String = row.get(2)?;
                let status: String = row.get(6)?;
                let created_at: String = row.get(7)?;
                let resolved_at: Option<String> = row.get(8)?;

                Ok(LintFinding {
                    id: parse_uuid_or_nil(&id),
                    scope: lint_scope_from_str(&scope),
                    severity: lint_severity_from_str(&severity),
                    kind: row.get(3)?,
                    message: row.get(4)?,
                    evidence_json: row.get(5)?,
                    status: lint_status_from_str(&status),
                    created_at: parse_datetime_or_now(&created_at),
                    resolved_at: resolved_at.as_deref().map(parse_datetime_or_now),
                })
            })
            .map_err(|_| AppError::StatePoisoned("failed to query lint findings"))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| AppError::StatePoisoned("failed to read lint finding rows"))
    }
}

impl WorkingMemoryRepository for SqliteScreenplayRepository {
    fn load_working_memory(
        &self,
        project_id: &str,
        session_id: &str,
    ) -> Result<WorkingMemory, AppError> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT conversation_mode, conversation_topic, current_focus_json, constraints_json, open_questions_json, pinned_decisions_json, active_assumptions_json, recent_corrections_json, last_tool_actions_json, updated_at, story_backlog_json
                 FROM assistant_working_memory
                 WHERE project_id = ?1 AND session_id = ?2",
                params![project_id, session_id],
                |row| {
                    let conversation_mode: String = row.get(0)?;
                    let conversation_topic: String = row.get(1)?;
                    let current_focus_json: Option<String> = row.get(2)?;
                    let constraints_json: String = row.get(3)?;
                    let open_questions_json: String = row.get(4)?;
                    let pinned_decisions_json: String = row.get(5)?;
                    let active_assumptions_json: String = row.get(6)?;
                    let recent_corrections_json: String = row.get(7)?;
                    let last_tool_actions_json: String = row.get(8)?;
                    let updated_at: String = row.get(9)?;
                    let story_backlog_json: String = row.get(10)?;

                    Ok(WorkingMemory {
                        project_id: project_id.to_string(),
                        session_id: session_id.to_string(),
                        conversation_mode: match conversation_mode.as_str() {
                            "Refining" => crate::domain::ConversationMode::Refining,
                            "Committing" => crate::domain::ConversationMode::Committing,
                            _ => crate::domain::ConversationMode::Brainstorming,
                        },
                        conversation_topic: match conversation_topic.as_str() {
                            "Setting" => crate::domain::ConversationTopic::Setting,
                            "Character" => crate::domain::ConversationTopic::Character,
                            "Event" => crate::domain::ConversationTopic::Event,
                            "Relationship" => crate::domain::ConversationTopic::Relationship,
                            _ => crate::domain::ConversationTopic::General,
                        },
                        current_focus: current_focus_json
                            .as_deref()
                            .and_then(|value| serde_json::from_str::<FocusItem>(value).ok()),
                        constraints: serde_json::from_str(&constraints_json).unwrap_or_default(),
                        open_questions: serde_json::from_str(&open_questions_json).unwrap_or_default(),
                        pinned_decisions: serde_json::from_str(&pinned_decisions_json).unwrap_or_default(),
                        active_assumptions: serde_json::from_str(&active_assumptions_json)
                            .unwrap_or_default(),
                        recent_corrections: serde_json::from_str(&recent_corrections_json)
                            .unwrap_or_default(),
                        last_tool_actions: serde_json::from_str(&last_tool_actions_json)
                            .unwrap_or_default(),
                        story_backlog: serde_json::from_str(&story_backlog_json).unwrap_or_default(),
                        updated_at: parse_datetime_or_now(&updated_at),
                    })
                },
            )
            .optional()
            .map_err(|_| AppError::StatePoisoned("failed to load assistant working memory"))?
            .map_or_else(
                || {
                    Ok(WorkingMemory {
                        project_id: project_id.to_string(),
                        session_id: session_id.to_string(),
                        ..WorkingMemory::default()
                    })
                },
                Ok,
            )
    }

    fn save_working_memory(&self, memory: WorkingMemory) -> Result<WorkingMemory, AppError> {
        let connection = self.connection()?;
        let current_focus_json = memory
            .current_focus
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|_| AppError::StatePoisoned("failed to serialize working memory focus"))?;
        let constraints_json = serde_json::to_string(&memory.constraints)
            .map_err(|_| AppError::StatePoisoned("failed to serialize constraints"))?;
        let open_questions_json = serde_json::to_string(&memory.open_questions)
            .map_err(|_| AppError::StatePoisoned("failed to serialize open questions"))?;
        let pinned_decisions_json = serde_json::to_string(&memory.pinned_decisions)
            .map_err(|_| AppError::StatePoisoned("failed to serialize pinned decisions"))?;
        let active_assumptions_json = serde_json::to_string(&memory.active_assumptions)
            .map_err(|_| AppError::StatePoisoned("failed to serialize active assumptions"))?;
        let recent_corrections_json = serde_json::to_string(&memory.recent_corrections)
            .map_err(|_| AppError::StatePoisoned("failed to serialize recent corrections"))?;
        let last_tool_actions_json = serde_json::to_string(&memory.last_tool_actions)
            .map_err(|_| AppError::StatePoisoned("failed to serialize tool actions"))?;
        let story_backlog_json = serde_json::to_string(&memory.story_backlog)
            .map_err(|_| AppError::StatePoisoned("failed to serialize story backlog"))?;

        connection
            .execute(
                "INSERT INTO assistant_working_memory
                    (project_id, session_id, conversation_mode, conversation_topic, current_focus_json, constraints_json, open_questions_json, pinned_decisions_json, active_assumptions_json, recent_corrections_json, last_tool_actions_json, updated_at, story_backlog_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT(project_id, session_id) DO UPDATE SET
                    conversation_mode = excluded.conversation_mode,
                    conversation_topic = excluded.conversation_topic,
                    current_focus_json = excluded.current_focus_json,
                    constraints_json = excluded.constraints_json,
                    open_questions_json = excluded.open_questions_json,
                    pinned_decisions_json = excluded.pinned_decisions_json,
                    active_assumptions_json = excluded.active_assumptions_json,
                    recent_corrections_json = excluded.recent_corrections_json,
                    last_tool_actions_json = excluded.last_tool_actions_json,
                    updated_at = excluded.updated_at,
                    story_backlog_json = excluded.story_backlog_json",
                params![
                    memory.project_id,
                    memory.session_id,
                    match memory.conversation_mode {
                        crate::domain::ConversationMode::Brainstorming => "Brainstorming",
                        crate::domain::ConversationMode::Refining => "Refining",
                        crate::domain::ConversationMode::Committing => "Committing",
                    },
                    match memory.conversation_topic {
                        crate::domain::ConversationTopic::Setting => "Setting",
                        crate::domain::ConversationTopic::Character => "Character",
                        crate::domain::ConversationTopic::Event => "Event",
                        crate::domain::ConversationTopic::Relationship => "Relationship",
                        crate::domain::ConversationTopic::General => "General",
                    },
                    current_focus_json,
                    constraints_json,
                    open_questions_json,
                    pinned_decisions_json,
                    active_assumptions_json,
                    recent_corrections_json,
                    last_tool_actions_json,
                    memory.updated_at.to_rfc3339(),
                    story_backlog_json,
                ],
            )
            .map_err(|_| AppError::StatePoisoned("failed to save assistant working memory"))?;

        Ok(memory)
    }
}

fn parse_uuid_or_nil(value: &str) -> Uuid {
    Uuid::parse_str(value).unwrap_or_else(|_| Uuid::nil())
}

fn parse_datetime_or_now(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn map_document_ontology_link_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<DocumentOntologyLink> {
    let id: String = row.get(0)?;
    let status: String = row.get(5)?;
    let provenance_id: Option<String> = row.get(6)?;
    let created_at: String = row.get(7)?;
    let updated_at: String = row.get(8)?;

    Ok(DocumentOntologyLink {
        id: parse_uuid_or_nil(&id),
        document_ref: row.get(1)?,
        ontology_ref: row.get(2)?,
        link_kind: row.get(3)?,
        confidence: row.get(4)?,
        status: link_status_from_str(&status),
        provenance_id: provenance_id.as_deref().map(parse_uuid_or_nil),
        created_at: parse_datetime_or_now(&created_at),
        updated_at: parse_datetime_or_now(&updated_at),
    })
}

fn ontology_entity_kind_to_str(kind: &OntologyEntityKind) -> &'static str {
    match kind {
        OntologyEntityKind::Character => "character",
        OntologyEntityKind::Event => "event",
        OntologyEntityKind::Setting => "setting",
        OntologyEntityKind::WorldContext => "world_context",
    }
}

fn ontology_entity_kind_from_str(value: &str) -> OntologyEntityKind {
    match value {
        "event" => OntologyEntityKind::Event,
        "setting" => OntologyEntityKind::Setting,
        "world_context" => OntologyEntityKind::WorldContext,
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
        OntologyRelationshipKind::SiblingOfCharacter => "sibling_of_character",
    }
}

fn ontology_relationship_kind_from_str(value: &str) -> OntologyRelationshipKind {
    match value {
        "participant_in_event" => OntologyRelationshipKind::ParticipantInEvent,
        "supports_character" => OntologyRelationshipKind::SupportsCharacter,
        "opposes_character" => OntologyRelationshipKind::OpposesCharacter,
        "advises_character" => OntologyRelationshipKind::AdvisesCharacter,
        "sibling_of_character" => OntologyRelationshipKind::SiblingOfCharacter,
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

fn sync_source_kind_to_str(value: &SyncSourceKind) -> &'static str {
    match value {
        SyncSourceKind::NarrativeChat => "narrative_chat",
        SyncSourceKind::ScreenplayExtraction => "screenplay_extraction",
        SyncSourceKind::OntologySuggestion => "ontology_suggestion",
        SyncSourceKind::LintResolution => "lint_resolution",
    }
}

fn sync_source_kind_from_str(value: &str) -> SyncSourceKind {
    match value {
        "screenplay_extraction" => SyncSourceKind::ScreenplayExtraction,
        "ontology_suggestion" => SyncSourceKind::OntologySuggestion,
        "lint_resolution" => SyncSourceKind::LintResolution,
        _ => SyncSourceKind::NarrativeChat,
    }
}

fn sync_run_status_to_str(value: &SyncRunStatus) -> &'static str {
    match value {
        SyncRunStatus::Running => "running",
        SyncRunStatus::Completed => "completed",
        SyncRunStatus::Failed => "failed",
    }
}

fn sync_run_status_from_str(value: &str) -> SyncRunStatus {
    match value {
        "completed" => SyncRunStatus::Completed,
        "failed" => SyncRunStatus::Failed,
        _ => SyncRunStatus::Running,
    }
}

fn sync_target_layer_to_str(value: &SyncTargetLayer) -> &'static str {
    match value {
        SyncTargetLayer::Document => "document",
        SyncTargetLayer::Ontology => "ontology",
        SyncTargetLayer::Link => "link",
    }
}

fn sync_target_layer_from_str(value: &str) -> SyncTargetLayer {
    match value {
        "document" => SyncTargetLayer::Document,
        "link" => SyncTargetLayer::Link,
        _ => SyncTargetLayer::Ontology,
    }
}

fn sync_target_kind_to_str(value: &SyncTargetKind) -> &'static str {
    match value {
        SyncTargetKind::Character => "character",
        SyncTargetKind::Event => "event",
        SyncTargetKind::Relationship => "relationship",
        SyncTargetKind::Setting => "setting",
        SyncTargetKind::WorldContext => "world_context",
        SyncTargetKind::ScreenplayPatch => "screenplay_patch",
        SyncTargetKind::DocumentMetadata => "document_metadata",
        SyncTargetKind::Link => "link",
        SyncTargetKind::LintFinding => "lint_finding",
    }
}

fn sync_target_kind_from_str(value: &str) -> SyncTargetKind {
    match value {
        "event" => SyncTargetKind::Event,
        "relationship" => SyncTargetKind::Relationship,
        "setting" => SyncTargetKind::Setting,
        "world_context" => SyncTargetKind::WorldContext,
        "screenplay_patch" => SyncTargetKind::ScreenplayPatch,
        "document_metadata" => SyncTargetKind::DocumentMetadata,
        "link" => SyncTargetKind::Link,
        "lint_finding" => SyncTargetKind::LintFinding,
        _ => SyncTargetKind::Character,
    }
}

fn sync_action_kind_to_str(value: &SyncActionKind) -> &'static str {
    match value {
        SyncActionKind::Create => "create",
        SyncActionKind::Update => "update",
        SyncActionKind::Merge => "merge",
        SyncActionKind::Relink => "relink",
        SyncActionKind::Patch => "patch",
        SyncActionKind::Delete => "delete",
    }
}

fn sync_action_kind_from_str(value: &str) -> SyncActionKind {
    match value {
        "update" => SyncActionKind::Update,
        "merge" => SyncActionKind::Merge,
        "relink" => SyncActionKind::Relink,
        "patch" => SyncActionKind::Patch,
        "delete" => SyncActionKind::Delete,
        _ => SyncActionKind::Create,
    }
}

fn candidate_status_to_str(value: &CandidateStatus) -> &'static str {
    match value {
        CandidateStatus::Draft => "draft",
        CandidateStatus::Ready => "ready",
        CandidateStatus::Applied => "applied",
        CandidateStatus::Rejected => "rejected",
        CandidateStatus::Superseded => "superseded",
        CandidateStatus::Expired => "expired",
        CandidateStatus::Conflicted => "conflicted",
    }
}

fn candidate_status_from_str(value: &str) -> CandidateStatus {
    match value {
        "ready" => CandidateStatus::Ready,
        "applied" => CandidateStatus::Applied,
        "rejected" => CandidateStatus::Rejected,
        "superseded" => CandidateStatus::Superseded,
        "expired" => CandidateStatus::Expired,
        "conflicted" => CandidateStatus::Conflicted,
        _ => CandidateStatus::Draft,
    }
}

fn conflict_kind_to_str(value: &crate::domain::ConflictKind) -> &'static str {
    match value {
        crate::domain::ConflictKind::AmbiguousMatch => "ambiguous_match",
        crate::domain::ConflictKind::DuplicateEntity => "duplicate_entity",
        crate::domain::ConflictKind::VersionMismatch => "version_mismatch",
        crate::domain::ConflictKind::ContradictoryTimeline => "contradictory_timeline",
        crate::domain::ConflictKind::UnsupportedPatch => "unsupported_patch",
    }
}

fn conflict_kind_from_str(value: &str) -> crate::domain::ConflictKind {
    match value {
        "duplicate_entity" => crate::domain::ConflictKind::DuplicateEntity,
        "version_mismatch" => crate::domain::ConflictKind::VersionMismatch,
        "contradictory_timeline" => crate::domain::ConflictKind::ContradictoryTimeline,
        "unsupported_patch" => crate::domain::ConflictKind::UnsupportedPatch,
        _ => crate::domain::ConflictKind::AmbiguousMatch,
    }
}

fn link_status_to_str(value: &crate::domain::LinkStatus) -> &'static str {
    match value {
        crate::domain::LinkStatus::Linked => "linked",
        crate::domain::LinkStatus::Suggested => "suggested",
        crate::domain::LinkStatus::Conflicted => "conflicted",
        crate::domain::LinkStatus::Orphaned => "orphaned",
    }
}

fn link_status_from_str(value: &str) -> crate::domain::LinkStatus {
    match value {
        "suggested" => crate::domain::LinkStatus::Suggested,
        "conflicted" => crate::domain::LinkStatus::Conflicted,
        "orphaned" => crate::domain::LinkStatus::Orphaned,
        _ => crate::domain::LinkStatus::Linked,
    }
}

fn lint_scope_to_str(value: &LintScope) -> &'static str {
    match value {
        LintScope::Document => "document",
        LintScope::Ontology => "ontology",
        LintScope::Alignment => "alignment",
    }
}

fn lint_scope_from_str(value: &str) -> LintScope {
    match value {
        "document" => LintScope::Document,
        "alignment" => LintScope::Alignment,
        _ => LintScope::Ontology,
    }
}

fn lint_severity_to_str(value: &LintSeverity) -> &'static str {
    match value {
        LintSeverity::Info => "info",
        LintSeverity::Warning => "warning",
        LintSeverity::Error => "error",
    }
}

fn lint_severity_from_str(value: &str) -> LintSeverity {
    match value {
        "warning" => LintSeverity::Warning,
        "error" => LintSeverity::Error,
        _ => LintSeverity::Info,
    }
}

fn lint_status_to_str(value: &LintStatus) -> &'static str {
    match value {
        LintStatus::Open => "open",
        LintStatus::Resolved => "resolved",
        LintStatus::Dismissed => "dismissed",
    }
}

fn lint_status_from_str(value: &str) -> LintStatus {
    match value {
        "resolved" => LintStatus::Resolved,
        "dismissed" => LintStatus::Dismissed,
        _ => LintStatus::Open,
    }
}
