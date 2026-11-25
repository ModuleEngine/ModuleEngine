use super::*;
use module_engine_core::{
    AuthoredObject, AuthoredScene, ExecutableSystem, GameEvent, GameInput, GameProject,
    GameSession, PhysicsHit, PhysicsPose, PhysicsQuery, PhysicsQueryError, PhysicsQueryFilter,
    PhysicsQueryResult, PhysicsShape, SystemContext, SystemNode, SystemResource, SystemStage,
    Transform, WorldKind, WorldObject,
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(crate) const BALL: u64 = 9_007_199_254_740_993;
pub(crate) const FLOOR: u64 = 100;
const SENSOR: u64 = 101;
const ANCHOR: u64 = 10;
const SUSPENDED: u64 = 11;
const MOVER: u64 = 12;
pub(crate) const INPUT: &str = "fixture.move";
pub(crate) const QUERY: &str = "fixture.query";

pub(crate) fn authored(
    id: u64,
    position: [f32; 3],
    components: &[(&str, Value)],
) -> AuthoredObject {
    AuthoredObject {
        object: WorldObject {
            id,
            parent: None,
            transform: Transform {
                translation: position,
                ..Default::default()
            },
            color: [180, 210, 245, 255],
        },
        name: Default::default(),
        prefab: None,
        components: components
            .iter()
            .map(|(key, value)| ((*key).into(), value.clone()))
            .collect(),
        overrides: Default::default(),
        references: Default::default(),
    }
}

pub(crate) fn node(id: &str) -> SystemNode {
    SystemNode {
        id: id.into(),
        module: id.into(),
        params: Value::Null,
        bindings: Default::default(),
        after: vec![],
    }
}

pub(crate) fn physics_node() -> SystemNode {
    let mut system = node(MODULE_ID);
    system.bindings = [
        ("world".into(), "world".into()),
        ("events".into(), "events".into()),
    ]
    .into();
    system
}

pub(crate) fn fixture(nodes: Vec<SystemNode>) -> GameProject {
    let objects = vec![
        authored(
            FLOOR,
            [0., -0.25, 0.],
            &[(COLLIDER, json!({"half_extents":[8., 0.25, 8.]}))],
        ),
        authored(
            BALL,
            [0., 2., 0.],
            &[
                (BODY, json!({})),
                (
                    COLLIDER,
                    json!({"kind":"ball","radius":0.3,"extra":{"authored":null}}),
                ),
            ],
        ),
        authored(
            SENSOR,
            [0., 1.1, 0.],
            &[(
                COLLIDER,
                json!({"sensor":true,"half_extents":[0.5,0.12,0.5]}),
            )],
        ),
        authored(ANCHOR, [-2., 1., 0.], &[(BODY, json!({"kind":"fixed"}))]),
        authored(
            SUSPENDED,
            [-2., 2., 0.],
            &[
                (BODY, json!({})),
                (COLLIDER, json!({"kind":"capsule_y"})),
                (JOINT, json!({"target":ANCHOR, "target_anchor":[0.,1.,0.]})),
            ],
        ),
        authored(
            MOVER,
            [4., 0.7, 0.],
            &[
                (BODY, json!({"kind":"kinematic"})),
                (COLLIDER, json!({"half_extents":[0.2,0.2,0.2]})),
            ],
        ),
    ];
    let resources = [
        ("world", "GameWorld"),
        ("events", "GameEvents"),
        ("input", "GameInput"),
    ]
    .into_iter()
    .map(|(name, data_type)| SystemResource {
        name: name.into(),
        data_type: data_type.into(),
        external: true,
        initial: Value::Null,
    })
    .collect();
    GameProject {
        entry_scene: "fixture".into(),
        scenes: [(
            "fixture".into(),
            AuthoredScene {
                name: "Physics fixture".into(),
                objects,
            },
        )]
        .into(),
        execution: ExecutionGraph {
            resources,
            stages: vec![SystemStage {
                name: "Fixture".into(),
                nodes,
            }],
        },
        player_preferences: None,
        scene_assets: Default::default(),
        audio_cues: Default::default(),
        prefabs: Default::default(),
    }
}

pub(crate) fn register(
    registry: &mut SystemRegistry,
    id: &str,
    factory: impl Fn(&Value) -> Result<Box<dyn ExecutableSystem>, String> + Send + Sync + 'static,
) {
    registry
        .register(
            SystemDescriptor {
                id: id.into(),
                state_schema: format!("{id}/1"),
                components: vec![],
                ports: vec![],
                requires: vec![],
            },
            factory,
        )
        .unwrap();
}

struct MoveFromInput;
impl ExecutableSystem for MoveFromInput {
    fn step(&mut self, ctx: &mut SystemContext<'_>, _: &mut Value) -> Result<(), String> {
        if let Some(x) = ctx.input.values.get("fixture_x").and_then(Value::as_f64) {
            ctx.game.world.transform_mut(MOVER).unwrap().translation[0] = x as f32;
        }
        Ok(())
    }
}

struct Query;
impl ExecutableSystem for Query {
    fn step(&mut self, ctx: &mut SystemContext<'_>, _: &mut Value) -> Result<(), String> {
        let ray = PhysicsQuery::Ray {
            origin: [0., 5., 0.],
            direction: [0., -2., 0.],
            max_distance: 10.,
            filter: PhysicsQueryFilter::default(),
        };
        let cast = PhysicsQuery::Cast {
            shape: PhysicsShape::Ball { radius: 0.15 },
            pose: PhysicsPose {
                position: [0., 5., 0.],
                ..Default::default()
            },
            direction: [0., -1., 0.],
            max_distance: 10.,
            filter: PhysicsQueryFilter::default(),
        };
        let overlap = PhysicsQuery::Overlap {
            shape: PhysicsShape::Box {
                half_extents: [0.5, 0.5, 0.5],
            },
            pose: PhysicsPose {
                position: [0., 1.1, 0.],
                ..Default::default()
            },
            filter: PhysicsQueryFilter {
                exclude_object: Some(BALL),
                include_sensors: true,
            },
        };
        let results = [ray, cast, overlap].map(|query| ctx.query_physics(&query));
        ctx.events.push(GameEvent::new(QUERY, json!(results)));
        Ok(())
    }
}

pub(crate) struct LateReject(pub Arc<AtomicBool>);
impl ExecutableSystem for LateReject {
    fn step(&mut self, _: &mut SystemContext<'_>, _: &mut Value) -> Result<(), String> {
        Ok(())
    }

    fn commit_tick(&mut self) -> Result<(), String> {
        if self.0.swap(false, Ordering::Relaxed) {
            return Err("fixture late physics rejection".into());
        }
        Ok(())
    }
}

pub(crate) fn register_reject(registry: &mut SystemRegistry, flag: Arc<AtomicBool>) -> SystemNode {
    let id = "fixture.reject";
    register(registry, id, move |_| {
        Ok(Box::new(LateReject(flag.clone())))
    });
    node(id)
}

pub(crate) fn inputs() -> Vec<GameInput> {
    (0..90)
        .map(|tick| GameInput {
            values: [("fixture_x".into(), json!(4. - (tick % 24) as f32 * 0.02))].into(),
            ..Default::default()
        })
        .collect()
}

pub(crate) fn captures(session: &GameSession) -> Value {
    let state = session.state();
    json!({"tick": state.tick, "scene": state.scene,
        "entities": state.world.entities_snapshot(), "events": session.events()})
}

fn replay(recording: &[u8]) -> (Vec<Value>, GameSession) {
    let mut systems = SystemRegistry::builtin();
    install(&mut systems).unwrap();
    register(&mut systems, INPUT, |_| Ok(Box::new(MoveFromInput)));
    register(&mut systems, QUERY, |_| Ok(Box::new(Query)));
    let mut session = GameSession::new_with_registry(
        fixture(vec![node(INPUT), physics_node(), node(QUERY)]),
        WorldKind::Ecs,
        systems,
    )
    .unwrap();
    let mut trace = vec![];
    for input in serde_json::from_slice::<Vec<GameInput>>(recording).unwrap() {
        assert_eq!(session.step(1. / 60., &input).unwrap(), 1);
        trace.push(captures(&session));
    }
    (trace, session)
}

#[test]
fn recorded_input_replays_transforms_ordered_contacts_and_queries() {
    let recording = serde_json::to_vec(&inputs()).unwrap();
    let (first, session) = replay(&recording);
    let (second, _) = replay(&recording);
    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );

    let first_results = first[0]["events"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|event| event.get(QUERY))
        .unwrap();
    let [ray, cast, overlap]: [module_engine_core::PhysicsQueryResponse; 3] =
        serde_json::from_value(first_results.clone()).unwrap();
    let Ok(PhysicsQueryResult::Hit(Some(PhysicsHit {
        object, distance, ..
    }))) = ray
    else {
        panic!("Missing ray hit: {ray:?}");
    };
    assert_eq!(object, BALL);
    assert!((2.65..2.8).contains(&distance), "ray distance {distance}");
    let Ok(PhysicsQueryResult::Hit(Some(hit))) = cast else {
        panic!("Missing shape hit: {cast:?}")
    };
    assert_eq!(hit.object, BALL);
    assert!((distance - hit.distance - 0.15).abs() < 0.01);
    assert_eq!(overlap, Ok(PhysicsQueryResult::Overlap(vec![SENSOR])));

    let events: Vec<_> = first
        .iter()
        .flat_map(|frame| frame["events"].as_array().unwrap())
        .collect();
    for name in [COLLISION_STARTED, COLLISION_STOPPED] {
        assert!(
            events.iter().any(|event| event
                .get(name)
                .is_some_and(|payload| { *payload == json!({"a":SENSOR,"b":BALL,"sensor":true}) })),
            "missing {name} for sensor"
        );
    }
    assert!(events.iter().any(|event| {
        event.get(COLLISION_STARTED) == Some(&json!({"a":FLOOR,"b":BALL,"sensor":false}))
    }));
    let y = |id| session.state().world.transform(id).unwrap().translation[1];
    assert!(
        (0.27..0.32).contains(&y(BALL)),
        "ball did not rest on the floor: {}",
        y(BALL)
    );
    assert!(
        (1.98..2.02).contains(&y(SUSPENDED)),
        "joint did not hold: {}",
        y(SUSPENDED)
    );
}

