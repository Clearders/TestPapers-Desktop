PRAGMA foreign_keys = ON;

CREATE TABLE workspace_meta (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    workspace_id TEXT NOT NULL,
    local_principal_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
    created_at INTEGER NOT NULL
);

CREATE TABLE questions (
    id TEXT PRIMARY KEY
        CHECK (length(id) = 36 AND id = lower(id)
            AND substr(id, 9, 1) = '-' AND substr(id, 14, 1) = '-'
            AND substr(id, 19, 1) = '-' AND substr(id, 24, 1) = '-'),
    owner_id TEXT NOT NULL CHECK (length(owner_id) = 36 AND owner_id = lower(owner_id)),
    replication_scope TEXT NOT NULL
        CHECK (replication_scope IN ('local_private', 'cloud_synced', 'collaborative_shared')),
    schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1),
    version INTEGER NOT NULL CHECK (version >= 1),
    content_hash TEXT NOT NULL
        CHECK (length(content_hash) = 64 AND content_hash = lower(content_hash)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    deleted_by_id TEXT,
    type TEXT NOT NULL
        CHECK (type IN ('single_choice', 'multiple_choice', 'true_false', 'blank', 'short_answer', 'essay')),
    subjects_json TEXT NOT NULL CHECK (json_valid(subjects_json) AND json_array_length(subjects_json) > 0),
    difficulty TEXT NOT NULL CHECK (difficulty IN ('easy', 'medium', 'hard')),
    tags_json TEXT NOT NULL CHECK (json_valid(tags_json)),
    text TEXT NOT NULL,
    options_json TEXT CHECK (options_json IS NULL OR json_valid(options_json)),
    answer_json TEXT NOT NULL CHECK (json_valid(answer_json)),
    has_latex INTEGER NOT NULL DEFAULT 0 CHECK (has_latex IN (0, 1)),
    source TEXT,
    essay_blank_space_json TEXT CHECK (essay_blank_space_json IS NULL OR json_valid(essay_blank_space_json)),
    score_weight TEXT NOT NULL DEFAULT '1',
    CHECK ((deleted_at IS NULL AND deleted_by_id IS NULL)
        OR (deleted_at IS NOT NULL AND deleted_by_id IS NOT NULL))
);

CREATE INDEX questions_live_updated_idx ON questions(deleted_at, updated_at DESC, id);
CREATE INDEX questions_type_idx ON questions(type, deleted_at);
CREATE INDEX questions_difficulty_idx ON questions(difficulty, deleted_at);

CREATE TABLE question_subjects (
    question_id TEXT NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    value TEXT NOT NULL,
    PRIMARY KEY (question_id, value),
    UNIQUE (question_id, position)
);
CREATE INDEX question_subjects_value_idx ON question_subjects(value, question_id);

CREATE TABLE question_tags (
    question_id TEXT NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK (position >= 0),
    value TEXT NOT NULL,
    PRIMARY KEY (question_id, value),
    UNIQUE (question_id, position)
);
CREATE INDEX question_tags_value_idx ON question_tags(value, question_id);

