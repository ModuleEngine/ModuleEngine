//! Puzzle authoring and runtime evidence through shared editor/project paths.
use crate::{game_ui::GameWorkbench, project, workspace_files};
use eframe::egui::{self, Event, Pos2, Rect, Vec2};
use module_engine_core::{GameState, GraphDocument, WorldKind};
use serde_json::{Value, json};
use std::{fs, path::Path};

const DT: f64 = 1. / 60.;
use module_engine_editor_composition::puzzle::verification::{board, require};

fn independent_activity(document: &GraphDocument, backend: &Path) -> Result<Value, String> {
    module_engine_editor_composition::puzzle::verification::independent_activity(
        document,
        &crate::host::composition_at(backend)?,
    )
}

fn label_position(output: &egui::FullOutput, label: &str) -> Result<Pos2, String> {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .ok_or_else(|| format!("Inspector did not render '{label}'"))
}

fn pointer(position: Pos2, pressed: bool) -> Vec<Event> {
    vec![
        Event::PointerMoved(position),
        Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

/// Drive the numeric widget and ordinary Apply button, without a draft backdoor.
fn inspector_edit(document: &mut GraphDocument, backend: &Path) -> Result<Value, String> {
    let before = board(document)?.clone();
    require(
        before == json!({}),
        "Expected the preset's unmaterialized board defaults",
    )?;
    let composition = crate::host::composition_at(backend)?;
    let resolved = composition
        .systems
        .components()
        .resolve_component("puzzle.board", &before);
    require(
        resolved["cell_size"] == json!(1.) && resolved["max_turns"] == json!(8),
        "Registered Puzzle defaults differ from its authoring contract",
    )?;
    let mut workbench = GameWorkbench::default();
    workbench.scene = "board".into();
    workbench.select(Some(1));
    workbench.set_engine_context(None, backend);
    let context = egui::Context::default();
    context.all_styles_mut(|style| style.animation_time = 0.);
    let mut time = 0.;
    let mut render = |workbench: &mut GameWorkbench, document: &mut GraphDocument, events| {
        time += DT;
        context.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200., 1600.))),
                time: Some(time),
                events,
                focused: true,
                ..Default::default()
            },
            |ui| {
                workbench.inspector_ui(ui, document);
            },
        )
    };
    let output = render(&mut workbench, document, vec![]);
    for label in [
        "puzzle.board",
        "width",
        "height",
        "cell_size",
        "max_turns",
        "Apply fields",
    ] {
        label_position(&output, label)?;
    }
    let row = label_position(&output, "cell_size")?;
    let number = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) => {
                let point = text.pos + text.galley.size() * 0.5;
                (point.x > row.x
                    && (point.y - row.y).abs() < 4.
                    && text.galley.text().parse::<f64>().is_ok())
                .then_some(point)
            }
            _ => None,
        })
        .min_by(|left, right| left.x.total_cmp(&right.x))
        .ok_or("Inspector did not render the cell_size numeric widget")?;
    output.drop_without_applying_deltas();
    render(&mut workbench, document, pointer(number, true)).drop_without_applying_deltas();
    let dragged = number + Vec2::new(12., 0.);
    render(&mut workbench, document, vec![Event::PointerMoved(dragged)])
        .drop_without_applying_deltas();
    let dragged = dragged + Vec2::new(12., 0.);
    render(&mut workbench, document, vec![Event::PointerMoved(dragged)])
        .drop_without_applying_deltas();
    let output = render(&mut workbench, document, pointer(dragged, false));
    require(
        board(document)? == &before,
        "Inspector changed authored data before Apply",
    )?;
    let apply = label_position(&output, "Apply fields")?;
    output.drop_without_applying_deltas();
    for pressed in [true, false] {
        render(&mut workbench, document, pointer(apply, pressed)).drop_without_applying_deltas();
    }
    if let Some(error) = &workbench.error {
        return Err(format!("Inspector: {error}"));
    }
    let after = board(document)?.clone();
    let size = after["cell_size"]
        .as_f64()
        .ok_or("Apply did not write cell_size")?;
    require(
        size > 1. && size <= 10.,
        "Real Inspector drag did not change cell_size",
    )?;
    require(
        after["width"] == resolved["width"]
            && after["height"] == resolved["height"]
            && after["max_turns"] == resolved["max_turns"],
        "Inspector Apply did not retain the other registered defaults",
    )?;
    require(
        workbench.host.is_none() && workbench.preview_host.is_none(),
        "Inspection initialized a runtime host",
    )?;
    composition.validate_game(document)?;
    project::stamp_composition_versions(document, &composition)?;
    Ok(
        json!({"path":"generic Inspector numeric DragValue and Apply fields","field":"puzzle.board.cell_size","authored_before":before,"resolved_defaults":resolved,"authored_after":after,"runtime_created_for_inspection":false}),
    )
}

