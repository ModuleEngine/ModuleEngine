//! Diagnostic metadata only; not project/runtime or Full/Team acceptance.
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("Usage: capability_inventory <project.json> [installed-source-root]")?;
    let root = std::env::args_os()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."));
    let document = serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let composition = module_engine_player::composition_at(&root)?;
    // Registration does not create this directory or read game save files.
    let services =
        module_engine_player::service_registry_at(root.join(".work/capability-inventory-unused"))?;
    let inventory = module_engine_player::capabilities::capability_inventory(
        &document,
        &composition,
        &services,
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&inventory).map_err(|e| e.to_string())?
    );
    Ok(())
}
