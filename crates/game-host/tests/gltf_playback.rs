use module_engine_content::ContentLibrary;
use module_engine_core::{
    AuthoredObject, Camera, CompositionContext, Frame, GameInput, GameProject, ModuleRegistry,
    ProfileIdentity, SceneSnapshot, SdkIdentity, SystemRegistry, Transform, WorldKind, WorldObject,
    demo_document, transform_point,
};
use module_engine_host::{GameHost, HostOptions, RenderMode};
use module_engine_host_services::HostServiceRegistry;
use module_engine_project::{AssetManifest, AssetWorkspace, asset_reference, hash_file};
use module_engine_vulkan::{RenderPath, default_graph, validation_error_count};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};

const WIDTH: usize = 480;
const HEIGHT: usize = 360;
const OBJECT: u64 = 1;

fn host(root: &Path, project: &Path, path: &str) -> (GameHost, String) {
    let mut document = demo_document(WorldKind::Ecs);
    document.custom_sources.clear();
    document.root.nodes.retain(|node| {
        matches!(
            node.id.as_str(),
            "world" | "extract" | "renderer" | "viewport"
        )
    });
    document
        .root
        .edges
        .retain(|edge| matches!(edge.from_node.as_str(), "world" | "extract" | "renderer"));
    let renderer = document
        .root
        .nodes
        .iter_mut()
        .find(|node| node.id == "renderer")
        .unwrap();
    renderer.module = "render.vulkan".into();
    renderer.subgraph = Some(Box::new(default_graph(RenderPath::Forward)));
    AssetWorkspace::new(project, Vec::<String>::new())
        .unwrap()
        .synchronize(&mut document)
        .unwrap();
    let manifest = AssetManifest::from_value(&document.asset_manifest).unwrap();
    let (id, _) = manifest
        .assets
        .iter()
        .find(|(_, entry)| entry.path == path)
        .unwrap();
    let reference = asset_reference(id);

    let mut object = AuthoredObject::new(WorldObject {
        id: OBJECT,
        parent: None,
        transform: Transform::default(),
        color: [255; 4],
    });
    object
        .components
        .insert("mesh".into(), json!({"path": reference}));
    let mut game = GameProject::empty();
    game.scenes.get_mut("empty").unwrap().objects = vec![object];
    document.game = Some(game);

    let mut modules = ModuleRegistry::builtin();
    module_engine_vulkan::install(&mut modules, root).unwrap();
    let composition = CompositionContext::new(modules, SystemRegistry::builtin()).unwrap();
    composition
        .capture_lock(
            &document,
            ProfileIdentity::from_payload("fixture.r71", "1", &json!({})).unwrap(),
            Some(SdkIdentity::current()),
        )
        .unwrap()
        .apply_to(&mut document);
    let mut host = GameHost::build(
        document,
        composition,
        HostOptions {
            asset_root: project.into(),
            backend_root: Some(root.into()),
            compile_root: None,
            render: RenderMode::Graph,
            services: HostServiceRegistry::new(),
            services_enabled: false,
        },
        None,
    )
    .unwrap();
    host.camera = fixed_camera(&host.author_snapshot().unwrap());
    (host, reference)
}

fn fixed_camera(scene: &SceneSnapshot) -> Camera {
    let mut low = [f32::INFINITY; 3];
    let mut high = [f32::NEG_INFINITY; 3];
    for item in &scene.items {
        for &vertex in &item.mesh.vertices {
            let point = transform_point(item.model, vertex);
            for (axis, value) in point.into_iter().enumerate() {
                low[axis] = low[axis].min(value);
                high[axis] = high[axis].max(value);
            }
        }
    }
    let extent = (0..3).map(|i| high[i] - low[i]).fold(0., f32::max);
    assert!(extent.is_finite() && extent > 0.);
    Camera {
        target: std::array::from_fn(|i| (high[i] + low[i]) / 2.),
        distance: (extent * 1.8).clamp(0.2, 1000.),
        yaw: 1.1,
        pitch: 0.15,
    }
}

fn moving_vertices(a: &[[f32; 3]], b: &[[f32; 3]]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .filter(|(a, b)| (0..3).any(|axis| (a[axis] - b[axis]).abs() > 0.000_01))
        .count()
}

fn png(path: &Path, frame: &Frame) {
    let mut encoder = png::Encoder::new(
        fs::File::create_new(path).unwrap(),
        frame.width as u32,
        frame.height as u32,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&frame.rgba)
        .unwrap();
}

