//! Fixed input replay through the shared GameHost for R1.5.
use module_engine_core::{GameInput, GraphDocument};
use module_engine_editor_composition::arpg::ArpgInput;
use module_engine_editor_composition::g1::{
    Direction, Phase, action_input, direction_input, level_catalog, view,
};
use module_engine_host::{GameHost, HostOptions, RenderMode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path};

#[derive(Deserialize, Serialize)]
struct Segment {
    name: String,
    inputs: Vec<GameInput>,
}

fn input_tape() -> Vec<Segment> {
    let mut segments = vec![Segment {
        name: "g1-menu".into(),
        inputs: vec![GameInput::default(), action_input("g1.play")],
    }];
    for (index, level) in level_catalog().iter().enumerate() {
        let mut inputs = Vec::new();
        if index > 0 {
            inputs.push(action_input("g1.next"));
        }
        inputs.extend(level.solution.chars().map(|token| {
            direction_input(match token {
                'N' => Direction::North,
                'E' => Direction::East,
                'S' => Direction::South,
                'W' => Direction::West,
                _ => panic!("Unknown witness direction {token}"),
            })
        }));
        segments.push(Segment {
            name: level.id.into(),
            inputs,
        });
    }
    segments.push(Segment {
        name: "arpg-small-monsters".into(),
        inputs: (0..180)
            .map(|tick| {
                ArpgInput {
                    movement: if tick < 32 { [0., -1.] } else { [0., 0.] },
                    aim_world: [0., 0., -10.],
                    attack: (tick + 12) % 22 == 0,
                    ..ArpgInput::default()
                }
                .into_input()
            })
            .collect(),
    });
    segments
}

fn host(root: &Path, document: GraphDocument, features: &[&str], custom: bool) -> GameHost {
    let composition = module_engine_player::composition_for_features(
        root,
        &features.iter().map(|&feature| feature.into()).collect(),
    )
    .unwrap();
    assert_eq!(
        composition.systems.descriptor("game.combat").is_some(),
        custom
    );
    let world_node = composition
        .modules
        .render_route(&document)
        .unwrap()
        .world
        .id
        .clone();
    GameHost::build(
        document,
        composition,
        HostOptions {
            asset_root: root.into(),
            backend_root: Some(root.into()),
            compile_root: custom.then(|| root.into()),
            render: RenderMode::Headless { world_node },
            services: Default::default(),
            services_enabled: false,
        },
        None,
    )
    .unwrap()
}

fn replay_segment(host: &mut GameHost, segment: &Segment, log: &mut impl Write) -> Vec<Value> {
    let mut events = vec![];
    for input in &segment.inputs {
        let previous = host.session.state().tick;
        assert_eq!(host.step(1. / 60., input).unwrap(), 1);
        let state = host.session.state();
        assert_eq!(state.tick, previous + 1);
        let ordered: Vec<_> = host
            .session
            .events()
            .iter()
            .map(|event| json!({"type": event.name, "data": event.payload}))
            .collect();
        serde_json::to_writer(&mut *log, &json!({
            "segment": segment.name, "scene": state.scene, "tick": state.tick, "events": ordered,
        })).unwrap();
        writeln!(log).unwrap();
        events.extend(ordered);
    }
    events
}

#[test]
#[ignore = "explicit R1.5 replay with the checked-in ARPG project's compiled Custom worker"]
fn r15_schedule_event_replay() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let output = std::env::var_os("MODULE_ENGINE_R15_RUN").expect("Set MODULE_ENGINE_R15_RUN");
    let output = Path::new(&output);
    fs::create_dir(output).unwrap();
    let tape = std::env::var_os("MODULE_ENGINE_R15_INPUT").map_or_else(
        || serde_json::to_vec(&input_tape()).unwrap(),
        |path| fs::read(path).unwrap(),
    );
    fs::write(output.join("input.json"), &tape).unwrap();
    let segments: Vec<Segment> = serde_json::from_slice(&tape).unwrap();
    let mut log = std::io::BufWriter::new(fs::File::create(output.join("events.ndjson")).unwrap());
    let mut g1 = host(
        &root,
        crate::game_templates::g1_document(),
        &["g1", "ui", "vulkan", "audio", "persistence"],
        false,
    );
    let levels = level_catalog();
    assert_eq!(segments.len(), levels.len() + 2);
    assert_eq!(segments[0].name, "g1-menu");
    replay_segment(&mut g1, &segments[0], &mut log);
    for (index, (segment, level)) in segments[1..].iter().zip(levels).enumerate() {
        assert_eq!(segment.name, level.id);
        let events = replay_segment(&mut g1, segment, &mut log);
        let state = view(g1.session.state()).unwrap();
        assert_eq!(state.current_level, index);
        assert_eq!(
            state.phase,
            if index + 1 == levels.len() {
                Phase::Complete
            } else {
                Phase::Victory
            }
        );
        assert_eq!(state.moves as usize, level.solution.len());
        assert_eq!(
            events
                .iter()
                .filter(|event| event["type"] == "g1.moved")
                .count(),
            level.solution.len()
        );
    }
    let source = fs::read(root.join("game-project.json")).unwrap();
    let migrated = crate::engine_profiles::migration_candidate(
        &source,
        crate::engine_profiles::ProfileKind::Arpg,
        &root,
    )
    .unwrap();
    let mut arpg = host(
        &root,
        migrated.document,
        &["arpg", "ui", "vulkan", "audio", "persistence"],
        true,
    );
    assert_eq!(arpg.session.state().scene, "arena");
    let combat = segments.last().unwrap();
    assert_eq!(combat.name, "arpg-small-monsters");
    let events = replay_segment(&mut arpg, combat, &mut log);
    log.flush().unwrap();
    assert!(
        events
            .iter()
            .any(|event| event["type"] == "Hit" && event["data"]["attacker"] == 1)
    );
    assert!(
        events
            .iter()
            .any(|event| event["type"] == "Hit" && event["data"]["target"] == 1)
    );
    assert!(
        events
            .iter()
            .any(|event| event["type"] == "Death" && event["data"]["object"] != 1)
    );
    assert_eq!(fs::read(root.join("game-project.json")).unwrap(), source);
}
