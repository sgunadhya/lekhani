CREATE TABLE IF NOT EXISTS assistant_working_memory (
  project_id TEXT NOT NULL,
  session_id TEXT NOT NULL,
  current_focus_json TEXT,
  open_questions_json TEXT NOT NULL DEFAULT '[]',
  pinned_decisions_json TEXT NOT NULL DEFAULT '[]',
  active_assumptions_json TEXT NOT NULL DEFAULT '[]',
  recent_corrections_json TEXT NOT NULL DEFAULT '[]',
  last_tool_actions_json TEXT NOT NULL DEFAULT '[]',
  updated_at TEXT NOT NULL,
  PRIMARY KEY (project_id, session_id)
);

CREATE INDEX IF NOT EXISTS idx_assistant_working_memory_updated_at
  ON assistant_working_memory(updated_at DESC);
