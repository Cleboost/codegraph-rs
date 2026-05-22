use anyhow::Result;
use camino::Utf8Path;
use serde_json::Value;

pub fn read_or_default(path: &Utf8Path) -> Result<Value> {
    if !path.exists() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    let bytes = std::fs::read(path.as_std_path())?;
    if bytes.is_empty() {
        return Ok(Value::Object(serde_json::Map::new()));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn write_pretty(path: &Utf8Path, v: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent.as_std_path())?;
    }
    let s = serde_json::to_string_pretty(v)?;
    std::fs::write(path.as_std_path(), format!("{s}\n"))?;
    Ok(())
}