#[test]
fn invalid_delta_preserves_the_last_accepted_solver_state() {
    use module_engine_core::SystemExecutor;
    let project = fixture(vec![physics_node()]);
    let mut registry = SystemRegistry::builtin();
    install(&mut registry).unwrap();
    let session =
        GameSession::new_with_registry(project.clone(), WorldKind::Ecs, registry.clone()).unwrap();
    let mut candidate = session.state().clone();
    let mut expected = candidate.clone();
    let mut candidate_executor = SystemExecutor::new(&project.execution, &registry).unwrap();
    let mut expected_executor = SystemExecutor::new(&project.execution, &registry).unwrap();
    let mut candidate_events = vec![];
    let mut expected_events = vec![];
    let input = GameInput::default();
    let fixed_dt = 1. / 60.;
    for _ in 0..2 {
        candidate_executor
            .step(&mut candidate, &input, fixed_dt, &mut candidate_events)
            .unwrap();
        expected_executor
            .step(&mut expected, &input, fixed_dt, &mut expected_events)
            .unwrap();
    }
    let err = candidate_executor
        .step(&mut candidate, &input, fixed_dt * 2., &mut candidate_events)
        .unwrap_err();
    assert!(err.contains("Rapier requires the 60 Hz game tick"), "{err}");
    assert_eq!(
        candidate.world.transform(BALL),
        expected.world.transform(BALL)
    );
    candidate_executor
        .step(&mut candidate, &input, fixed_dt, &mut candidate_events)
        .unwrap();
    expected_executor
        .step(&mut expected, &input, fixed_dt, &mut expected_events)
        .unwrap();
    assert_eq!(
        candidate.world.entities_snapshot(),
        expected.world.entities_snapshot()
    );
    assert_eq!(candidate_events, expected_events);
}

