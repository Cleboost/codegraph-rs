use crate::{db_err, SCHEMA_SQL, SCHEMA_VERSION};
use codegraph_core::Result;
use rusqlite::Connection;

pub(crate) fn run(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction().map_err(db_err)?;
    tx.execute_batch(SCHEMA_SQL).map_err(db_err)?;
    tx.execute(
        "INSERT INTO meta(key, value) VALUES('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [SCHEMA_VERSION.to_string()],
    )
    .map_err(db_err)?;
    tx.commit().map_err(db_err)?;
    Ok(())
}