#[test]
#[ignore = "requires pinned Khronos GLBs, Vulkan and DXC"]
fn khronos_glb_import_and_play() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let samples = PathBuf::from(std::env::var_os("MODULE_ENGINE_R71_SAMPLES").unwrap());
    let output = PathBuf::from(std::env::var_os("MODULE_ENGINE_R71_OUTPUT").unwrap());
    fs::create_dir(&output).expect("Use a new output directory with an existing parent");
    let before_validation = validation_error_count();
    let cesium = samples.join("CesiumMan.glb");
    let fox = samples.join("Fox.glb");
    assert_eq!(
        hash_file(&cesium).unwrap(),
        "b7001eaeea8254bd44773bcd247e78696d94169388fbb2a1800fc69434e777d9"
    );
    assert_eq!(
        hash_file(&fox).unwrap(),
        "d97044e701822bac5a62696459b27d7b375aada5de8574ed4362edbba94771f7"
    );
    for (label, source, clips) in [
        ("CesiumMan", cesium, &[None][..]),
        ("Fox", fox, &[Some("Survey"), Some("Walk"), Some("Run")][..]),
        (
            "static",
            root.join("assets/models/demo_crystal.glb"),
            &[][..],
        ),
    ] {
        let project = output.join(label);
        fs::create_dir(&project).unwrap();
        let mut library = ContentLibrary::new(&project).unwrap();
        let path = library.import_model(&source, "assets/imported").unwrap();
        assert_eq!(
            hash_file(&source).unwrap(),
            hash_file(&project.join(&path)).unwrap()
        );
        let model = library.load_model(&path).unwrap();
        assert_eq!(model.dependencies.iter().collect::<Vec<_>>(), vec![&path]);
        assert_eq!(
            model
                .animation_clips()
                .iter()
                .map(|clip| clip.name.as_deref())
                .collect::<Vec<_>>(),
            clips
        );
        for (index, clip) in model.animation_clips().iter().enumerate() {
            assert!(clip.duration > 0.);
            let a = model.sample_clip(index, clip.duration * 0.21).unwrap();
            let b = model.sample_clip(index, clip.duration * 0.62).unwrap();
            let moved: usize = a
                .iter()
                .zip(&b)
                .map(|(a, b)| moving_vertices(&a.mesh.vertices, &b.mesh.vertices))
                .sum();
            assert!(moved > 0, "{label} clip {index} must deform geometry");
            println!(
                "{label} clip {index} {:?} {:.5}s: {moved} moving vertices",
                clip.name, clip.duration
            );
        }
        let (mut host, reference) = host(&root, &project, &path);
        let camera = host.camera;
        let base_scene = host.author_snapshot().unwrap();
        let base_frame = host
            .try_render_interpolated(WIDTH, HEIGHT, 1.)
            .unwrap()
            .unwrap()
            .frame;
        assert!(base_frame.is_valid() && base_frame.stats.visible_objects > 0);
        png(&project.join("t000.png"), &base_frame);
        let mut tick = 0;
        for goal in [27, 54] {
            for _ in tick..goal {
                assert_eq!(host.step(1. / 60., &GameInput::default()).unwrap(), 1);
            }
            tick = goal;
            let snapshot = host.author_snapshot().unwrap();
            assert!(snapshot.time > base_scene.time);
            assert_eq!(host.camera, camera);
            assert_eq!(
                host.session.state().world.object(OBJECT).unwrap().transform,
                Transform::default()
            );
            let presented = host
                .try_render_interpolated(WIDTH, HEIGHT, 1.)
                .unwrap()
                .unwrap();
            assert_eq!(presented.provenance.current_tick, goal);
            assert_eq!(presented.camera, camera);
            let frame = presented.frame;
            assert!(frame.is_valid() && frame.stats.visible_objects > 0);
            let different_pixels = base_frame
                .rgba
                .chunks_exact(4)
                .zip(frame.rgba.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(snapshot.items.len(), base_scene.items.len());
            let mut moved = 0;
            for (a, b) in base_scene.items.iter().zip(&snapshot.items) {
                assert_eq!(a.id, b.id);
                assert_eq!(a.model, b.model);
                assert_eq!(a.material, b.material);
                assert_eq!(a.mesh.triangles, b.mesh.triangles);
                assert_eq!(a.mesh.texcoords, b.mesh.texcoords);
                moved += moving_vertices(&a.mesh.vertices, &b.mesh.vertices);
            }
            if clips.is_empty() {
                assert_eq!(snapshot.items, base_scene.items);
                assert_eq!(frame.rgba, base_frame.rgba);
            } else {
                assert!(
                    moved > 0 && different_pixels >= 100,
                    "{label} tick {goal} must show motion"
                );
            }
            let name = format!("t{goal:03}.png");
            png(&project.join(&name), &frame);
            println!(
                "{label} tick {goal} via {}: {moved} moving vertices, {different_pixels} changed pixels, image {name}",
                host.device
            );
        }
        assert!(
            host.try_render_interpolated(WIDTH, HEIGHT, 1.)
                .unwrap()
                .is_none()
        );
        host.restart().unwrap();
        assert_eq!(host.author_snapshot().unwrap(), base_scene);
        let restarted = host
            .try_render_interpolated(WIDTH, HEIGHT, 1.)
            .unwrap()
            .unwrap();
        assert_eq!(restarted.provenance.current_tick, 0);
        assert_eq!(base_frame.rgba, restarted.frame.rgba);
        assert_eq!(host.session.asset_bindings()[&OBJECT], reference);
    }
    assert_eq!(validation_error_count(), before_validation);
}
