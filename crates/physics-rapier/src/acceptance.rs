use super::{
    tests::{BALL, authored, captures, fixture, node, physics_node, register_reject},
    *,
};
use module_engine_arpg::{GameStateExt, GameStatus};
use module_engine_core::{
    CompositionContext, ExecutableSystem, GameInput, GraphDocument, ModuleRegistry,
    ProfileIdentity, SdkIdentity, SystemContext, SystemNode, SystemStage, WorldCommand, WorldKind,
};
use module_engine_host::{GameHost, HostOptions, RenderMode};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn headless(mut document: GraphDocument, systems: SystemRegistry, compile: bool) -> GameHost {
    // The stock content and schedule are used without a window. Connect the
    // canonical ECS explicitly and capture fresh fixture pins through Core.
    document.root.nodes.retain(|node| node.id == "world");
    document.root.edges.clear();
    document.engine.enabled_modules.insert("world.ecs".into());
    let composition = CompositionContext::new(ModuleRegistry::builtin(), systems).unwrap();
    composition
        .capture_lock(
            &document,
            ProfileIdentity::from_payload("r61.fixture", "1", &json!({"r61":1})).unwrap(),
            Some(SdkIdentity::current()),
        )
        .unwrap()
        .apply_to(&mut document);
    let root = root();
    GameHost::build(
        document,
        composition,
        HostOptions {
            asset_root: root.clone(),
            backend_root: None,
            compile_root: compile.then_some(root),
            render: RenderMode::Headless {
                world_node: "world".into(),
            },
            services: Default::default(),
            services_enabled: false,
        },
        None,
    )
    .unwrap()
}

fn fixture_host(flag: Arc<AtomicBool>, custom: bool) -> GameHost {
    let mut registry = SystemRegistry::builtin();
    install(&mut registry).unwrap();
    let mut document = module_engine_core::demo_document(WorldKind::Ecs);
    document.custom_sources.clear();
    let mut nodes = vec![physics_node(), register_reject(&mut registry, flag)];
    if custom {
        install_custom(&mut registry);
        document
            .custom_sources
            .insert("physics-probe".into(), PROBE.into());
        nodes.insert(0, custom_node("probe-before", "physics-probe"));
        nodes.insert(2, custom_node("probe-after", "physics-probe"));
    }
    document.game = Some(fixture(nodes));
    headless(document, registry, custom)
}

fn rollback_and_retry(custom: bool) {
    let reject = Arc::new(AtomicBool::new(false));
    let mut candidate = fixture_host(reject.clone(), custom);
    let mut expected = fixture_host(Arc::new(AtomicBool::new(false)), custom);
    let input = GameInput::default();
    let mut rejected = false;
    for _ in 0..75 {
        assert_eq!(expected.step(1. / 60., &input).unwrap(), 1);
        let collide = expected
            .session
            .events()
            .iter()
            .any(|event| event.name == COLLISION_STARTED);
        if !rejected && collide {
            let accepted = captures(&candidate.session);
            let lifetime = candidate.session.state().world.lifetime(BALL);
            reject.store(true, Ordering::Relaxed);
            let error = candidate.step(1. / 60., &input).unwrap_err();
            assert!(error.contains("fixture late physics rejection"), "{error}");
            assert_eq!(captures(&candidate.session), accepted);
            assert_eq!(candidate.session.state().world.lifetime(BALL), lifetime);
            assert_eq!(candidate.step(0., &input).unwrap(), 1);
            rejected = true;
        } else {
            assert_eq!(candidate.step(1. / 60., &input).unwrap(), 1);
        }
        assert_eq!(captures(&candidate.session), captures(&expected.session));
    }
    assert!(
        rejected,
        "late commit was never exercised on a collision tick"
    );
    if custom {
        let events = candidate.session.events();
        let state = events
            .iter()
            .find(|event| event.name == "fixture.physics")
            .unwrap();
        assert!(state.payload["runs"].as_u64().unwrap() >= 70);
        assert!(
            events
                .iter()
                .any(|event| event.name == "fixture.physics_unavailable")
        );
    }
}

#[test]
fn game_host_late_commit_rejects_the_solver_and_retry_preserves_contacts() {
    rollback_and_retry(false);
}

#[test]
#[ignore = "compiles and runs actual Custom Rust physics consumers through GameHost"]
fn custom_rust_queries_share_the_physics_step_and_rollback() {
    rollback_and_retry(true);
}

fn install_custom(registry: &mut SystemRegistry) {
    registry
        .register(
            SystemDescriptor {
                id: "custom.rust.update".into(),
                state_schema: "owned-rust-state-v1".into(),
                components: vec![],
                requires: vec![],
                ports: ["world", "input", "events"]
                    .into_iter()
                    .zip(["GameWorld", "GameInput", "GameEvents"])
                    .map(|(name, data_type)| SystemPort {
                        name: name.into(),
                        data_type: data_type.into(),
                        access: if name == "input" {
                            ResourceAccess::Read
                        } else {
                            ResourceAccess::ReadWrite
                        },
                    })
                    .collect(),
            },
            |_| Err("Custom is bound by GameHost".into()),
        )
        .unwrap();
}