fn hosted_workflow(
    document: &GraphDocument,
    asset_root: &Path,
    backend: &Path,
    kind: WorldKind,
    folder: &Path,
    label: &str,
) -> Result<(Value, GameState, Vec<u8>), String> {
    module_engine_editor_composition::puzzle::verification::hosted_workflow(
        document,
        asset_root,
        backend,
        kind,
        folder,
        label,
        &module_engine_editor_composition::puzzle::verification::Environment {
            build: |document, root, backend, kind| {
                crate::game_host::new_at(document, root, backend, None, kind)
            },
            write_png: crate::write_png,
        },
    )
}

/// Use an isolated case; the supplied project and all authored bytes stay intact.
pub fn run(source: &Path, folder: &Path) -> Result<Value, String> {
    let source_hash = module_engine_project::hash_file(source)?;
    let supplied = project::read(source)?;
    let distribution = crate::distribution::detect(source, &supplied, None)?;
    require(
        distribution.can_author_engine(),
        "Puzzle Full export verification requires the complete matching Full source package",
    )?;
    fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    let case = folder.join(format!(
        "case-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    ));
    fs::create_dir(&case).map_err(|error| error.to_string())?;
    let mut document = crate::game_host::puzzle_document();
    let activity = independent_activity(&document, &distribution.package_root)?;
    let inspector = inspector_edit(&mut document, &distribution.package_root)?;
    let saved = workspace_files::save_as(
        source,
        &case.join("author-project/project.json"),
        &document,
        &distribution,
    )?;
    project::save(&saved.path, &saved.document)?;
    let reopened = project::read(&saved.path)?;
    require(
        reopened == saved.document && board(&reopened)? == board(&document)?,
        "Public Save As/Save/reopen changed the authored puzzle",
    )?;
    let author_bytes = fs::read(&saved.path).map_err(|error| error.to_string())?;
    let author_root = saved.path.parent().ok_or("Saved project has no parent")?;
    let (objects, objects_state, objects_pixels) = hosted_workflow(
        &reopened,
        author_root,
        &distribution.package_root,
        WorldKind::Objects,
        &case,
        "objects",
    )?;
    let (ecs, ecs_state, ecs_pixels) = hosted_workflow(
        &reopened,
        author_root,
        &distribution.package_root,
        WorldKind::Ecs,
        &case,
        "ecs",
    )?;
    require(
        objects_state == ecs_state && objects_pixels == ecs_pixels,
        "Objects/ECS puzzle outcomes, world state or rendered geometry diverged",
    )?;
    let exported = project::export_with_package_root(
        &case.join("full-project"),
        &reopened,
        author_root,
        &distribution.package_root,
    )?;
    let exported_path = exported.join("project.json");
    let exported_document = project::read(&exported_path)?;
    let exported_capability =
        crate::distribution::detect(&exported_path, &exported_document, None)?;
    require(
        exported_capability.can_author_engine()
            && exported_capability
                .package_root
                .canonicalize()
                .map_err(|error| error.to_string())?
                == exported.canonicalize().map_err(|error| error.to_string())?,
        "Export did not reopen as its own complete editable source package",
    )?;
    require(
        exported_document.game == reopened.game,
        "Full source export changed the authored puzzle",
    )?;
    module_engine_project::AssetManifest::from_value(&exported_document.asset_manifest)?
        .verify(&exported)?;
    independent_activity(&exported_document, &exported)?;
    let (relocated, relocated_state, relocated_pixels) = hosted_workflow(
        &exported_document,
        &exported,
        &exported,
        WorldKind::Ecs,
        &case,
        "full-export",
    )?;
    require(
        relocated_state == ecs_state && relocated_pixels == ecs_pixels,
        "Relocated Full export changed the puzzle result or rendering",
    )?;
    require(
        fs::read(&saved.path).map_err(|error| error.to_string())? == author_bytes
            && module_engine_project::hash_file(source)? == source_hash,
        "Verification changed an authored project file",
    )?;
    let report = json!({"functional_checks_passed":true,"case":case,"fixture":"fresh application Puzzle preset; supplied project selects the checked Engine and resource context","source_project_unchanged":true,"author_project_bytes_unchanged":true,"activity":activity,"inspector":inspector,"save_reopen":{"public_save_as_save_read":true,"document_equal":true,"path":saved.path},"objects":objects,"ecs":ecs,"parity":{"logical_solved_state_equal":true,"rendered_rgba_equal":true},"full_source_export":{"path":exported,"editable_source_capability":true,"relocated_host":relocated,"logical_solved_state_equal":true,"rendered_rgba_equal":true},"scope":"Generic Inspector, checked project persistence, native GameHost simulation and Vulkan 3D rendering, and Full editable source export. This is not an independent Player package check. Empty authored audio cues are verified; audio service allocation and endpoint absence are outside this check."});
    let bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
    fs::write(case.join("puzzle-report.json"), &bytes).map_err(|error| error.to_string())?;
    fs::write(folder.join("puzzle-report.json"), bytes).map_err(|error| error.to_string())?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use module_engine_core::{GameInput, GameSession};
    use module_engine_editor_composition::puzzle::verification::{SOLUTION, on_goal, outcome};
    use module_engine_editor_composition::puzzle::{PuzzleOutcome, direction_input};

    #[test]
    fn generic_inspector_save_reopen_and_puzzle_only_runtime_preserve_authored_project() {
        let backend = project::source_root();
        let mut document = crate::game_host::puzzle_document();
        independent_activity(&document, &backend).unwrap();
        let inspection = inspector_edit(&mut document, &backend).unwrap();
        assert!(inspection["authored_after"]["cell_size"].as_f64().unwrap() > 1.);
        let root = std::env::temp_dir().join(format!(
            "ModuleEngine puzzle author {} {}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("original")).unwrap();
        let original = root.join("original/project.json");
        let distribution =
            crate::distribution::detect(&backend.join("project.json"), &document, None).unwrap();
        let saved = workspace_files::save_as(
            &original,
            &root.join("saved/project.json"),
            &document,
            &distribution,
        )
        .unwrap();
        project::save(&saved.path, &saved.document).unwrap();
        let reopened = project::read(&saved.path).unwrap();
        assert_eq!(reopened, saved.document);
        assert_eq!(board(&reopened).unwrap(), board(&document).unwrap());
        let bytes = fs::read(&saved.path).unwrap();
        for kind in [WorldKind::Objects, WorldKind::Ecs] {
            let registry = module_engine_editor_composition::puzzle::registry();
            assert!(registry.components().schema("health").is_none());
            assert!(registry.descriptor("game.combat").is_none());
            let mut session =
                GameSession::new_with_registry(reopened.game.clone().unwrap(), kind, registry)
                    .unwrap();
            session.step(DT, &GameInput::default()).unwrap();
            let initial = session.state().clone();
            let pawn = initial.world.object(100).unwrap();
            assert!(
                (pawn.transform.translation[2]
                    - board(&reopened).unwrap()["cell_size"].as_f64().unwrap() as f32)
                    .abs()
                    < 0.00001,
                "Inspector edit did not affect source system initialization"
            );
            for direction in SOLUTION {
                session.step(DT, &direction_input(direction)).unwrap();
            }
            outcome(&session, PuzzleOutcome::Solved, 5).unwrap();
            assert!(session.state().control.halted && on_goal(session.state()));
            session
                .step(
                    DT,
                    &GameInput {
                        restart: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            session.step(DT, &GameInput::default()).unwrap();
            assert_eq!(session.state(), &initial);
            assert_eq!(session.project(), reopened.game.as_ref().unwrap());
        }
        assert_eq!(fs::read(&saved.path).unwrap(), bytes);
        let root = root.canonicalize().unwrap();
        let temporary = std::env::temp_dir().canonicalize().unwrap();
        assert!(root.starts_with(&temporary) && root != temporary);
        fs::remove_dir_all(root).unwrap();
    }
}
