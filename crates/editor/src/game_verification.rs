//! Supply the checked application roots and rendering services to the example.
use module_engine_core::GraphDocument;
use module_engine_editor_composition::two_arenas::verification;
use std::path::Path;

pub fn run(
    document: &GraphDocument,
    root: &Path,
    compile_root: Option<&Path>,
    folder: &Path,
) -> Result<serde_json::Value, String> {
    verification::run(
        &verification::VerificationHost {
            build: crate::game_host::new,
            composition: crate::host::composition_at,
            write_png: crate::write_png,
        },
        document,
        root,
        compile_root,
        folder,
    )
}
