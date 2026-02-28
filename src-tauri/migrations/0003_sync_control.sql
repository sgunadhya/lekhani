PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS sync_runs (
    id TEXT PRIMARY KEY,
    source_kind TEXT NOT NULL,
    source_ref TEXT,
    document_version INTEGER,
    ontology_version INTEGER,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE TABLE IF NOT EXISTS sync_candidates (
    id TEXT PRIMARY KEY,
    sync_run_id TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    source_ref TEXT,
    target_layer TEXT NOT NULL,
    target_kind TEXT NOT NULL,
    action_kind TEXT NOT NULL,
    status TEXT NOT NULL,
    confidence REAL,
    title TEXT NOT NULL,
    summary TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    evidence_json TEXT,
    created_at TEXT NOT NULL,
    resolved_at TEXT,
    FOREIGN KEY (sync_run_id) REFERENCES sync_runs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS provenance_records (
    id TEXT PRIMARY KEY,
    sync_run_id TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    source_ref TEXT,
    derived_kind TEXT NOT NULL,
    derived_ref TEXT NOT NULL,
    confidence REAL,
    notes TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (sync_run_id) REFERENCES sync_runs(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS document_ontology_links (
    id TEXT PRIMARY KEY,
    document_ref TEXT NOT NULL,
    ontology_ref TEXT NOT NULL,
    link_kind TEXT NOT NULL,
    confidence REAL,
    status TEXT NOT NULL,
    provenance_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (provenance_id) REFERENCES provenance_records(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS sync_conflicts (
    id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL,
    conflict_kind TEXT NOT NULL,
    summary TEXT NOT NULL,
    details_json TEXT,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    resolved_at TEXT,
    FOREIGN KEY (candidate_id) REFERENCES sync_candidates(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS lint_findings (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    severity TEXT NOT NULL,
    kind TEXT NOT NULL,
    message TEXT NOT NULL,
    evidence_json TEXT,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    resolved_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_sync_candidates_sync_run_id
    ON sync_candidates(sync_run_id);

CREATE INDEX IF NOT EXISTS idx_sync_candidates_status
    ON sync_candidates(status);

CREATE INDEX IF NOT EXISTS idx_provenance_records_sync_run_id
    ON provenance_records(sync_run_id);

CREATE INDEX IF NOT EXISTS idx_document_ontology_links_document_ref
    ON document_ontology_links(document_ref);

CREATE INDEX IF NOT EXISTS idx_document_ontology_links_ontology_ref
    ON document_ontology_links(ontology_ref);

CREATE INDEX IF NOT EXISTS idx_sync_conflicts_candidate_id
    ON sync_conflicts(candidate_id);

CREATE INDEX IF NOT EXISTS idx_lint_findings_status
    ON lint_findings(status);