CREATE VIRTUAL TABLE questions_fts USING fts5(
    question_id UNINDEXED,
    text,
    source,
    subjects,
    tags,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TRIGGER questions_after_insert AFTER INSERT ON questions BEGIN
    INSERT INTO question_subjects(question_id, position, value)
        SELECT new.id, CAST(key AS INTEGER), value FROM json_each(new.subjects_json);
    INSERT INTO question_tags(question_id, position, value)
        SELECT new.id, CAST(key AS INTEGER), value FROM json_each(new.tags_json);
    INSERT INTO questions_fts(question_id, text, source, subjects, tags)
        VALUES (
            new.id,
            new.text,
            coalesce(new.source, ''),
            coalesce((SELECT group_concat(value, ' ') FROM json_each(new.subjects_json)), ''),
            coalesce((SELECT group_concat(value, ' ') FROM json_each(new.tags_json)), '')
        );
END;

CREATE TRIGGER questions_after_content_update
AFTER UPDATE OF subjects_json, tags_json, text, source ON questions BEGIN
    DELETE FROM question_subjects WHERE question_id = old.id;
    INSERT INTO question_subjects(question_id, position, value)
        SELECT new.id, CAST(key AS INTEGER), value FROM json_each(new.subjects_json);
    DELETE FROM question_tags WHERE question_id = old.id;
    INSERT INTO question_tags(question_id, position, value)
        SELECT new.id, CAST(key AS INTEGER), value FROM json_each(new.tags_json);
    DELETE FROM questions_fts WHERE question_id = old.id;
    INSERT INTO questions_fts(question_id, text, source, subjects, tags)
        VALUES (
            new.id,
            new.text,
            coalesce(new.source, ''),
            coalesce((SELECT group_concat(value, ' ') FROM json_each(new.subjects_json)), ''),
            coalesce((SELECT group_concat(value, ' ') FROM json_each(new.tags_json)), '')
        );
END;

CREATE TRIGGER questions_after_delete AFTER DELETE ON questions BEGIN
    DELETE FROM questions_fts WHERE question_id = old.id;
END;

CREATE TABLE papers (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND id = lower(id)),
    owner_id TEXT NOT NULL CHECK (length(owner_id) = 36 AND owner_id = lower(owner_id)),
    replication_scope TEXT NOT NULL
        CHECK (replication_scope IN ('local_private', 'cloud_synced', 'collaborative_shared')),
    schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1),
    version INTEGER NOT NULL CHECK (version >= 1),
    content_hash TEXT NOT NULL CHECK (length(content_hash) = 64 AND content_hash = lower(content_hash)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    deleted_by_id TEXT,
    title TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 255),
    subject TEXT NOT NULL CHECK (length(subject) BETWEEN 1 AND 255),
    duration_minutes INTEGER NOT NULL CHECK (duration_minutes BETWEEN 1 AND 2147483647),
    total_marks TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'published', 'archived')),
    items_json TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(items_json) AND json_type(items_json) = 'array'),
    CHECK ((deleted_at IS NULL AND deleted_by_id IS NULL)
        OR (deleted_at IS NOT NULL AND deleted_by_id IS NOT NULL))
);
CREATE INDEX papers_live_updated_idx ON papers(deleted_at, updated_at DESC, id);

CREATE TABLE paper_items (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND id = lower(id)),
    paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
    question_id TEXT REFERENCES questions(id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
    item_order INTEGER NOT NULL CHECK (item_order BETWEEN 0 AND 2147483647),
    marks TEXT,
    question_snapshot_json TEXT NOT NULL
        CHECK (json_valid(question_snapshot_json) AND json_type(question_snapshot_json) = 'object'),
    UNIQUE (paper_id, item_order)
);
CREATE INDEX paper_items_paper_idx ON paper_items(paper_id, item_order, id);
CREATE INDEX paper_items_question_idx ON paper_items(question_id) WHERE question_id IS NOT NULL;

CREATE TABLE drafts (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND id = lower(id)),
    owner_id TEXT NOT NULL CHECK (length(owner_id) = 36 AND owner_id = lower(owner_id)),
    replication_scope TEXT NOT NULL
        CHECK (replication_scope IN ('local_private', 'cloud_synced', 'collaborative_shared')),
    schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1),
    version INTEGER NOT NULL CHECK (version >= 1),
    content_hash TEXT NOT NULL CHECK (length(content_hash) = 64 AND content_hash = lower(content_hash)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    deleted_by_id TEXT,
    name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    paper_id TEXT REFERENCES papers(id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
    state_json TEXT NOT NULL CHECK (json_valid(state_json)),
    review_status TEXT NOT NULL DEFAULT 'draft'
        CHECK (review_status IN ('draft', 'in_review', 'approved', 'changes_requested')),
    updated_by_id TEXT,
    CHECK ((deleted_at IS NULL AND deleted_by_id IS NULL)
        OR (deleted_at IS NOT NULL AND deleted_by_id IS NOT NULL))
);
CREATE INDEX drafts_live_updated_idx ON drafts(deleted_at, updated_at DESC, id);
CREATE INDEX drafts_paper_idx ON drafts(paper_id) WHERE paper_id IS NOT NULL;

CREATE TABLE comments (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND id = lower(id)),
    owner_id TEXT NOT NULL CHECK (length(owner_id) = 36 AND owner_id = lower(owner_id)),
    replication_scope TEXT NOT NULL
        CHECK (replication_scope IN ('cloud_synced', 'collaborative_shared')),
    schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1),
    version INTEGER NOT NULL CHECK (version >= 1),
    content_hash TEXT NOT NULL CHECK (length(content_hash) = 64 AND content_hash = lower(content_hash)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    deleted_by_id TEXT,
    target_type TEXT NOT NULL CHECK (target_type IN ('question', 'paper', 'draft')),
    target_id TEXT NOT NULL CHECK (length(target_id) = 36 AND target_id = lower(target_id)),
    parent_comment_id TEXT REFERENCES comments(id) ON DELETE RESTRICT DEFERRABLE INITIALLY DEFERRED,
    anchor_json TEXT CHECK (anchor_json IS NULL OR json_valid(anchor_json)),
    body TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved')),
    resolved_at INTEGER,
    resolved_by_id TEXT,
    CHECK ((deleted_at IS NULL AND deleted_by_id IS NULL)
        OR (deleted_at IS NOT NULL AND deleted_by_id IS NOT NULL)),
    CHECK ((status = 'open' AND resolved_at IS NULL AND resolved_by_id IS NULL)
        OR (status = 'resolved' AND resolved_at IS NOT NULL AND resolved_by_id IS NOT NULL))
);
CREATE INDEX comments_target_idx ON comments(target_type, target_id, deleted_at, created_at);
CREATE INDEX comments_parent_idx ON comments(parent_comment_id) WHERE parent_comment_id IS NOT NULL;

