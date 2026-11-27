//! Shell boundary for the game-owned G1 acceptance scenario.
use module_engine_core::GraphDocument;
use module_engine_host::{GameHost, HostOptions, RenderMode};
use serde_json::Value;
use std::path::Path;

fn build(
    document: &GraphDocument,
    root: &Path,
    backend: &Path,
    state: &Path,
) -> Result<GameHost, String> {
    GameHost::build(
        document.clone(),
        crate::host::composition_at(backend)?,
        HostOptions {
            asset_root: root.into(),
            backend_root: Some(backend.into()),
            compile_root: None,
            render: RenderMode::Graph,
            services: module_engine_player::service_registry_at(state.into())?,
            services_enabled: true,
        },
        None,
    )
}
pub fn run(source: &Path, folder: &Path) -> Result<Value, String> {
    let document = crate::project::read(source)?;
    let distribution = crate::distribution::detect(source, &document, None)?;
    distribution.validate_document(&document)?;
    let backend = distribution.package_root;
    let composition = crate::host::composition_at(&backend)?;
    let selected = module_engine_player::native_features::installed_native_features()?
        .select_document(&document, &composition)?;
    module_engine_editor_composition::g1::verification::run(
        source,
        folder,
        document,
        backend,
        selected,
        &module_engine_editor_composition::g1::verification::Environment {
            build,
            write_png: module_engine_player::write_png,
        },
    )
}
