CREATE TABLE analysis_runs (
 id TEXT PRIMARY KEY NOT NULL,
 session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
 input_revision INTEGER NOT NULL CHECK(input_revision > 0),
 policy_revision INTEGER NOT NULL CHECK(policy_revision > 0),
 input_hash TEXT NOT NULL CHECK(length(input_hash) = 64),
 request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
 model TEXT NOT NULL,
 attempt_id TEXT,
 purpose TEXT NOT NULL CHECK(purpose IN ('generation','grading')),
 status TEXT NOT NULL CHECK(status IN ('running','published','cancelled','failed')),
 created_at TEXT NOT NULL,
 finished_at TEXT,
 CHECK((purpose='generation' AND attempt_id IS NULL) OR (purpose='grading' AND attempt_id IS NOT NULL))
);
CREATE INDEX analysis_runs_session ON analysis_runs(session_id,created_at,id);
CREATE TABLE learning_documents (
 id TEXT PRIMARY KEY NOT NULL,
 run_id TEXT NOT NULL UNIQUE REFERENCES analysis_runs(id) ON DELETE CASCADE,
 session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
 input_revision INTEGER NOT NULL,
 input_hash TEXT NOT NULL,
 model TEXT NOT NULL,
 contract_version TEXT NOT NULL,
 document_json TEXT NOT NULL,
 created_at TEXT NOT NULL,
 UNIQUE(id,session_id)
);
CREATE INDEX learning_document_session ON learning_documents(session_id,created_at DESC,id DESC);
CREATE TABLE selfcheck_questions (
 id TEXT PRIMARY KEY NOT NULL,
 document_id TEXT NOT NULL REFERENCES learning_documents(id) ON DELETE CASCADE,
 concept_key TEXT NOT NULL,
 variant TEXT NOT NULL CHECK(variant IN ('initial','delayed')),
 position INTEGER NOT NULL CHECK(position >= 0),
 question_json TEXT NOT NULL,
 UNIQUE(document_id,concept_key,variant)
);
CREATE TABLE learning_exposures (
 document_id TEXT NOT NULL REFERENCES learning_documents(id) ON DELETE CASCADE,
 concept_key TEXT NOT NULL,
 exposed_at TEXT NOT NULL,
 PRIMARY KEY(document_id,concept_key)
);
CREATE TABLE selfcheck_reveals (
 question_id TEXT PRIMARY KEY NOT NULL REFERENCES selfcheck_questions(id) ON DELETE CASCADE,
 revealed_at TEXT NOT NULL
);
CREATE TABLE selfcheck_attempts (
 id TEXT PRIMARY KEY NOT NULL,
 question_id TEXT NOT NULL REFERENCES selfcheck_questions(id) ON DELETE CASCADE,
 answer TEXT NOT NULL,
 assistance TEXT NOT NULL CHECK(assistance IN ('independent','assisted','unknown')),
 solution_seen INTEGER NOT NULL CHECK(solution_seen IN (0,1)),
 grade TEXT NOT NULL CHECK(grade IN ('correct','incorrect','uncertain','pending')),
 feedback_json TEXT NOT NULL,
 submitted_at TEXT NOT NULL,
 request_hash TEXT NOT NULL CHECK(length(request_hash) = 64),
 grading_run_id TEXT REFERENCES analysis_runs(id) ON DELETE SET NULL
);
CREATE INDEX selfcheck_attempt_order ON selfcheck_attempts(question_id,submitted_at DESC,id DESC);