CREATE TABLE favorites (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND id = lower(id)),
    owner_id TEXT NOT NULL CHECK (length(owner_id) = 36 AND owner_id = lower(owner_id)),
    replication_scope TEXT NOT NULL CHECK (replication_scope = 'cloud_synced'),
    schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1),
    version INTEGER NOT NULL CHECK (version >= 1),
    content_hash TEXT NOT NULL CHECK (length(content_hash) = 64 AND content_hash = lower(content_hash)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    deleted_by_id TEXT,
    target_type TEXT NOT NULL CHECK (target_type IN ('question', 'paper')),
    target_id TEXT NOT NULL CHECK (length(target_id) = 36 AND target_id = lower(target_id)),
    UNIQUE (owner_id, target_type, target_id),
    CHECK ((deleted_at IS NULL AND deleted_by_id IS NULL)
        OR (deleted_at IS NOT NULL AND deleted_by_id IS NOT NULL))
);
CREATE INDEX favorites_target_idx ON favorites(target_type, target_id, deleted_at);

CREATE TABLE settings (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND id = lower(id)),
    owner_id TEXT NOT NULL CHECK (length(owner_id) = 36 AND owner_id = lower(owner_id)),
    replication_scope TEXT NOT NULL CHECK (replication_scope IN ('local_private', 'cloud_synced')),
    schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1),
    version INTEGER NOT NULL CHECK (version >= 1),
    content_hash TEXT NOT NULL CHECK (length(content_hash) = 64 AND content_hash = lower(content_hash)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    deleted_by_id TEXT,
    setting_scope TEXT NOT NULL CHECK (setting_scope IN ('device', 'account')),
    key TEXT NOT NULL CHECK (length(key) BETWEEN 1 AND 160 AND key = lower(key)),
    value_json TEXT NOT NULL CHECK (json_valid(value_json)),
    UNIQUE (owner_id, setting_scope, key),
    CHECK ((replication_scope = 'local_private' AND setting_scope = 'device')
        OR (replication_scope = 'cloud_synced' AND setting_scope = 'account')),
    CHECK ((deleted_at IS NULL AND deleted_by_id IS NULL)
        OR (deleted_at IS NOT NULL AND deleted_by_id IS NOT NULL))
);
CREATE INDEX settings_owner_scope_idx ON settings(owner_id, setting_scope, key);

CREATE TABLE entity_history (
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version >= 1),
    content_hash TEXT NOT NULL CHECK (length(content_hash) = 64),
    action TEXT NOT NULL CHECK (action IN ('create', 'update', 'delete', 'restore', 'revert')),
    snapshot_json TEXT NOT NULL CHECK (json_valid(snapshot_json)),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (entity_type, entity_id, version)
);
CREATE INDEX entity_history_entity_idx
    ON entity_history(entity_type, entity_id, version DESC);

