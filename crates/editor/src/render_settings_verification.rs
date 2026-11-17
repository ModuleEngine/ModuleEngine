//! A static authored sample and real GameHost frames for the AA delivery route.
//! This diagnostic never exports packages or changes existing projects.
use module_engine_core::{
    AuthoredName, AuthoredObject, Camera, Frame, GameInput, GameProject, GraphDocument, Transform,
    WorldKind, WorldObject, set_project_variant,
};
use module_engine_host::{GameHost, HostOptions, RenderMode};
use module_engine_host_services::HostServiceRegistry;
use module_engine_project::{externalize_scenes, hash_file};
use module_engine_vulkan::RenderPath;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

const WIDTH: usize = 960;
const HEIGHT: usize = 540;

fn require(ok: bool, reason: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(reason.into()) }
}

pub(crate) fn sample(backend: &Path) -> Result<GraphDocument, String> {
    let mut document = crate::host::gpu_document(WorldKind::Ecs, RenderPath::Forward);
    document.name = "Contrasting edges".into();
    document.root.nodes.retain(|node| {
        !matches!(
            node.module.as_str(),
            "custom.rust" | "custom.rust.update" | "clock.time" | "scalar.value"
        )
    });
    let ids = document
        .root
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<BTreeSet<_>>();
    document
        .root
        .edges
        .retain(|edge| ids.contains(&edge.from_node) && ids.contains(&edge.to_node));
    document.custom_sources.clear();
    document.project.rules.clear();
    let mut game = GameProject::empty();
    let scene = game
        .scenes
        .get_mut(&game.entry_scene)
        .ok_or("Sample scene missing")?;
    scene.name = "Contrasting edges".into();
    // Cubes are native renderer-neutral geometry. The thin, rotated bars expose
    // long high-contrast diagonals without imported assets or moving systems.
    for index in 0..6 {
        let mut object = AuthoredObject::new(WorldObject {
            id: index + 1,
            parent: None,
            transform: Transform {
                translation: [-0.35, -0.65 + index as f32 * 0.32, 0.35],
                rotation: [0., 0., 0.12 + index as f32 * 0.075],
                scale: [1.45, 0.025, 0.035],
            },
            color: [255, 255, 255, 255],
        });
        object.name = AuthoredName::Text(format!("White diagonal {}", index + 1));
        scene.objects.push(object);
    }
    let mut ground = AuthoredObject::new(WorldObject {
        id: 20,
        parent: None,
        transform: Transform {
            translation: [0., -1.05, 0.],
            scale: [3.3, 0.05, 2.5],
            ..Transform::default()
        },
        color: [10, 12, 16, 255],
    });
    ground.name = AuthoredName::Text("Dark ground".into());
    scene.objects.push(ground);
    document.game = Some(game);
    crate::host::ensure_authoring_configuration(&mut document);
    set_project_variant(&mut document, "antialiasing", "fxaa")?;
    crate::engine_profiles::finish_template(
        &mut document,
        crate::engine_profiles::ProfileKind::Neutral,
        backend,
    )?;
    externalize_scenes(&document, false)
}

fn authored_hashes(
    root: &Path,
    document: &GraphDocument,
) -> Result<BTreeMap<String, String>, String> {
    let mut paths = vec!["project.json".to_owned()];
    if let Some(game) = &document.game {
        paths.extend(
            game.scene_assets
                .values()
                .map(|binding| binding.path.clone()),
        );
    }
    paths
        .into_iter()
        .map(|relative| Ok((relative.clone(), hash_file(&root.join(relative))?)))
        .collect()
}

fn changed_pixels(a: &Frame, b: &Frame) -> usize {
    a.rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.rgba.as_chunks::<4>().0.iter())
        .filter(|(a, b)| a != b)
        .count()
}