fn custom_node(id: &str, source: &str) -> SystemNode {
    SystemNode {
        id: id.into(),
        module: "custom.rust.update".into(),
        params: json!({"source_id":source}),
        after: vec![],
        bindings: ["world", "input", "events"]
            .into_iter()
            .map(|pin| (pin.into(), pin.into()))
            .collect(),
    }
}

const PROBE: &str = r#"// module-engine-state: {"schema":"physics-fixture/1","initial":{"runs":0}}
fn update(ctx: &mut ScriptContext) {
    use module_engine_core::{PhysicsPose, PhysicsQuery, PhysicsQueryError, PhysicsQueryFilter, PhysicsQueryResult, PhysicsShape};
    let ray = PhysicsQuery::Ray {
        origin:[0.,5.,0.], direction:[0.,-1.,0.], max_distance:10., filter:PhysicsQueryFilter::default(),
    };
    if !ctx.physics_queries_available() {
        assert_eq!(ctx.query_physics(&ray), Err(PhysicsQueryError::Unavailable));
        ctx.emit_event(serde_json::json!("fixture.physics_unavailable"));
        return;
    }
    let Ok(PhysicsQueryResult::Hit(Some(hit))) = ctx.query_physics(&ray) else { panic!("expected ray hit") };
    assert_eq!(hit.object, 9007199254740993);
    let invalid = PhysicsQuery::Ray {
        origin:[f32::NAN,0.,0.], direction:[0.,1.,0.], max_distance:10., filter:PhysicsQueryFilter::default(),
    };
    assert!(matches!(ctx.query_physics(&invalid), Err(PhysicsQueryError::InvalidInput(_))));
    let sweep = ctx.query_physics(&PhysicsQuery::Cast {
        shape:PhysicsShape::Ball {radius:0.12}, pose:PhysicsPose {position:[0.,5.,0.],rotation:[0.;3]},
        direction:[0.,-1.,0.], max_distance:10., filter:PhysicsQueryFilter::default(),
    }).unwrap();
    let overlap = ctx.query_physics(&PhysicsQuery::Overlap {
        shape:PhysicsShape::Ball {radius:0.1}, pose:PhysicsPose {position:[0.,1.1,0.],rotation:[0.;3]},
        filter:PhysicsQueryFilter {exclude_object:Some(9007199254740993),include_sensors:true},
    }).unwrap();
    assert_eq!(overlap, PhysicsQueryResult::Overlap(vec![101]));
    let runs = ctx.state["runs"].as_u64().unwrap() + 1;
    ctx.state["runs"] = serde_json::json!(runs);
    ctx.emit_event(serde_json::json!({"fixture.physics":{"runs":runs,"ray":hit,"sweep":sweep}}));
}
"#;

struct TimedStep {
    inner: simulation::RapierStep,
    samples: Arc<Mutex<Vec<u128>>>,
}

impl ExecutableSystem for TimedStep {
    fn step(&mut self, ctx: &mut SystemContext<'_>, state: &mut Value) -> Result<(), String> {
        let start = Instant::now();
        let result = self.inner.step(ctx, state);
        let elapsed = start.elapsed().as_nanos();
        self.samples.lock().unwrap().push(elapsed);
        result
    }
    fn commit_tick(&mut self) -> Result<(), String> {
        self.inner.commit_tick()
    }
    fn rollback_tick(&mut self) -> Result<(), String> {
        self.inner.rollback_tick()
    }
}

const PROJECTILES: u64 = 32;
const PROJECTILE_ID: u64 = 2000;
const WARMUP: usize = 15;
const SAMPLES: usize = 120;
const DRIVER: &str = "fixture.projectiles";

fn projectile(slot: u64) -> module_engine_core::AuthoredObject {
    let x = (slot % 16) as f32 * 0.82 - 6.15;
    let z = 5. + (slot / 16) as f32 * 0.65;
    authored(
        PROJECTILE_ID + slot,
        [x, 0.2, z],
        &[
            (
                BODY,
                json!({"kind":"dynamic","gravity_scale":0.0,
            "linear_velocity":[0.,0.,-28.],"ccd":true,"mass":0.015}),
            ),
            (
                COLLIDER,
                json!({"kind":"ball","radius":0.085,"friction":0.0}),
            ),
        ],
    )
}

struct Projectiles;
impl ExecutableSystem for Projectiles {
    fn step(&mut self, ctx: &mut SystemContext<'_>, _: &mut Value) -> Result<(), String> {
        let slot = ctx.game.tick % PROJECTILES;
        let next = projectile(slot);
        let id = next.object.id;
        ctx.apply_commands(&[
            WorldCommand::Remove { id },
            WorldCommand::Spawn {
                object: next.object,
            },
        ])?;
        for (component, value) in next.components {
            ctx.game.world.insert_component(id, component, value)?;
        }
        Ok(())
    }
}

