use clepsydra_core::config::Config;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let schema = schemars::schema_for!(Config);
    let schema_json = serde_json::to_string_pretty(&schema)?;
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../clepsydra.schema.json");
    fs::write(path, schema_json)?;
    Ok(())
}