/// Run with `module-engine-editor --render-settings-check <new-output-folder>`.
/// The result contains ordinary saved on/off projects, not delivery packages.
pub fn run(output: &Path) -> Result<Value, String> {
    if output.exists() {
        require(
            output.is_dir(),
            "Render settings output must be a directory",
        )?;
        require(
            fs::read_dir(output)
                .map_err(|error| error.to_string())?
                .next()
                .is_none(),
            "Render settings check needs a new or empty output directory",
        )?;
    }
    fs::create_dir_all(output).map_err(|error| error.to_string())?;
    let output = output.canonicalize().map_err(|error| error.to_string())?;
    let backend = module_engine_vulkan::resource_root();
    let on = sample(&backend)?;
    let mut off = on.clone();
    set_project_variant(&mut off, "antialiasing", "off")?;
    let mut off_without_choice = off.clone();
    off_without_choice.project = on.project.clone();
    require(
        off_without_choice == on,
        "AA pair differs beyond the Project selection",
    )?;
    let on_root = output.join("fixture-on");
    let off_root = output.join("fixture-off");
    fs::create_dir(&on_root).map_err(|error| error.to_string())?;
    fs::create_dir(&off_root).map_err(|error| error.to_string())?;
    crate::project::save(&on_root.join("project.json"), &on)?;
    crate::project::save(&off_root.join("project.json"), &off)?;
    require(
        crate::project::read(&on_root.join("project.json"))? == on,
        "On fixture failed save/reopen",
    )?;
    require(
        crate::project::read(&off_root.join("project.json"))? == off,
        "Off fixture failed save/reopen",
    )?;
    let on_hashes = authored_hashes(&on_root, &on)?;
    let off_hashes = authored_hashes(&off_root, &off)?;
    let original_on = on.clone();
    let original_off = off.clone();
    let mut host = GameHost::build(
        on.clone(),
        crate::host::composition_at(&backend)?,
        HostOptions {
            asset_root: on_root.clone(),
            backend_root: Some(backend.clone()),
            compile_root: None,
            render: RenderMode::Graph,
            services: HostServiceRegistry::new(),
            services_enabled: false,
        },
        None,
    )?;
    require(
        host.slots.is_empty(),
        "Static sample unexpectedly created Custom workers",
    )?;
    require(
        host.presentation_id().is_none(),
        "Static sample unexpectedly selected gameplay presentation",
    )?;
    require(
        host.camera == Camera::default(),
        "Static sample did not use the ordinary default camera",
    )?;
    // Match the relocated Player's --verify-package capture tick exactly.
    for _ in 0..3 {
        host.step(1. / 60., &GameInput::default())?;
    }
    host.set_detailed_stats(true);
    let state = host.session.state().clone();
    let camera = host.camera;
    let snapshot = host.author_snapshot()?;
    let enabled = host.render(WIDTH, HEIGHT)?;
    crate::write_png(&output.join("editor-on.png"), &enabled)?;
    host.sync_document(&off)?;
    let disabled = host.render(WIDTH, HEIGHT)?;
    crate::write_png(&output.join("editor-off.png"), &disabled)?;
    require(
        host.session.state() == &state
            && host.camera == camera
            && host.author_snapshot()? == snapshot,
        "AA edit changed camera, simulation or static geometry",
    )?;
    host.sync_document(&on)?;
    let restored = host.render(WIDTH, HEIGHT)?;
    crate::write_png(&output.join("editor-restored.png"), &restored)?;
    let pixels = changed_pixels(&enabled, &disabled);
    require(
        enabled.stats.shaded_pixels > 1000 && disabled.stats.shaded_pixels > 1000,
        "Contrasting edges fixture contains insufficient visible geometry",
    )?;
    require(
        pixels > 100,
        "FXAA/off did not change enough real GameHost pixels; inspect the fixture/render path",
    )?;
    require(
        enabled.rgba == restored.rgba,
        "Restoring FXAA did not restore the original frame exactly",
    )?;
    require(
        host.session.state() == &state && host.camera == camera,
        "Restoration changed simulation or camera",
    )?;
    require(
        on == original_on && off == original_off,
        "Diagnostic mutated authored input",
    )?;
    require(
        authored_hashes(&on_root, &on)? == on_hashes
            && authored_hashes(&off_root, &off)? == off_hashes,
        "Rendering changed saved authored bytes",
    )?;
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let report = json!({
        "schema_version":1,"kind":"render-settings-gamehost","passed":true,"native":true,
        "backend":host.device,"engine_version":env!("CARGO_PKG_VERSION"),
        "executable":executable,"executable_sha256":hash_file(&executable)?,
        "render_size":[WIDTH,HEIGHT],"camera":camera,"state":state,
        "changed_pixels":pixels,"restored_frame_exact":true,"world_camera_geometry_preserved":true,
        "authored_documents_unchanged":true,"authored_file_bytes_unchanged":true,"save_reopen_equal":true,
        "geometry_stats":enabled.stats,"custom_workers":0,"services_enabled":false,
        "fixture_on":{"project":on_root.join("project.json"),"files":on_hashes},
        "fixture_off":{"project":off_root.join("project.json"),"files":off_hashes},
        "composition":{"engine_profile":on.engine_profile,"engine_lock":on.engine_lock,"module_versions":on.module_versions},
        "scope":"Real native GameHost Vulkan CPU-readback frames using the Editor/Player host and matching Player verification resolution/tick. Native window presentation, human edge-quality assessment and actual Full/Team/Player export are separate checks. No Rust system compilation or gameplay controller."
    });
    fs::write(
        output.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(report)
}