fn s2_document() -> GraphDocument {
    let mut document =
        module_engine_two_arenas::document(module_engine_core::demo_document(WorldKind::Ecs));
    let game = document.game.as_mut().unwrap();
    let scene = game.scenes.get_mut("arena").unwrap();
    for entry in &mut scene.objects {
        if entry.object.id == 1 || (10..30).contains(&entry.object.id) {
            entry
                .components
                .insert(BODY.into(), json!({"kind":"kinematic"}));
            entry.components.insert(
                COLLIDER.into(),
                json!({
                    "kind":"capsule_y","radius":entry.object.transform.scale[0],"half_height":0.3,
                }),
            );
        } else if (1000..=1006).contains(&entry.object.id) {
            entry.components.insert(
                COLLIDER.into(),
                json!({"half_extents":entry.object.transform.scale}),
            );
        }
    }
    scene.objects.extend((0..PROJECTILES).map(projectile));
    game.execution.stages.push(SystemStage {
        name: "R6.1 projectile fixture".into(),
        nodes: vec![node(DRIVER), physics_node()],
    });
    document
}

#[test]
#[ignore = "scored Release S2 physics run with the stock controller and a continuous CCD projectile load"]
fn release_s2_with_projectiles_stays_within_one_ms_per_tick() {
    assert!(
        !std::hint::black_box(cfg!(debug_assertions)),
        "run this physics benchmark in Release"
    );
    let mut all_pass = true;
    for run in 1..=3 {
        let samples = Arc::new(Mutex::new(Vec::with_capacity(WARMUP + SAMPLES)));
        let mut registry = SystemRegistry::builtin();
        module_engine_arpg::install(&mut registry).unwrap();
        install_custom(&mut registry);
        let timings = samples.clone();
        install_with(&mut registry, move |params| {
            Ok(Box::new(TimedStep {
                inner: simulation::RapierStep::new(params)?,
                samples: timings.clone(),
            }))
        })
        .unwrap();
        registry
            .register(
                SystemDescriptor {
                    id: DRIVER.into(),
                    state_schema: format!("{DRIVER}/1"),
                    components: vec![],
                    ports: vec![],
                    requires: vec![],
                },
                |_| Ok(Box::new(Projectiles)),
            )
            .unwrap();
        let mut host = headless(s2_document(), registry, true);
        let input = module_engine_arpg::ArpgInput::default().into_input();
        let mut contacts = 0;
        for tick in 0..WARMUP + SAMPLES {
            assert_eq!(host.step(1. / 60., &input).unwrap(), 1);
            let state = host.session.state();
            assert_eq!(state.arpg().status, GameStatus::Playing);
            assert_eq!(state.scene, "arena");
            assert_eq!(state.living_enemies(), 20);
            assert!((PROJECTILE_ID..PROJECTILE_ID + PROJECTILES).all(|id| {
                state.world.component(id, BODY).is_some()
                    && state.world.component(id, COLLIDER).is_some()
            }));
            if tick >= WARMUP {
                contacts += host
                    .session
                    .events()
                    .iter()
                    .filter(|event| {
                        event.name == COLLISION_STARTED
                            && [event.payload["a"].as_u64(), event.payload["b"].as_u64()]
                                .into_iter()
                                .flatten()
                                .any(|id| {
                                    (PROJECTILE_ID..PROJECTILE_ID + PROJECTILES).contains(&id)
                                })
                    })
                    .count();
            }
        }
        assert!(contacts > 0, "unexercised projectile fixture");
        let samples = samples.lock().unwrap();
        assert_eq!(samples.len(), WARMUP + SAMPLES);
        let scored = &samples[WARMUP..];
        let mut sorted = scored.to_vec();
        sorted.sort_unstable();
        let max_ns = sorted[SAMPLES - 1];
        let mean_ns = scored.iter().sum::<u128>() as f64 / SAMPLES as f64;
        println!(
            "{}",
            json!({
                "run":run, "scenario":"S2 Small monsters with projectiles", "simulation_hz":60,
                "mode":"headless GameHost with the stock Two Arenas Custom controller", "living_enemies":20,
                "projectiles_per_tick":PROJECTILES, "projectile_contacts":contacts, "warmup_ticks":WARMUP,
                "step_ns":scored, "mean_ns":mean_ns, "p95_ns":sorted[(SAMPLES * 95).div_ceil(100)-1],
                "max_ns":max_ns, "limit_ns":1_000_000, "pass":max_ns <= 1_000_000,
            })
        );
        all_pass &= max_ns <= 1_000_000;
    }
    assert!(all_pass, "one or more scored physics ticks exceeded 1 ms");
}
