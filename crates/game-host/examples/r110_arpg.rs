//! Isolated benchmark over the stock native ARPG schedule and GameHost.
use module_engine_arpg::{ArpgInput, GameStateExt};
use module_engine_core::*;
use module_engine_host::{GameHost, HostOptions, RenderMode};
use module_engine_host_services::HostServiceRegistry;
use module_engine_project::hash_bytes;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

const DT: f64 = 1. / 60.;
const WARMUP: usize = 120;
const TICKS: usize = 600;
const ENEMIES: usize = 20;

struct RejectLate(Arc<AtomicBool>);
impl ExecutableSystem for RejectLate {
    fn step(&mut self, ctx: &mut SystemContext<'_>, state: &mut Value) -> Result<(), String> {
        let before = state.as_u64().unwrap_or(0);
        assert_eq!(ctx.resource("counter")?, &json!(before));
        let after = json!(before + 1);
        ctx.set_resource("counter", after.clone())?;
        ctx.game
            .module_data
            .insert("probe.rv_c4".into(), after.clone());
        ctx.events
            .push(GameEvent::new("probe.rv_c4", after.clone()));
        ctx.game.control.halted = self.0.load(Ordering::Relaxed);
        *state = after;
        Ok(())
    }

    fn commit_tick(&mut self) -> Result<(), String> {
        if self.0.load(Ordering::Relaxed) {
            return Err("RV-C.4 late commit rejection".into());
        }
        Ok(())
    }
}

fn build(reject: Option<Arc<AtomicBool>>) -> Result<GameHost, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut document = demo_document(WorldKind::Ecs);
    document.root.nodes.truncate(4);
    document.root.edges.truncate(3);
    document.custom_sources.clear();
    let mut project = module_engine_two_arenas::demo_project();
    let arena = project.scenes.get_mut("arena").unwrap();
    // Keep the authored spawn positions and the full eight-node schedule.
    // Raise both sides' HP for a sustained fight with exactly 20 living enemies.
    let health = json!({"maximum":10000.0,"current":10000.0});
    let player = arena
        .objects
        .iter_mut()
        .find(|entry| entry.object.id == 1)
        .unwrap();
    player.components.insert("health".into(), health.clone());
    project
        .prefabs
        .get_mut("small-monster")
        .unwrap()
        .components
        .insert("health".into(), health);
    let mut systems = SystemRegistry::builtin();
    module_engine_arpg::install(&mut systems)?;
    if let Some(reject) = reject {
        systems.register(
            SystemDescriptor {
                id: "probe.rv_c4".into(),
                state_schema: "probe.rv_c4/1".into(),
                components: vec![],
                ports: vec![SystemPort {
                    name: "counter".into(),
                    data_type: "Number".into(),
                    access: ResourceAccess::ReadWrite,
                }],
                requires: vec![],
            },
            move |_| Ok(Box::new(RejectLate(reject.clone()))),
        )?;
        project.execution.resources.push(SystemResource {
            name: "counter".into(),
            data_type: "Number".into(),
            external: true,
            initial: json!(0),
        });
        project.execution.stages.push(SystemStage {
            name: "Late rejection probe".into(),
            nodes: vec![SystemNode {
                id: "late".into(),
                module: "probe.rv_c4".into(),
                params: json!({}),
                bindings: BTreeMap::from([("counter".into(), "counter".into())]),
                after: vec!["system-7".into()],
            }],
        });
    }
    document.game = Some(project);
    let composition = CompositionContext::new(ModuleRegistry::builtin(), systems)?;
    let profile = document.engine_lock.as_ref().unwrap().profile.clone();
    composition
        .capture_lock(&document, profile, Some(SdkIdentity::current()))?
        .apply_to(&mut document);
    let mut host = GameHost::build(
        document,
        composition,
        HostOptions {
            asset_root: root,
            backend_root: None,
            compile_root: None,
            render: RenderMode::Headless {
                world_node: "world".into(),
            },
            services: HostServiceRegistry::default(),
            services_enabled: false,
        },
        None,
    )?;
    host.set_frame_profiling(true);
    assert_eq!(host.session.state().living_enemies(), ENEMIES);
    Ok(host)
}

fn input(index: usize) -> GameInput {
    let movement = match index % 180 {
        0..40 => [0., -0.6],
        40..65 => [0.5, 0.],
        65..105 => [0., 0.6],
        105..130 => [-0.5, 0.],
        _ => [0., 0.],
    };
    ArpgInput {
        movement,
        aim_world: [0.; 3],
        attack: index.is_multiple_of(22),
        ..Default::default()
    }
    .into_input()
}

