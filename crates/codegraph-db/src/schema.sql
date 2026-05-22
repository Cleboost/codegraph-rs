-- codegraph schema v1 (Rust rewrite, fresh)
-- PRAGMAs set by Db::open before migrations.

CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS files (
    id         INTEGER PRIMARY KEY,
    path       TEXT NOT NULL UNIQUE,
    language   TEXT NOT NULL,
    sha256     TEXT NOT NULL,
    size       INTEGER NOT NULL,
    mtime      INTEGER NOT NULL,
    indexed_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_files_lang ON files(language);

CREATE TABLE IF NOT EXISTS nodes (
    id              INTEGER PRIMARY KEY,
    kind            TEXT NOT NULL,
    name            TEXT NOT NULL,
    qualified_name  TEXT,
    file_id         INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    start_line      INTEGER NOT NULL,
    end_line        INTEGER NOT NULL,
    signature       TEXT,
    docstring       TEXT,
    language        TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_nodes_name  ON nodes(name);
CREATE INDEX IF NOT EXISTS idx_nodes_qname ON nodes(qualified_name);
CREATE INDEX IF NOT EXISTS idx_nodes_file  ON nodes(file_id);
CREATE INDEX IF NOT EXISTS idx_nodes_kind  ON nodes(kind);

CREATE TABLE IF NOT EXISTS edges (
    id      INTEGER PRIMARY KEY,
    from_id INTEGER NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    to_id   INTEGER NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    kind    TEXT NOT NULL,
    file_id INTEGER REFERENCES files(id) ON DELETE CASCADE,
    line    INTEGER,
    source  TEXT
);

CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_id, kind);
CREATE INDEX IF NOT EXISTS idx_edges_to   ON edges(to_id, kind);
CREATE INDEX IF NOT EXISTS idx_edges_src  ON edges(source);

CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
    name, qualified_name, signature, docstring,
    content='nodes', content_rowid='id', tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS nodes_ai AFTER INSERT ON nodes BEGIN
    INSERT INTO nodes_fts(rowid, name, qualified_name, signature, docstring)
    VALUES (new.id, new.name, COALESCE(new.qualified_name,''), COALESCE(new.signature,''), COALESCE(new.docstring,''));
END;

CREATE TRIGGER IF NOT EXISTS nodes_ad AFTER DELETE ON nodes BEGIN
    INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, signature, docstring)
    VALUES ('delete', old.id, old.name, COALESCE(old.qualified_name,''), COALESCE(old.signature,''), COALESCE(old.docstring,''));
END;

CREATE TRIGGER IF NOT EXISTS nodes_au AFTER UPDATE ON nodes BEGIN
    INSERT INTO nodes_fts(nodes_fts, rowid, name, qualified_name, signature, docstring)
    VALUES ('delete', old.id, old.name, COALESCE(old.qualified_name,''), COALESCE(old.signature,''), COALESCE(old.docstring,''));
    INSERT INTO nodes_fts(rowid, name, qualified_name, signature, docstring)
    VALUES (new.id, new.name, COALESCE(new.qualified_name,''), COALESCE(new.signature,''), COALESCE(new.docstring,''));
END;