#[test]
fn queries_ignore_a_replaced_entity_with_the_same_stable_id() {
    use module_engine_core::WorldCommand;
    struct ReplaceBall;
    impl ExecutableSystem for ReplaceBall {
        fn step(&mut self, context: &mut SystemContext<'_>, _: &mut Value) -> Result<(), String> {
            let mut object = context.game.world.object(BALL).unwrap();
            object.transform.translation = [8., 3., 0.];
            context.apply_commands(&[
                WorldCommand::Remove { id: BALL },
                WorldCommand::Spawn { object },
            ])?;
            context.game.world.insert_component(
                BALL,
                COLLIDER,
                json!({"kind":"ball","radius":0.3}),
            )?;
            Ok(())
        }
    }
    let replace = "fixture.replace";
    let mut registry = SystemRegistry::builtin();
    install(&mut registry).unwrap();
    register(&mut registry, replace, |_| Ok(Box::new(ReplaceBall)));
    register(&mut registry, QUERY, |_| Ok(Box::new(Query)));
    let mut session = GameSession::new_with_registry(
        fixture(vec![physics_node(), node(replace), node(QUERY)]),
        WorldKind::Ecs,
        registry,
    )
    .unwrap();
    let accepted = session.state().world.lifetime(BALL);
    assert_eq!(session.step(1. / 60., &GameInput::default()).unwrap(), 1);
    assert_ne!(session.state().world.lifetime(BALL), accepted);
    let outputs: Vec<module_engine_core::PhysicsQueryResponse> =
        serde_json::from_value(session.events().last().unwrap().payload.clone()).unwrap();
    assert!(matches!(
        outputs[0],
        Ok(PhysicsQueryResult::Hit(Some(PhysicsHit {
            object: FLOOR,
            ..
        })))
    ));
}