fn accepted(host: &GameHost) -> Value {
    let state = host.session.state();
    json!({
        "state": state,
        "lifetimes": state.world.entity_ids().map(|id| (id, state.world.lifetime(id))).collect::<Vec<_>>(),
        "states": host.session.system_states(),

    })
}

fn digest(value: &Value) -> String {
    hash_bytes(&serde_json::to_vec(value).unwrap())
}

fn rollback(reject_tick: bool) -> Result<Value, String> {
    let reject = Arc::new(AtomicBool::new(false));
    let mut retried = build(Some(reject.clone()))?;
    for index in 0..WARMUP {
        let input = input(index);
        assert_eq!(retried.step(DT, &input)?, 1);
    }
    let before = accepted(&retried);
    assert_eq!(before["states"]["late"], json!(WARMUP));
    assert_eq!(before["state"]["module_data"]["probe.rv_c4"], json!(WARMUP));
    assert!(!retried.session.events().is_empty());
    let input = input(WARMUP);
    let failure = if reject_tick {
        reject.store(true, Ordering::Relaxed);
        let error = retried.step(DT, &input).expect_err("commit must reject");
        assert!(
            error.contains("System 'late' commit: RV-C.4 late commit rejection"),
            "{error}"
        );
        assert_eq!(before, accepted(&retried));
        assert!(retried.session.events().is_empty());
        reject.store(false, Ordering::Relaxed);
        // Advance using the rejected call's accumulated DT and no new time.
        assert_eq!(retried.step(0., &input)?, 1);
        Some(
            json!({"error":error,"previous_state_lifetimes_resources_system_states_control_preserved":true,
            "rejected_events_empty":true}),
        )
    } else {
        assert_eq!(retried.step(DT, &input)?, 1);
        None
    };
    let after = accepted(&retried);
    assert_eq!(after["states"]["late"], json!(WARMUP + 1));
    assert_eq!(retried.session.state().tick as usize, WARMUP + 1);
    // Compare the digests with a separate 'oracle' process so both processes
    // start with the same lifetime-ID counter. Rollback above checks the
    // original object's exact lifetimes in the same host.
    Ok(
        json!({"failure":failure,"accepted_before_sha256":digest(&before),
        "accepted_after_sha256":digest(&after), "ticks_after":WARMUP+1,
        "accepted_events_sha256":digest(&json!(retried.session.events()))}),
    )
}

fn measure() -> Result<Value, String> {
    let mut host = build(None)?;
    let fixture =
        hash_bytes(&serde_json::to_vec(host.accepted_document().game.as_ref().unwrap()).unwrap());
    for index in 0..WARMUP {
        assert_eq!(host.step(DT, &input(index))?, 1);
        host.session.take_profiled_ticks_ms().unwrap();
    }
    assert_eq!(host.session.state().living_enemies(), ENEMIES);
    let mut tick_ms = Vec::with_capacity(TICKS);
    let mut host_step_ms = Vec::with_capacity(TICKS);
    let mut events = Vec::with_capacity(TICKS);
    let mut counts = BTreeMap::new();

    for index in WARMUP..WARMUP + TICKS {
        let input = input(index);
        let start = Instant::now();
        let ticks = host.step(DT, &input)?;
        host_step_ms.push(start.elapsed().as_secs_f64() * 1000.);
        assert_eq!(ticks, 1);
        tick_ms.push(host.session.take_profiled_ticks_ms().unwrap());
        let tick_events = host.session.events();
        for event in tick_events {
            *counts.entry(event.name.clone()).or_insert(0) += 1;
        }
        events.push(tick_events.to_vec());
    }

    assert!(!counts.contains_key("Death"));
    assert!(counts.get("Hit").copied().unwrap_or(0) > 0, "{counts:?}");
    assert_eq!(host.session.state().living_enemies(), ENEMIES);
    let accepted = accepted(&host);
    Ok(
        json!({"fixture_sha256":fixture,"warmup":WARMUP,"ticks":TICKS,"living_enemies":ENEMIES,
        "event_counts":counts,"event_trace_sha256":digest(&json!(events)),"accepted_sha256":digest(&accepted),
        "tick_ms":tick_ms,"host_step_ms":host_step_ms}),
    )
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().ok_or("Expected total, rollback or oracle")?;
    let path = PathBuf::from(args.next().ok_or("Expected output JSON path")?);
    let report = match mode.as_str() {
        "total" => measure()?,

        "rollback" => rollback(true)?,
        "oracle" => rollback(false)?,
        _ => return Err(format!("Unknown mode {mode}")),
    };
    let file = std::fs::File::options()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(file, &report).map_err(|e| e.to_string())
}
