# Spec 03 — DB layer

**État**: pending

## Objectif

Couche SQLite minimale et rapide. Crate `codegraph-db`.

## Stack

- `rusqlite` features `bundled` + `backup`. Bundled = SQLite statique → zero dep système.
- Pas de pool — SQLite WAL gère écriture mono, lectures parallèles depuis autres connexions. Un `Connection` par thread suffisant; pour les batches d'extraction, une connexion writer + N readers via `parking_lot::Mutex`.
- Schema versionné via table `meta(key, value)`; clé `schema_version`.

## API publique

```rust
pub struct Db { conn: Mutex<Connection> }

impl Db {
    pub fn open(path: &Utf8Path) -> Result<Self>;          // create + migrate
    pub fn open_read_only(path: &Utf8Path) -> Result<Self>;
    pub fn close(self) -> Result<()>;
    pub fn schema_version(&self) -> u32;

    // Writes (transaction-scoped)
    pub fn upsert_file(&self, f: &FileRow) -> Result<i64>;
    pub fn insert_nodes(&self, nodes: &[Node]) -> Result<Vec<i64>>;
    pub fn insert_edges(&self, edges: &[Edge]) -> Result<()>;
    pub fn delete_file_cascade(&self, file_id: i64) -> Result<()>;

    // Reads
    pub fn search_nodes(&self, q: &str, limit: u32) -> Result<Vec<Node>>;
    pub fn node_by_id(&self, id: i64) -> Result<Option<Node>>;
    pub fn nodes_by_name(&self, name: &str) -> Result<Vec<Node>>;
    pub fn callers_of(&self, id: i64) -> Result<Vec<Edge>>;
    pub fn callees_of(&self, id: i64) -> Result<Vec<Edge>>;
    pub fn files_under(&self, prefix: &str) -> Result<Vec<FileRow>>;
    pub fn stats(&self) -> Result<DbStats>;
}
```

## Schema

`schema.sql` (déjà ébauché):
- `meta(key, value)`: schema_version, last_index_ts, indexer_version.
- `files(id, path, language, sha256, size, mtime, indexed_at)` — path unique.
- `nodes(id, kind, name, qualified_name, file_id, start_line, end_line, signature, docstring, language)` — indices sur `name`, `qualified_name`, `file_id`, `kind`.
- `edges(id, from_id, to_id, kind, file_id, line)` — indices `(from_id, kind)` et `(to_id, kind)`.
- `nodes_fts` virtual FTS5 sur `name, qualified_name, signature, docstring`, `content='nodes' content_rowid='id'`, tokenizer `unicode61`.
- Triggers `nodes_ai`, `nodes_ad`, `nodes_au` pour sync FTS↔table.

## Migrations

`fn migrate(conn: &mut Connection)`:
1. Lit `meta.schema_version` (NULL = fresh).
2. Pour chaque version `<current_target`, applique `migrations/vNNN.sql` puis bump version dans `meta`.
3. Tout sous une transaction.

Pas de downgrade. Pas de migration depuis archive TS.

## Validation

- Tests unitaires `crates/codegraph-db/tests/`: open temp, insert 100 nodes/edges, search FTS, delete cascade.
- `cargo bench` (à voir) pour mesurer `insert_nodes(1000)` latence — référence pour optimiser batch size.

## Pièges

- FTS5 triggers doivent passer `delete-then-insert` sur UPDATE (pattern documenté SQLite).
- `bundled` ajoute ~1.5MB au binaire — accepté.
- WAL nécessite que le système supporte `mmap` shared; OK Linux/macOS/Windows.