#[test]
fn installed_physics_is_inactive_without_a_scheduled_step() {
    let mut registry = SystemRegistry::builtin();
    install(&mut registry).unwrap();
    register(&mut registry, QUERY, |_| Ok(Box::new(Query)));
    let project = fixture(vec![node(QUERY)]);
    let mut session =
        GameSession::new_with_registry(project.clone(), WorldKind::Ecs, registry).unwrap();
    assert!(session.state().world.component(BALL, BODY).is_none());
    assert_eq!(session.project(), &project);
    assert_eq!(session.step(1. / 60., &GameInput::default()).unwrap(), 1);
    let outputs: Vec<module_engine_core::PhysicsQueryResponse> =
        serde_json::from_value(session.events()[0].payload.clone()).unwrap();
    assert_eq!(outputs, vec![Err(PhysicsQueryError::Unavailable); 3]);
    assert_eq!(
        session.state().world.transform(BALL).unwrap().translation[1],
        2.
    );
}

#[cfg(feature = "debug-draw")]
#[test]
fn debug_lines_show_author_preview_colliders_sensors_and_joints_without_a_tick() {
    let mut registry = SystemRegistry::builtin();
    install(&mut registry).unwrap();
    let session =
        GameSession::new_with_registry(fixture(vec![physics_node()]), WorldKind::Ecs, registry)
            .unwrap();
    let lines = debug_lines(&session.state().world).unwrap();
    for kind in [
        DebugLineKind::Body,
        DebugLineKind::Collider,
        DebugLineKind::Sensor,
        DebugLineKind::Joint,
    ] {
        assert!(
            lines.iter().any(|line| line.kind == kind),
            "missing {kind:?}"
        );
    }
    assert!(lines.iter().any(|line| line.kind == DebugLineKind::Collider
        && (1.69..2.31).contains(&line.start[1])
        && line.start[0].abs() < 0.31));
    assert_eq!(session.state().tick, 0);
}
