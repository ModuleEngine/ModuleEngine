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
        features.contains(&"arpg")
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

fn replay_g1(host: &mut GameHost, segments: &[Segment], log: &mut impl Write) {
    let levels = level_catalog();
    assert_eq!(segments.len(), levels.len() + 1);
    assert_eq!(segments[0].name, "g1-menu");
    replay_segment(host, &segments[0], log);
    for (index, (segment, level)) in segments[1..].iter().zip(levels).enumerate() {
        assert_eq!(segment.name, level.id);
        let events = replay_segment(host, segment, log);
        let state = view(host.session.state()).unwrap();
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
}

#[cfg(feature = "verification")]
#[test]
fn r62_gameplay_collision_parity() {
    use module_engine_editor_composition::{
        arpg::{GameStateExt, GameStatus},
        two_arenas::{self, verification::progression_input},
    };
    use module_engine_physics_rapier::COLLIDER;

    let root = module_engine_vulkan::resource_root();
    let g1_project = crate::game_templates::g1_document();
    let g1_player = crate::runtime_delivery::prepare(&g1_project, &root).unwrap();
    let g1_features = crate::game_package::selected_features(&g1_player, &root).unwrap();
    assert!(!g1_features.contains("rapier-physics"));
    let g1_features: Vec<_> = g1_features.iter().map(String::as_str).collect();
    let mut g1 = host(&root, g1_player.clone(), &g1_features, false);
    let mut g1_installed = host(
        &root,
        g1_player,
        &[g1_features.as_slice(), &["rapier-physics"]].concat(),
        false,
    );
    let mut segments = input_tape();
    segments.pop();
    let mut g1_log = Vec::new();
    let mut g1_installed_log = Vec::new();
    replay_g1(&mut g1, &segments, &mut g1_log);
    replay_g1(&mut g1_installed, &segments, &mut g1_installed_log);
    assert_eq!(g1_log, g1_installed_log);

    let mut candidate = crate::game_templates::demo_document();
    candidate.custom_sources.clear();
    let game = candidate.game.as_mut().unwrap();
    assert_eq!(
        game.execution.stages.pop().unwrap().nodes[0].id,
        "game-controller"
    );
    for scene in game.scenes.values_mut() {
        for player in scene
            .objects
            .iter_mut()
            .filter(|object| object.components.contains_key("player"))
        {
            player
                .components
                .insert("health".into(), json!({"maximum":5000.,"current":5000.}));
        }
    }
    let mut reference = candidate.clone();
    reference.game.as_mut().unwrap().execution = two_arenas::demo_project().execution;
    let make_host = |mut document, physics| {
        crate::engine_profiles::finish_template(
            &mut document,
            crate::engine_profiles::ProfileKind::Arpg,
            &root,
        )
        .unwrap();
        let prepared = crate::runtime_delivery::prepare(&document, &root).unwrap();
        let features = crate::game_package::selected_features(&prepared, &root).unwrap();
        assert_eq!(features.contains("rapier-physics"), physics);
        let features: Vec<_> = features.iter().map(String::as_str).collect();
        host(&root, prepared, &features, false)
    };
    let mut legacy = make_host(reference.clone(), false);
    let mut rapier = make_host(candidate.clone(), true);
    assert_eq!(legacy.session.state().living_enemies(), 20);
    assert_eq!(rapier.session.state().living_enemies(), 20);

    // Diagonal input reaches the east wall, slides along it and reverses away.
    let mut wall_positions = Vec::new();
    for tick in 0..190 {
        let movement = match tick {
            0..=99 => [1., 0.],
            100..=169 => [1., -1.],
            _ => [-1., 0.],
        };
        let input = ArpgInput {
            movement,
            ..Default::default()
        }
        .into_input();
        legacy.step(1. / 60., &input).unwrap();
        rapier.step(1. / 60., &input).unwrap();
        let position = |host: &GameHost| {
            host.session
                .state()
                .player()
                .unwrap()
                .object
                .transform
                .translation
        };
        assert_eq!(
            position(&legacy),
            position(&rapier),
            "wall replay input {tick}"
        );
        wall_positions.push(position(&rapier));
    }
    assert!((7.3..=7.46).contains(&wall_positions[99][0]));
    assert_eq!(wall_positions[169][0], wall_positions[99][0]);
    assert!(wall_positions[169][2] < wall_positions[99][2] - 3.);
    assert!(wall_positions[189][0] < wall_positions[169][0] - 1.);

    // Alter only a physics collider so this control fails if both hosts use the planar fallback.
    let mut shifted = candidate;
    shifted
        .game
        .as_mut()
        .unwrap()
        .scenes
        .get_mut("arena")
        .unwrap()
        .objects
        .iter_mut()
        .find(|object| object.object.id == 1002)
        .unwrap()
        .components
        .get_mut(COLLIDER)
        .unwrap()["offset"] = json!([-2., 0., 0.]);
    let mut shifted = make_host(shifted, true);
    let east = ArpgInput {
        movement: [1., 0.],
        ..Default::default()
    }
    .into_input();
    for _ in 0..100 {
        shifted.step(1. / 60., &east).unwrap();
    }
    let shifted_x = shifted
        .session
        .state()
        .player()
        .unwrap()
        .object
        .transform
        .translation[0];
    assert!(
        (5.3..=5.46).contains(&shifted_x),
        "shifted wall stopped player at x={shifted_x}"
    );

    legacy.session.restart().unwrap();
    rapier.session.restart().unwrap();
    let mut events = std::collections::BTreeMap::new();
    let mut boss_warning = false;
    for index in 0..9000 {
        let input = progression_input(legacy.session.state(), index).unwrap();
        assert_eq!(legacy.step(1. / 60., &input).unwrap(), 1);
        assert_eq!(rapier.step(1. / 60., &input).unwrap(), 1);
        let before = legacy.session.state();
        let after = rapier.session.state();
        assert_eq!(&before.scene, &after.scene, "progression input {index}");
        assert_eq!(
            legacy.session.snapshot(),
            rapier.session.snapshot(),
            "progression input {index}"
        );
        assert_eq!(before.arpg(), after.arpg(), "progression input {index}");
        assert_eq!(
            legacy.session.events(),
            rapier.session.events(),
            "progression input {index}"
        );
        boss_warning |= after.scene == "boss" && !after.arpg().telegraphs.is_empty();
        for event in rapier.session.events() {
            *events.entry(event.name.clone()).or_insert(0) += 1;
        }
        if after.arpg().status == GameStatus::Victory {
            break;
        }
    }
    assert_eq!(rapier.session.state().arpg().status, GameStatus::Victory);
    assert_eq!(rapier.session.state().scene, "boss");
    assert!(boss_warning);
    assert!(
        ["Attack", "Hit", "Death", "SceneChanged", "Victory", "Sound"]
            .iter()
            .all(|event| events.contains_key(*event))
    );
    eprintln!(
        "R6.2: 10 Crate Garden levels, wall x={}, shifted x={}, ARPG ticks={}, events={events:?}",
        wall_positions[99][0],
        shifted_x,
        rapier.session.state().tick
    );
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
    replay_g1(&mut g1, &segments[..segments.len() - 1], &mut log);
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
