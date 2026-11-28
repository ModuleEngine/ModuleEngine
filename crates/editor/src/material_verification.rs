//! End-to-end R5.1 capture on unmodified, pinned Khronos source models.
use super::{BrowserAction, Editor, import_resource_transaction};
use crate::{distribution, game_host, game_package, project, workspace_files};
use eframe::egui;
use module_engine_content::{ContentLibrary, LoadedModel};
use module_engine_core::{
    AuthoredObject, MATERIAL_ASSET_COMPONENT, TextureEncoding, Transform, WorldObject,
    transform_point,
};
use module_engine_project::{AssetManifest, AssetWorkspace, asset_reference, hash_file};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

const REVISION: &str = "e124e8d8d7330a5372bc2b4d00d39a4a9d7997ea";
const SAMPLES: [(&str, &str, usize); 2] = [
    (
        "DamagedHelmet",
        "Models/DamagedHelmet/glTF-Binary/DamagedHelmet.glb",
        1,
    ),
    (
        "FlightHelmet",
        "Models/FlightHelmet/glTF/FlightHelmet.gltf",
        6,
    ),
];

pub fn run(source: &Path, fixtures: &Path, output: &Path) -> Result<Value, String> {
    let source = source.canonicalize().map_err(|e| e.to_string())?;
    let original_hash = hash_file(&source)?;
    let original = project::read(&source)?;
    let distribution = distribution::detect(&source, &original, None)?;
    let compiler = distribution.compile_root()?;
    let backend = &distribution.package_root;
    let fixtures = fixtures.canonicalize().map_err(|e| e.to_string())?;
    let source_hashes = verify_originals(&fixtures)?;
    fs::create_dir(output).map_err(|e| format!("Material evidence directory: {e}"))?;
    let output = output.canonicalize().map_err(|e| e.to_string())?;
    let personal = output.join("editor-personal");
    fs::create_dir(&personal).map_err(|e| e.to_string())?;

    let created = workspace_files::new_project(
        &source,
        &output.join("author"),
        &original,
        &distribution,
        workspace_files::ProjectTemplate::Empty,
    )?;
    let root = created.path.parent().ok_or("Author project has no root")?;
    let mut document = created.document;
    let workspace = AssetWorkspace::new(root, Vec::<String>::new())?;
    let mut imported = Vec::new();
    let mut inventory = Vec::new();
    for (ordinal, (name, relative, primitives)) in SAMPLES.into_iter().enumerate() {
        let path = import_resource_transaction(root, &fixtures.join(relative), "assets", |path| {
            Ok((path.clone(), workspace.adopt_import(&document, &[path])?))
        })?;
        let (model_path, operation) = path;
        document = operation.document;
        let manifest = AssetManifest::from_value(&document.asset_manifest)?;
        let mut content = ContentLibrary::new(root)?;
        let model = content.load_model(&model_path)?;
        inventory.push(inspect_model(name, primitives, &model)?);
        let id = manifest
            .assets
            .iter()
            .find(|(_, asset)| asset.path == model_path)
            .map(|(id, _)| id)
            .ok_or("Imported model has no stable asset ID")?;
        let mut object = AuthoredObject::new(WorldObject {
            id: (ordinal + 1) as u64,
            parent: None,
            transform: frame_sample(&model, if ordinal == 0 { -1.15 } else { 1.15 })?,
            color: [255; 4],
        });
        object
            .components
            .insert("mesh".into(), json!({"path": asset_reference(id)}));
        imported.push(object);
    }
    let game = document
        .game
        .as_mut()
        .ok_or("Acceptance project has no game")?;
    game.scenes
        .get_mut(&game.entry_scene)
        .ok_or("Acceptance project has no scene")?
        .objects = imported;
    project::save(&created.path, &document)?;
    let author_distribution = distribution::detect(&created.path, &document, None)?;
    let mut editor = Editor::new_with_diagnostic_personal_root(
        document,
        created.path.clone(),
        author_distribution,
        Some(personal),
    );
    let context = egui::Context::default();
    let material_path = "assets/Helmet surface.material";
    asset_command(
        &mut editor,
        &context,
        BrowserAction::NewMaterial {
            path: material_path.into(),
        },
    )?;
    let manifest = AssetManifest::from_value(&editor.document.asset_manifest)?;
    let (material_id, _) = manifest
        .assets
        .iter()
        .find(|(_, entry)| entry.path == material_path)
        .ok_or("New Material did not create an asset entry")?;

    let mut candidate = editor.document.clone();
    let game = candidate.game.as_mut().unwrap();
    game.scenes.get_mut(&game.entry_scene).unwrap().objects[0]
        .components
        .insert(
            MATERIAL_ASSET_COMPONENT.into(),
            json!({"asset": asset_reference(material_id)}),
        );
    editor.commit_scene_document(candidate, "Assign Helmet surface")?;
    if !editor.save_workflow() {
        return Err(editor.status.clone());
    }

    let mut captures = Vec::new();
    for (label, roughness) in [("before", 0.95), ("after", 0.12)] {
        let manifest = AssetManifest::from_value(&editor.document.asset_manifest)?;
        let bytes = serde_json::to_vec_pretty(&json!({"schema_version":1,"roughness":roughness}))
            .map_err(|e| e.to_string())?;
        asset_command(
            &mut editor,
            &context,
            BrowserAction::SaveMaterial {
                id: material_id.clone(),
                expected_hash: manifest.assets[material_id].sha256.clone(),
                bytes,
            },
        )?;
        let document = project::read(&created.path)?;
        let world = game_host::selected_world_kind(&document, &editor.registry)?;
        let mut authored =
            game_host::new_at(document.clone(), root, backend, Some(&compiler), world)?;
        authored.set_services_enabled(false);
        let snapshot = authored.author_snapshot()?;
        if snapshot
            .items
            .iter()
            .filter(|item| item.id == 1)
            .any(|item| {
                item.material
                    .pbr
                    .as_ref()
                    .is_none_or(|surface| surface.roughness != roughness as f32)
            })
            || !snapshot.items.iter().any(|item| item.id == 1)
        {
            return Err("Saved material roughness did not reach the authored snapshot".into());
        }

        let mut graph_evidence = Vec::new();
        let mut gpu = module_engine_vulkan::VulkanRenderer::new_at(backend.clone())
            .map_err(|e| e.to_string())?;
        for (name, path) in [
            ("forward", module_engine_vulkan::RenderPath::Forward),
            ("deferred", module_engine_vulkan::RenderPath::Deferred),
        ] {
            let evidence = output.join(format!("{label}-{name}.png"));
            let frame = gpu
                .render(
                    &snapshot,
                    authored.camera,
                    960,
                    540,
                    &module_engine_vulkan::default_graph(path),
                    &Default::default(),
                    &Default::default(),
                )
                .map_err(|e| e.to_string())?;
            if frame.stats.triangles == 0 {
                return Err(format!("{label} {name}: no rendered triangles"));
            }
            let work = gpu.last_frame_work().ok_or("GPU work receipt missing")?;
            if work.material_bc7_images != 14 || work.material_bc5_images != 6 {
                return Err(format!(
                    "{label} {name}: expected 14 BC7 and six BC5 images, got {} BC7 and {} BC5 on {}",
                    work.material_bc7_images,
                    work.material_bc5_images,
                    gpu.device_name()
                ));
            }
            module_engine_player::write_png(&evidence, &frame)?;
            graph_evidence.push(json!({"path":name,"adapter":gpu.device_name(),"width":frame.width,"height":frame.height,
                "file":evidence.file_name(),"sha256":hash_file(&evidence)?,"work":work}));
        }
        let preview = output.join(format!("{label}-authored.png"));
        let frame = authored.render(960, 540)?;
        module_engine_player::write_png(&preview, &frame)?;
        drop(gpu);
        drop(authored);

        let player = game_package::export(
            &document,
            root,
            backend,
            &compiler,
            &output.join(format!("{label}-player")),
        )?;
        let player_output = output.join(format!("{label}-player-evidence"));
        let executable = player.join(module_engine_player::package::PLAYER_EXECUTABLE);
        let status = Command::new(&executable)
            .arg("--verify-package")
            .arg(&player)
            .arg(&player_output)
            .current_dir(&player)
            .output()
            .map_err(|e| e.to_string())?;
        fs::write(
            output.join(format!("{label}-player-stdout.log")),
            &status.stdout,
        )
        .map_err(|e| e.to_string())?;
        fs::write(
            output.join(format!("{label}-player-stderr.log")),
            &status.stderr,
        )
        .map_err(|e| e.to_string())?;
        if !status.status.success() {
            return Err(format!(
                "{label} standalone Player: {}",
                String::from_utf8_lossy(&status.stderr)
            ));
        }
        let player_frame = player_output.join("player-frame.png");
        let authored_hash = hash_file(&preview)?;
        let player_hash = hash_file(&player_frame)?;
        if authored_hash != player_hash {
            return Err(format!(
                "{label}: authored preview and standalone Player produced different pixels"
            ));
        }
        let manifest_bytes = fs::read(player.join("player.json")).map_err(|e| e.to_string())?;
        let package: game_package::PlayerManifest =
            serde_json::from_slice(&manifest_bytes).map_err(|e| e.to_string())?;
        let block_count = package
            .files
            .keys()
            .filter(|path| path.starts_with(".textures/"))
            .count();
        if block_count != 20 {
            return Err(format!(
                "Player sealed {block_count} BC texture files, expected 20"
            ));
        }
        captures.push(
            json!({"label":label,"roughness":roughness,"author_png_sha256":authored_hash,
            "player_png_sha256":player_hash,"player_executable_sha256":hash_file(&executable)?,
            "player_block_textures":block_count,"graphs":graph_evidence}),
        );
    }
    if captures[0]["player_png_sha256"] == captures[1]["player_png_sha256"] {
        return Err("Material edit produced identical Player pixels".into());
    }
    if hash_file(&source)? != original_hash {
        return Err("Material check changed the input project".into());
    }
    let report = json!({"schema_version":1,"check":"R5.1 material import and Player", "khronos_revision":REVISION,
        "fixture_hashes":source_hashes,"models":inventory,"captures":captures,"input_project_unchanged":true});
    fs::write(
        output.join("material-report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(report)
}

fn verify_originals(fixtures: &Path) -> Result<Value, String> {
    let mut paths = serde_json::Map::new();
    for line in include_str!("material_khronos_sha256.txt").lines() {
        let (expected, relative) = line
            .split_once("  ")
            .ok_or("Invalid fixture checksum entry")?;
        let file = module_engine_project::resolve_relative(fixtures, relative)?;
        let actual = hash_file(&file)?;
        if actual != expected {
            return Err(format!("Unexpected Khronos input SHA-256: {relative}"));
        }
        paths.insert(relative.into(), actual.into());
    }
    Ok(Value::Object(paths))
}

fn inspect_model(name: &str, primitive_count: usize, model: &LoadedModel) -> Result<Value, String> {
    if model.primitives.len() != primitive_count {
        return Err(format!("{name}: expected {primitive_count} primitives"));
    }
    let mut images = BTreeSet::new();
    let mut emissive = 0;
    for primitive in &model.primitives {
        let material = &primitive.material;
        let surface = material.pbr.as_ref().ok_or("glTF PBR data is missing")?;
        if primitive.mesh.normals.len() != primitive.mesh.vertices.len()
            || name == "FlightHelmet"
                && primitive.mesh.tangents.len() != primitive.mesh.vertices.len()
        {
            return Err(format!(
                "{name}: missing source normal or tangent attributes"
            ));
        }
        emissive += usize::from(surface.emissive_texture.is_some());
        for (role, texture, encoding) in [
            (
                "base color",
                &material.base_color_texture,
                TextureEncoding::Srgb,
            ),
            ("normal", &surface.normal_texture, TextureEncoding::Normal),
            (
                "metallic/roughness",
                &surface.metallic_roughness_texture,
                TextureEncoding::Linear,
            ),
            (
                "occlusion",
                &surface.occlusion_texture,
                TextureEncoding::Linear,
            ),
            (
                "emissive",
                &surface.emissive_texture,
                TextureEncoding::SrgbEmissive,
            ),
        ] {
            if role == "emissive" && name == "FlightHelmet" && texture.is_none() {
                continue;
            }
            let texture = texture
                .as_ref()
                .ok_or_else(|| format!("{name} is missing its {role} map"))?;
            let blocks = texture
                .blocks
                .as_ref()
                .ok_or_else(|| format!("{name} {role} has no BC mips"))?;
            if blocks.encoding != encoding {
                return Err(format!("{name} {role} has the wrong BC format"));
            }
            images.insert(std::sync::Arc::as_ptr(texture) as usize);
        }
    }
    Ok(
        json!({"name":name,"primitives":primitive_count,"distinct_maps":images.len(),"emissive_primitives":emissive,
        "imported_dependencies":model.dependencies}),
    )
}

fn frame_sample(model: &LoadedModel, x: f32) -> Result<Transform, String> {
    let mut lo = [f32::INFINITY; 3];
    let mut hi = [f32::NEG_INFINITY; 3];
    for primitive in &model.primitives {
        for vertex in &primitive.mesh.vertices {
            let point = transform_point(primitive.transform, *vertex);
            for axis in 0..3 {
                lo[axis] = lo[axis].min(point[axis]);
                hi[axis] = hi[axis].max(point[axis]);
            }
        }
    }
    let span = lo
        .into_iter()
        .zip(hi)
        .map(|(low, high)| high - low)
        .fold(0., f32::max);
    if !span.is_finite() || span <= 0. {
        return Err("Khronos mesh bounds are invalid".into());
    }
    let scale = 1.9 / span;
    let center: [f32; 3] = std::array::from_fn(|axis| lo[axis].midpoint(hi[axis]));
    Ok(Transform {
        translation: [
            x - center[0] * scale,
            -center[1] * scale,
            -center[2] * scale,
        ],
        scale: [scale; 3],
        ..Default::default()
    })
}

fn asset_command(
    editor: &mut Editor,
    context: &egui::Context,
    action: BrowserAction,
) -> Result<(), String> {
    editor.request_asset_action(action, context);
    if editor.workflow.job.is_none() {
        return Err(format!(
            "Editor did not start the asset command: {}",
            editor.status
        ));
    }
    let started = Instant::now();
    while editor.workflow.job.is_some() && started.elapsed() < Duration::from_secs(90) {
        context
            .run_ui(egui::RawInput::default(), |ui| {
                editor.workflow_frame(ui.ctx())
            })
            .drop_without_applying_deltas();
        std::thread::sleep(Duration::from_millis(5));
    }
    if editor.workflow.job.is_some() || editor.workflow.show_report {
        return Err(format!(
            "Editor asset command did not complete: {}",
            editor.workflow.report
        ));
    }
    Ok(())
}