CREATE TABLE pending_mutations (
    operation_id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    base_version INTEGER,
    base_content_hash TEXT,
    mutation_kind TEXT NOT NULL,
    candidate_json TEXT NOT NULL CHECK (json_valid(candidate_json)),
    created_at INTEGER NOT NULL,
    CHECK ((base_version IS NULL AND base_content_hash IS NULL)
        OR (base_version IS NOT NULL AND base_version >= 1
            AND base_content_hash IS NOT NULL AND length(base_content_hash) = 64))
);
CREATE INDEX pending_mutations_created_idx ON pending_mutations(created_at, operation_id);

CREATE TABLE conflict_candidates (
    candidate_id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    requested_base_version INTEGER NOT NULL CHECK (requested_base_version >= 1),
    requested_base_hash TEXT NOT NULL CHECK (length(requested_base_hash) = 64),
    current_version INTEGER NOT NULL CHECK (current_version >= 1),
    current_hash TEXT NOT NULL CHECK (length(current_hash) = 64),
    requested_action TEXT NOT NULL,
    candidate_json TEXT NOT NULL CHECK (json_valid(candidate_json)),
    created_at INTEGER NOT NULL
);
CREATE INDEX conflict_candidates_entity_idx
    ON conflict_candidates(entity_type, entity_id, created_at DESC);

CREATE TABLE attachments (
    id TEXT PRIMARY KEY CHECK (length(id) = 36 AND id = lower(id)),
    owner_id TEXT NOT NULL CHECK (length(owner_id) = 36 AND owner_id = lower(owner_id)),
    replication_scope TEXT NOT NULL
        CHECK (replication_scope IN ('local_private', 'cloud_synced', 'collaborative_shared')),
    schema_version INTEGER NOT NULL DEFAULT 1 CHECK (schema_version >= 1),
    version INTEGER NOT NULL CHECK (version >= 1),
    content_hash TEXT NOT NULL CHECK (length(content_hash) = 64),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    deleted_by_id TEXT,
    target_type TEXT NOT NULL CHECK (target_type IN ('question', 'paper', 'draft', 'comment')),
    target_id TEXT NOT NULL,
    file_name TEXT NOT NULL CHECK (length(file_name) BETWEEN 1 AND 255),
    media_type TEXT NOT NULL CHECK (length(media_type) BETWEEN 1 AND 255),
    byte_size INTEGER NOT NULL CHECK (byte_size BETWEEN 0 AND 9223372036854775807),
    blob_hash TEXT NOT NULL CHECK (length(blob_hash) = 64 AND blob_hash = lower(blob_hash)),
    caption TEXT,
    position INTEGER NOT NULL DEFAULT 0 CHECK (position BETWEEN 0 AND 2147483647),
    uploaded_by_id TEXT,
    relative_path TEXT NOT NULL,
    UNIQUE (target_type, target_id, position),
    CHECK ((deleted_at IS NULL AND deleted_by_id IS NULL)
        OR (deleted_at IS NOT NULL AND deleted_by_id IS NOT NULL))
);
CREATE INDEX attachments_target_idx ON attachments(target_type, target_id, deleted_at, position);
CREATE INDEX attachments_blob_idx ON attachments(blob_hash, deleted_at);

CREATE TABLE question_attachment_links (
    attachment_id TEXT PRIMARY KEY REFERENCES attachments(id) ON DELETE CASCADE,
    question_id TEXT NOT NULL REFERENCES questions(id) ON DELETE RESTRICT
);
CREATE INDEX question_attachment_links_question_idx
    ON question_attachment_links(question_id, attachment_id);

CREATE TABLE paper_attachment_links (
    attachment_id TEXT PRIMARY KEY REFERENCES attachments(id) ON DELETE CASCADE,
    paper_id TEXT NOT NULL REFERENCES papers(id) ON DELETE RESTRICT
);
CREATE INDEX paper_attachment_links_paper_idx ON paper_attachment_links(paper_id, attachment_id);

CREATE TABLE draft_attachment_links (
    attachment_id TEXT PRIMARY KEY REFERENCES attachments(id) ON DELETE CASCADE,
    draft_id TEXT NOT NULL REFERENCES drafts(id) ON DELETE RESTRICT
);
CREATE INDEX draft_attachment_links_draft_idx ON draft_attachment_links(draft_id, attachment_id);

