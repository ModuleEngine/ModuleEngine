//! S3 render-extraction acceptance through GameSession and GameHost.
use super::*;

#[path = "../../../examples/bench/s3_ecs.rs"]
mod s3;

const W: usize = 192;
const H: usize = 108;
const SYSTEM: &str = "bench.render_extraction";
const STATIC_ID: u64 = s3::MOVING as u64 + 1;
const LAST_ID: u64 = (s3::STATIC + s3::MOVING) as u64;
const NEW_ID: u64 = LAST_ID + 1;

struct Motion;
impl ExecutableSystem for Motion {
    fn step(&mut self, ctx: &mut SystemContext<'_>, _: &mut Value) -> Result<(), String> {
        if ctx.input.pressed("motion") {
            s3::tick(&mut ctx.game.world, ctx.game.tick as usize);
        }
        if ctx.input.pressed("schema") {
            ctx.game
                .world
                .component_mut(STATIC_ID, "bench.flags")
                .unwrap()["visible"] = json!(false);
        }
        if ctx.input.pressed("material") {
            ctx.game.world.insert_component(
                STATIC_ID,
                "material",
                json!({"base_color": [0.3, 0.6, 0.9, 1.], "base_color_texture": null}),
            )?;
        }
        if ctx.input.pressed("parent") {
            ctx.game.world.transform_mut(1).unwrap().translation[0] += 2.;
        }
        if ctx.input.pressed("reject") {
            return Err("fixture rejected staged writes".into());
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Packet {
    Reset(usize),
    Delta(Vec<(u64, SceneChangeKind)>),
}

#[derive(Default)]
struct Trace {
    packets: VecDeque<Packet>,
    decline: bool,
    snapshot: Option<SceneSnapshot>,
}

struct Receiver {
    inner: Box<dyn FrameRenderer>,
    trace: Arc<Mutex<Trace>>,
    // The CPU test exercises the same receiver used by the Vulkan adapter.
    cpu: Option<RenderScene>,
}
impl FrameRenderer for Receiver {
    fn backend_name(&self) -> &str {
        self.inner.backend_name()
    }

    fn performance(&self) -> RendererPerformance {
        self.inner.performance()
    }

    fn scene_sync(&self) -> SceneSync {
        self.cpu
            .as_ref()
            .map_or_else(|| self.inner.scene_sync(), RenderScene::sync)
    }

    fn update_scene(&mut self, update: SceneUpdate<'_>) -> Result<(), String> {
        let mut trace = self.trace.lock().unwrap();
        if trace.decline {
            return Err("fixture did not receive update".into());
        }
        trace.packets.push_back(match &update {
            SceneUpdate::Reset { snapshot, .. } => Packet::Reset(snapshot.items.len()),
            SceneUpdate::Delta { changes, .. } => Packet::Delta(
                changes
                    .iter()
                    .map(|change| (change.id, change.kind))
                    .collect(),
            ),
        });
        if let Some(scene) = &mut self.cpu {
            scene.apply(update)?;
            trace.snapshot = Some(scene.snapshot()?.clone());
            Ok(())
        } else {
            self.inner.update_scene(update)
        }
    }

    fn render(
        &mut self,
        _: &SceneSnapshot,
        _: Camera,
        _: usize,
        _: usize,
        _: &RenderOverrides,
    ) -> Result<Frame, String> {
        panic!("the host bypassed the retained receiver")
    }

    fn render_retained(
        &mut self,
        camera: Camera,
        width: usize,
        height: usize,
        controls: &RenderOverrides,
    ) -> Result<Frame, String> {
        if let Some(scene) = &self.cpu {
            self.inner
                .render(scene.snapshot()?, camera, width, height, controls)
        } else {
            self.inner.render_retained(camera, width, height, controls)
        }
    }

    fn render_retained_presented(
        &mut self,
        camera: Camera,
        width: usize,
        height: usize,
        controls: &RenderOverrides,
    ) -> Result<RenderSubmission, String> {
        if let Some(scene) = &self.cpu {
            self.inner
                .render_presented(scene.snapshot()?, camera, width, height, controls)
        } else {
            self.inner
                .render_retained_presented(camera, width, height, controls)
        }
    }
}

fn fixture(
    native: bool,
    objects: Vec<AuthoredObject>,
) -> (
    GameHost,
    Arc<Mutex<Trace>>,
    module_engine_script::TestArtifactGuard,
) {
    let temp =
        module_engine_script::TestArtifactGuard::new(&std::env::temp_dir(), "r16-extraction")
            .unwrap();
    let root = super::test_support::workspace_root();
    let mut modules = ModuleRegistry::builtin();
    let mut document = demo_document(WorldKind::Ecs);
    document.custom_sources.clear();
    document
        .root
        .nodes
        .retain(|n| matches!(n.id.as_str(), "world" | "extract" | "renderer" | "viewport"));
    document
        .root
        .edges
        .retain(|e| matches!(e.from_node.as_str(), "world" | "extract" | "renderer"));
    if native {
        module_engine_vulkan::install(&mut modules, &root).unwrap();
        let renderer = document
            .root
            .nodes
            .iter_mut()
            .find(|node| node.id == "renderer")
            .unwrap();
        renderer.module = "render.vulkan".into();
        renderer.subgraph = Some(Box::new(module_engine_vulkan::default_graph(
            module_engine_vulkan::RenderPath::Forward,
        )));
    }
    let mut systems = SystemRegistry::builtin();
    for component in s3::COMPONENTS {
        let fields = if component == "bench.flags" {
            vec![PropertySchema {
                id: "bench.flags.visible".into(),
                label: "Fixture visibility metadata".into(),
                scope: PropertyScope::Component {
                    component: component.into(),
                },
                value_type: PropertyType::Boolean,
                default: json!(true),
                requires_modules: vec![],
                requires_components: vec![],
            }]
        } else {
            vec![]
        };
        systems
            .components_mut()
            .register(ComponentSchema {
                id: component.into(),
                owner_module: SYSTEM.into(),
                version: "1".into(),
                label: component.into(),
                defaults: if fields.is_empty() {
                    json!({})
                } else {
                    json!({"visible": true})
                },
                fields,
                requires_modules: vec![],
                requires_components: vec![],
            })
            .unwrap();
    }
    systems
        .register(
            SystemDescriptor {
                id: SYSTEM.into(),
                state_schema: "bench.render_extraction/1".into(),
                components: s3::COMPONENTS.map(str::to_owned).to_vec(),
                ports: vec![],
                requires: vec![],
            },
            |_| Ok(Box::new(Motion)),
        )
        .unwrap();
    let mut game = GameProject::empty();
    game.scenes.get_mut("empty").unwrap().objects = objects;
    game.scenes.insert(
        "successor".into(),
        AuthoredScene {
            name: "Successor".into(),
            objects: vec![AuthoredObject::new(WorldObject {
                id: 1,
                ..s3::object(42)
            })],
        },
    );
    game.execution.stages.push(SystemStage {
        name: "motion".into(),
        nodes: vec![SystemNode {
            id: "motion".into(),
            module: SYSTEM.into(),
            params: json!({}),
            bindings: BTreeMap::new(),
            after: vec![],
        }],
    });
    document.game = Some(game);
    let composition = CompositionContext::new(modules, systems).unwrap();
    composition
        .capture_lock(
            &document,
            ProfileIdentity::from_payload("bench.r16", "1", &json!({})).unwrap(),
            Some(SdkIdentity::current()),
        )
        .unwrap()
        .apply_to(&mut document);
    let mut host = GameHost::build(
        document,
        composition,
        HostOptions {
            asset_root: temp.path().into(),
            backend_root: Some(root),
            compile_root: None,
            render: RenderMode::Graph,
            services: HostServiceRegistry::new(),
            services_enabled: false,
        },
        None,
    )
    .unwrap();
    let trace = Arc::new(Mutex::new(Trace::default()));
    host.renderer = Some(Box::new(Receiver {
        inner: host.renderer.take().unwrap(),
        trace: trace.clone(),
        cpu: (!native).then(RenderScene::default),
    }));
    (host, trace, temp)
}

fn step(host: &mut GameHost, actions: &[&str]) -> Result<usize, String> {
    host.step_budget(
        1. / 60.,
        &GameInput {
            actions: actions.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        },
        1,
    )
}

fn packet(trace: &Mutex<Trace>) -> Packet {
    let mut trace = trace.lock().unwrap();
    assert_eq!(trace.packets.len(), 1);
    trace.packets.pop_front().unwrap()
}

fn edit(moved: bool, material_changed: bool) -> SceneChangeKind {
    SceneChangeKind::Edited {
        moved,
        material_changed,
        mesh_changed: false,
    }
}

fn reference_frame(host: &mut GameHost, native: bool, trace: &Mutex<Trace>) -> Frame {
    let snapshot = host.author_snapshot().unwrap();
    if native {
        let route = host
            .composition
            .modules
            .render_route(&host.document)
            .unwrap();
        let mut reference = host
            .composition
            .modules
            .create_renderer(route.renderer, &host.document)
            .unwrap();
        let controls = host
            .effective_controls(&RenderOverrides::default())
            .unwrap();
        reference
            .render(&snapshot, host.camera, W, H, &controls)
            .unwrap()
    } else {
        assert_eq!(trace.lock().unwrap().snapshot.as_ref(), Some(&snapshot));
        render_scene(&snapshot, W, H, host.camera)
    }
}

fn s3_retained_scene(native: bool) {
    let objects = (0..s3::STATIC + s3::MOVING)
        .map(|i| {
            let mut authored = AuthoredObject::new(s3::object(i));
            if i < s3::ENTITIES {
                authored.components = s3::entity(s3::object(i)).components;
                // Authored schema columns require object payloads; the raw S3
                // fixture stores these two values as a scalar and null.
                for name in ["bench.team", "bench.target"] {
                    let value = authored.components.get_mut(name).unwrap();
                    *value = json!({"value": value.take()});
                }
            }
            authored
        })
        .collect();
    let (mut host, trace, _temp) = fixture(native, objects);
    host.camera = Camera {
        yaw: 0.25,
        pitch: 1.17,
        distance: 196.,
        target: [0.; 3],
    };
    host.set_frame_profiling(true);
    let initial = host.try_render_interpolated(W, H, 1.).unwrap().unwrap();
    assert_eq!(packet(&trace), Packet::Reset(11_000));
    assert!(initial.frame.is_valid());
    let initial_uploads = host.renderer_performance();
    if native {
        // One shared cube (8 vertices with position/UV and 36 indices).
        assert_eq!(initial_uploads.mesh_upload_bytes, 8 * 20 + 36 * 4);
        assert_eq!(initial_uploads.instance_upload_bytes, 11_000 * 64);
        assert_eq!(initial_uploads.material_row_upload_bytes, 11_000 * 20 + 4);
    }
    assert_eq!(step(&mut host, &["motion"]).unwrap(), 1);
    let moved = host.try_render_interpolated(W, H, 1.).unwrap().unwrap();
    let moved_uploads = host.renderer_performance();
    if native {
        assert_eq!(
            moved_uploads.mesh_upload_bytes,
            initial_uploads.mesh_upload_bytes
        );
        assert_eq!(
            moved_uploads.instance_upload_bytes - initial_uploads.instance_upload_bytes,
            1_000 * 64
        );
        assert_eq!(
            moved_uploads.material_row_upload_bytes,
            initial_uploads.material_row_upload_bytes
        );
    }
    let expected = (1..=s3::MOVING as u64)
        .map(|id| (id, edit(true, false)))
        .collect();
    assert_eq!(packet(&trace), Packet::Delta(expected));
    assert_eq!(
        moved.frame.rgba,
        reference_frame(&mut host, native, &trace).rgba
    );
    assert_ne!(initial.frame.rgba, moved.frame.rgba);
    assert!(host.try_render_interpolated(W, H, 1.).unwrap().is_none());
    assert!(trace.lock().unwrap().packets.is_empty());
    let repeated = host.render(W, H).unwrap();
    assert_eq!(packet(&trace), Packet::Delta(vec![]));
    if native {
        let repeat = host.renderer_performance();
        assert_eq!(repeat.mesh_upload_bytes, moved_uploads.mesh_upload_bytes);
        assert_eq!(
            repeat.instance_upload_bytes,
            moved_uploads.instance_upload_bytes
        );
        assert_eq!(
            repeat.material_row_upload_bytes,
            moved_uploads.material_row_upload_bytes
        );
        println!(
            "Host Vulkan S3 GPU scene bytes: initial mesh={} instance={} material={}; moved mesh=0 instance={} material=0; repeat all=0",
            initial_uploads.mesh_upload_bytes,
            initial_uploads.instance_upload_bytes,
            initial_uploads.material_row_upload_bytes,
            moved_uploads.instance_upload_bytes - initial_uploads.instance_upload_bytes,
        );
    }
    assert_eq!(repeated.rgba, moved.frame.rgba);
    println!(
        "S3 {}: initialized=11000, moved=1000 (exact IDs 1..=1000), stationary=10000, repeat_delta=0, rgba_sha256={}, extract_ms={:?}",
        host.device,
        module_engine_project::hash_bytes(&moved.frame.rgba),
        moved.frame.timings.as_ref().and_then(|t| t.cpu_extract_ms)
    );

    assert_eq!(step(&mut host, &["schema"]).unwrap(), 1);
    assert_eq!(
        host.session
            .state()
            .world
            .component(STATIC_ID, "bench.flags")
            .unwrap()["visible"],
        json!(false)
    );
    let schema = host.try_render_interpolated(W, H, 1.).unwrap().unwrap();
    assert_eq!(packet(&trace), Packet::Delta(vec![]));
    assert_eq!(schema.frame.rgba, moved.frame.rgba);
    assert_eq!(step(&mut host, &["material"]).unwrap(), 1);
    let material = host.try_render_interpolated(W, H, 1.).unwrap().unwrap();
    assert_eq!(
        packet(&trace),
        Packet::Delta(vec![(STATIC_ID, edit(false, true))])
    );
    assert_eq!(
        material.frame.rgba,
        reference_frame(&mut host, native, &trace).rgba
    );

    if native {
        let uploads = host.renderer_performance();
        assert_eq!(uploads.mesh_upload_bytes, moved_uploads.mesh_upload_bytes);
        assert_eq!(
            uploads.instance_upload_bytes,
            moved_uploads.instance_upload_bytes
        );
        assert_eq!(
            uploads.material_row_upload_bytes - moved_uploads.material_row_upload_bytes,
            16
        );
    }

    let spawned = WorldObject {
        id: NEW_ID,
        ..s3::object(LAST_ID as usize)
    };
    host.session
        .apply_commands(&[
            WorldCommand::Spawn { object: spawned },
            WorldCommand::Remove { id: LAST_ID },
        ])
        .unwrap();
    let structural = host.try_render_interpolated(W, H, 1.).unwrap().unwrap();
    assert_eq!(
        packet(&trace),
        Packet::Delta(vec![
            (LAST_ID, SceneChangeKind::Removed),
            (NEW_ID, SceneChangeKind::Spawned)
        ])
    );
    assert_eq!(
        structural.frame.rgba,
        reference_frame(&mut host, native, &trace).rgba
    );

    trace.lock().unwrap().decline = true;
    for _ in 0..2 {
        host.session
            .apply_commands(&[WorldCommand::Translate {
                id: NEW_ID,
                delta: [0., 3., 0.],
            }])
            .unwrap();
        assert!(host.render(W, H).unwrap_err().contains("did not receive"));
    }
    trace.lock().unwrap().decline = false;
    let recovered = host.render(W, H).unwrap();
    assert_eq!(packet(&trace), Packet::Reset(11_000));
    assert_eq!(
        recovered.rgba,
        reference_frame(&mut host, native, &trace).rgba
    );
    println!(
        "S3 {}: schema_only_delta=0, material_only=1, removed=1, spawned=1, missed_updates=2, recovery_reset=11000; full-snapshot RGBA parity passed",
        host.device
    );
}

#[test]
fn retained_s3_cpu() {
    s3_retained_scene(false);
}

#[test]
#[ignore = "requires a compatible Vulkan adapter and DXC"]
fn retained_s3_vulkan() {
    s3_retained_scene(true);
}

#[test]
fn retained_hierarchy_rollback_interpolation_and_scene_switch() {
    hierarchy_rollback_interpolation_and_scene_switch(false);
}

#[test]
#[ignore = "requires native Vulkan and DXC"]
fn retained_vulkan_hierarchy_rollback_interpolation_and_scene_switch() {
    hierarchy_rollback_interpolation_and_scene_switch(true);
}

fn hierarchy_rollback_interpolation_and_scene_switch(native: bool) {
    let objects = [None, Some(1), None]
        .into_iter()
        .enumerate()
        .map(|(i, parent)| {
            AuthoredObject::new(WorldObject {
                parent,
                transform: Transform {
                    translation: [i as f32, 0., 0.],
                    ..Default::default()
                },
                ..s3::object(i)
            })
        })
        .collect();
    let (mut host, trace, _temp) = fixture(native, objects);
    host.render_presented_for(7, W, H).unwrap().unwrap();
    assert_eq!(packet(&trace), Packet::Reset(3));
    host.render_presented_for(8, W, H).unwrap().unwrap();
    assert_eq!(packet(&trace), Packet::Delta(vec![]));
    step(&mut host, &["parent"]).unwrap();
    for alpha in [0.5, 0.75, 1.] {
        let rendered = host.try_render_interpolated(W, H, alpha).unwrap().unwrap();
        assert_eq!(rendered.provenance.alpha, alpha);
        assert_eq!(
            packet(&trace),
            Packet::Delta(vec![(1, edit(true, false)), (2, edit(true, false))])
        );
        if !native {
            let trace = trace.lock().unwrap();
            let items = &trace.snapshot.as_ref().unwrap().items;
            assert_eq!(items[0].model[0][3], (2. * alpha) as f32);
            assert_eq!(items[1].model[0][3], (1. + 2. * alpha) as f32);
            assert_eq!(items[2].model[0][3], 2.);
        }
    }
    let accepted = host.session.state().clone();
    let uploads_before_rejection = host.renderer_performance();
    assert!(step(&mut host, &["parent", "reject"]).is_err());
    assert_eq!(host.session.state(), &accepted);
    assert!(host.try_render_interpolated(W, H, 1.).unwrap().is_none());
    host.render(W, H).unwrap();
    assert_eq!(packet(&trace), Packet::Delta(vec![]));
    if native {
        let uploads = host.renderer_performance();
        assert_eq!(
            uploads.mesh_upload_bytes,
            uploads_before_rejection.mesh_upload_bytes
        );
        assert_eq!(
            uploads.instance_upload_bytes,
            uploads_before_rejection.instance_upload_bytes
        );
        assert_eq!(
            uploads.material_row_upload_bytes,
            uploads_before_rejection.material_row_upload_bytes
        );
    }
    host.session.transition("successor").unwrap();
    let successor = host.try_render_interpolated(W, H, 0.5).unwrap().unwrap();
    assert_eq!(
        successor.provenance.previous_tick,
        successor.provenance.current_tick
    );
    assert_eq!(successor.provenance.alpha, 1.);
    assert_eq!(
        packet(&trace),
        Packet::Delta(vec![
            (1, SceneChangeKind::Replaced),
            (2, SceneChangeKind::Removed),
            (3, SceneChangeKind::Removed)
        ])
    );
    assert_eq!(
        successor.frame.rgba,
        reference_frame(&mut host, native, &trace).rgba
    );
}
