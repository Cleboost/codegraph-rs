//! SQLite-backed knowledge graph storage (rusqlite, bundled, FTS5).
//!
//! Fresh schema — no backwards compatibility with archive TS DB.

pub const SCHEMA_SQL: &str = include_str!("schema.sql");

// TODO: Connection wrapper, prepared statement cache, migrations, FTS5 index.
