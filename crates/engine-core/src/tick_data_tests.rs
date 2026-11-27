use crate::*;
use serde_json::{Value, json};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

thread_local! {
    static CODECS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

struct Counter(Vec<u64>);
fn read(game: &GameState) -> Result<Counter, String> {
    CODECS.update(|(reads, writes)| (reads + 1, writes));
    serde_json::from_value(
        game.module_data
            .get("counter")
            .cloned()
            .unwrap_or(json!([])),
    )
    .map(Counter)
    .map_err(|error| error.to_string())
}
fn write(counter: Counter, game: &mut GameState) -> Result<(), String> {
    CODECS.update(|(reads, writes)| (reads, writes + 1));
    game.module_data.insert("counter".into(), json!(counter.0));
    Ok(())
}

struct NativeCounter(bool);
impl ExecutableSystem for NativeCounter {
    fn tick_data_contribution(&self) -> Option<&str> {
        Some("fixture.native")
    }
    fn step(&mut self, ctx: &mut SystemContext<'_>, state: &mut Value) -> Result<(), String> {
        self.0 = ctx.input.pressed("reject");
        ctx.with_tick_data::<Counter, _>(|ctx, counter| {
            counter.0.push(counter.0.len() as u64);
            *state = json!(counter.0.len());
            ctx.events.push(GameEvent::new("count", state.clone()));
            Ok(())
        })
        .expect("Running native data contract")
    }
    fn commit_tick(&mut self) -> Result<(), String> {
        if self.0 {
            Err("late rejection".into())
        } else {
            Ok(())
        }
    }
}

struct PortableCounter;
impl ExecutableSystem for PortableCounter {
    fn step(&mut self, ctx: &mut SystemContext<'_>, _: &mut Value) -> Result<(), String> {
        let current = ctx.game.module_data.get_mut("counter").unwrap();
        assert_eq!(current, &json!([0, 1]));
        if ctx.input.pressed("corrupt") {
            *current = json!("invalid");
        } else {
            current.as_array_mut().unwrap().push(json!(2));
        }
        Ok(())
    }
}

fn fixture(modules: &[&str]) -> (SystemExecutor, GameState) {
    let mut registry = SystemRegistry::default();
    for module in ["native", "portable"] {
        registry
            .register(
                SystemDescriptor {
                    id: module.into(),
                    state_schema: "1".into(),
                    components: vec![],
                    ports: vec![],
                    requires: vec![],
                },
                move |_| {
                    Ok(if module == "native" {
                        Box::new(NativeCounter(false)) as Box<dyn ExecutableSystem>
                    } else {
                        Box::new(PortableCounter)
                    })
                },
            )
            .unwrap();
    }
    registry
        .register_game_contribution(GameContribution {
            id: "fixture.native".into(),
            activation_modules: BTreeSet::from(["native".into()]),
            begin_tick: |_| Err("portable begin must be replaced".into()),
            initial_camera: |_| Camera::default(),
            follow_target: |_| None,
            decorate_snapshot: |_, _| {},
            script_input: |_| Value::Null,
            validate_event: |_| Ok(()),
            validate_state: |game| {
                serde_json::from_value::<Vec<u64>>(
                    game.module_data
                        .get("counter")
                        .cloned()
                        .unwrap_or(json!([])),
                )
                .map(|_| ())
                .map_err(|error| error.to_string())
            },
            service_requests: |_| Ok(vec![]),
            diagnostics: |_| Value::Null,
        })
        .unwrap();
    registry
        .register_tick_data("fixture.native", read, write)
        .unwrap();
    assert!(
        registry
            .register_tick_data("fixture.native", read, write)
            .is_err()
    );
    let graph = ExecutionGraph {
        resources: vec![],
        stages: vec![SystemStage {
            name: "Test".into(),
            nodes: modules
                .iter()
                .enumerate()
                .map(|(index, module)| SystemNode {
                    id: index.to_string(),
                    module: (*module).into(),
                    params: Value::Null,
                    bindings: BTreeMap::new(),
                    after: vec![],
                })
                .collect(),
        }],
    };
    let game = GameState {
        scene: "empty".into(),
        tick: 0,
        time: 0.,
        world: EcsWorld::from_entities(vec![]).unwrap(),
        module_data: BTreeMap::new(),
        control: SessionControl::default(),
    };
    (SystemExecutor::new(&graph, &registry).unwrap(), game)
}

#[test]
fn contiguous_native_nodes_decode_and_encode_once_per_tick_with_session_isolation() {
    CODECS.set((0, 0));
    let (mut first, mut game) = fixture(&["native", "native", "native"]);
    let (mut second, mut other) = fixture(&["native"]);
    first
        .step(&mut game, &GameInput::default(), 1. / 60., &mut vec![])
        .unwrap();
    assert_eq!(CODECS.get(), (1, 1));
    second
        .step(&mut other, &GameInput::default(), 1. / 60., &mut vec![])
        .unwrap();
    assert_eq!(other.module_data["counter"], json!([0]));
    first
        .step(&mut game, &GameInput::default(), 1. / 60., &mut vec![])
        .unwrap();
    assert_eq!(game.module_data["counter"], json!([0, 1, 2, 3, 4, 5]));
    assert_eq!(CODECS.get(), (3, 3));
}

#[test]
fn portable_interleaving_sees_native_edits_and_native_reload_sees_portable_edits() {
    CODECS.set((0, 0));
    let (mut executor, mut game) = fixture(&["native", "native", "portable", "native"]);
    executor
        .step(&mut game, &GameInput::default(), 1. / 60., &mut vec![])
        .unwrap();
    assert_eq!(game.module_data["counter"], json!([0, 1, 2, 3]));
    assert_eq!(CODECS.get(), (2, 2));
}

#[test]
fn corrupt_portable_edits_reject_at_reentry_and_at_final_validation() {
    for modules in [
        vec!["native", "native", "portable", "native"],
        vec!["native", "native", "portable"],
    ] {
        let (mut executor, mut game) = fixture(&modules);
        let before = game.clone();
        let mut events = vec![];
        let input = GameInput {
            actions: BTreeSet::from(["corrupt".into()]),
            ..Default::default()
        };
        assert!(
            executor
                .step(&mut game, &input, 1. / 60., &mut events)
                .is_err()
        );
        assert_eq!(game, before);
        assert!(events.is_empty());
        executor
            .step(&mut game, &GameInput::default(), 1. / 60., &mut events)
            .unwrap();
    }
}

#[test]
fn native_late_rejection_discards_typed_data_and_retry_matches_fresh_replay() {
    let (mut executor, mut game) = fixture(&["native", "native"]);
    let before = game.clone();
    let states = executor.states();
    let mut events = vec![];
    let input = GameInput {
        actions: BTreeSet::from(["reject".into()]),
        ..Default::default()
    };
    assert!(
        executor
            .step(&mut game, &input, 1. / 60., &mut events)
            .unwrap_err()
            .contains("late rejection")
    );
    assert_eq!(game, before);
    assert_eq!(executor.states(), states);
    assert!(events.is_empty());
    executor
        .step(&mut game, &GameInput::default(), 1. / 60., &mut events)
        .unwrap();
    let (mut oracle, mut expected) = fixture(&["native", "native"]);
    let mut expected_events = vec![];
    oracle
        .step(
            &mut expected,
            &GameInput::default(),
            1. / 60.,
            &mut expected_events,
        )
        .unwrap();
    assert_eq!(game, expected);
    assert_eq!(events, expected_events);
    assert_eq!(executor.states(), oracle.states());
}

struct PortableReplacement;
impl ExecutableSystem for PortableReplacement {
    fn initialize(&mut self, ctx: &mut SystemContext<'_>, _: &mut Value) -> Result<(), String> {
        assert_eq!(ctx.game.module_data["counter"], json!([0, 1, 2, 3]));
        assert!(ctx.with_tick_data::<Counter, _>(|_, _| Ok(())).is_none());
        ctx.game
            .module_data
            .get_mut("counter")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .push(json!(4));
        Ok(())
    }
    fn step(&mut self, ctx: &mut SystemContext<'_>, _: &mut Value) -> Result<(), String> {
        assert_eq!(ctx.game.module_data["counter"], json!([0, 1, 2, 3, 4]));
        Ok(())
    }
}

#[test]
fn replacement_initialization_sees_prior_native_flush_and_following_native_sees_its_edits() {
    let (mut executor, mut game) = fixture(&["native", "native", "native"]);
    executor
        .step(&mut game, &GameInput::default(), 1. / 60., &mut vec![])
        .unwrap();
    let mut replacement = SystemRegistry::default();
    replacement
        .register(
            SystemDescriptor {
                id: "native".into(),
                state_schema: "1".into(),
                components: vec![],
                ports: vec![],
                requires: vec![],
            },
            |_| Ok(Box::new(PortableReplacement)),
        )
        .unwrap();
    executor.replace("1", &replacement, None, false).unwrap();
    executor
        .step(&mut game, &GameInput::default(), 1. / 60., &mut vec![])
        .unwrap();
    assert_eq!(game.module_data["counter"], json!([0, 1, 2, 3, 4, 5]));
}