CREATE TABLE comment_attachment_links (
    attachment_id TEXT PRIMARY KEY REFERENCES attachments(id) ON DELETE CASCADE,
    comment_id TEXT NOT NULL REFERENCES comments(id) ON DELETE RESTRICT
);
CREATE INDEX comment_attachment_links_comment_idx ON comment_attachment_links(comment_id, attachment_id);

CREATE TRIGGER question_attachment_link_before_insert
BEFORE INSERT ON question_attachment_links BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1
        FROM attachments a
        JOIN questions q ON q.id = new.question_id
        WHERE a.id = new.attachment_id
          AND a.target_type = 'question'
          AND a.target_id = q.id
          AND a.owner_id = q.owner_id
          AND a.replication_scope = q.replication_scope
          AND a.deleted_at IS NULL
          AND q.deleted_at IS NULL
          AND NOT EXISTS (SELECT 1 FROM paper_attachment_links WHERE attachment_id = a.id)
          AND NOT EXISTS (SELECT 1 FROM draft_attachment_links WHERE attachment_id = a.id)
          AND NOT EXISTS (SELECT 1 FROM comment_attachment_links WHERE attachment_id = a.id)
    ) THEN RAISE(ABORT, 'attachment target must be a live question with the same owner and scope') END;
END;

CREATE TRIGGER paper_attachment_link_before_insert
BEFORE INSERT ON paper_attachment_links BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM attachments a JOIN papers p ON p.id = new.paper_id
        WHERE a.id = new.attachment_id AND a.target_type = 'paper' AND a.target_id = p.id
          AND a.owner_id = p.owner_id AND a.replication_scope = p.replication_scope
          AND a.deleted_at IS NULL AND p.deleted_at IS NULL
          AND NOT EXISTS (SELECT 1 FROM question_attachment_links WHERE attachment_id = a.id)
          AND NOT EXISTS (SELECT 1 FROM draft_attachment_links WHERE attachment_id = a.id)
          AND NOT EXISTS (SELECT 1 FROM comment_attachment_links WHERE attachment_id = a.id)
    ) THEN RAISE(ABORT, 'attachment target must be a live paper with the same owner and scope') END;
END;

CREATE TRIGGER draft_attachment_link_before_insert
BEFORE INSERT ON draft_attachment_links BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM attachments a JOIN drafts d ON d.id = new.draft_id
        WHERE a.id = new.attachment_id AND a.target_type = 'draft' AND a.target_id = d.id
          AND a.owner_id = d.owner_id AND a.replication_scope = d.replication_scope
          AND a.deleted_at IS NULL AND d.deleted_at IS NULL
          AND NOT EXISTS (SELECT 1 FROM question_attachment_links WHERE attachment_id = a.id)
          AND NOT EXISTS (SELECT 1 FROM paper_attachment_links WHERE attachment_id = a.id)
          AND NOT EXISTS (SELECT 1 FROM comment_attachment_links WHERE attachment_id = a.id)
    ) THEN RAISE(ABORT, 'attachment target must be a live draft with the same owner and scope') END;
END;

CREATE TRIGGER comment_attachment_link_before_insert
BEFORE INSERT ON comment_attachment_links BEGIN
    SELECT CASE WHEN NOT EXISTS (
        SELECT 1 FROM attachments a JOIN comments c ON c.id = new.comment_id
        WHERE a.id = new.attachment_id AND a.target_type = 'comment' AND a.target_id = c.id
          AND a.owner_id = c.owner_id AND a.replication_scope = c.replication_scope
          AND a.deleted_at IS NULL AND c.deleted_at IS NULL
          AND NOT EXISTS (SELECT 1 FROM question_attachment_links WHERE attachment_id = a.id)
          AND NOT EXISTS (SELECT 1 FROM paper_attachment_links WHERE attachment_id = a.id)
          AND NOT EXISTS (SELECT 1 FROM draft_attachment_links WHERE attachment_id = a.id)
    ) THEN RAISE(ABORT, 'attachment target must be a live comment with the same owner and scope') END;
END;

PRAGMA user_version = 1;
