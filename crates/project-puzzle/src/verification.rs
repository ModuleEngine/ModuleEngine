//! Game-owned Puzzle composition and hosted runtime acceptance scenarios.
use crate::{Direction, PuzzleOutcome, direction_input, frame};
use module_engine_core::{GameInput, GameSession, GameState, GraphDocument, WorldKind};
use module_engine_host::GameHost;
use serde_json::{Value, json};
use std::{fs, path::Path};

/// Services provided by the shell for the game-owned scenario.
pub struct Environment {
    pub build: fn(GraphDocument, &Path, &Path, WorldKind) -> Result<GameHost, String>,
    pub write_png: fn(&Path, &module_engine_core::Frame) -> Result<(), String>,
}

const DT: f64 = 1. / 60.;
const ARPG_SYSTEMS: [&str; 8] = [
    "game.input",
    "game.ai",
    "game.movement",
    "game.collision",
    "game.combat",
    "game.health",
    "game.portals",
    "game.audio",
];
pub const SOLUTION: [Direction; 5] = [
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::East,
    Direction::North,
];

pub fn require(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

pub fn board(document: &GraphDocument) -> Result<&Value, String> {
    document
        .game
        .as_ref()
        .and_then(|game| game.scenes.get("board"))
        .and_then(|scene| scene.objects.iter().find(|object| object.object.id == 1))
        .and_then(|object| object.components.get("puzzle.board"))
        .ok_or_else(|| "Puzzle board component is missing".into())
}

pub fn independent_activity(
    document: &GraphDocument,
    composition: &module_engine_core::CompositionContext,
) -> Result<Value, String> {
    let activity = composition.activity(document)?;
    require(
        activity.components.contains("puzzle.board") && activity.components.contains("puzzle.cell"),
        "Puzzle schemas are absent from derived component activity",
    )?;
    require(
        activity.components.iter().all(|id| {
            matches!(
                id.as_str(),
                "puzzle.board" | "puzzle.cell" | "mesh" | "material"
            )
        }) && ARPG_SYSTEMS
            .iter()
            .all(|id| !activity.modules.contains(*id)),
        "Puzzle composition activated a genre-specific gameplay contract",
    )?;
    let game = document.game.as_ref().ok_or("Puzzle game is missing")?;
    require(
        game.execution
            .stages
            .iter()
            .flat_map(|stage| &stage.nodes)
            .map(|node| node.module.as_str())
            .eq(["puzzle.turn", "puzzle.result"]),
        "Fresh Puzzle fixture has an unexpected execution schedule",
    )?;
    require(
        game.audio_cues.is_empty() && document.custom_sources.is_empty(),
        "Puzzle preset contains audio cues or Custom source instances",
    )?;
    require(
        game.scenes
            .values()
            .flat_map(|scene| &scene.objects)
            .all(|object| {
                object.components.keys().all(|id| {
                    matches!(
                        id.as_str(),
                        "puzzle.board" | "puzzle.cell" | "mesh" | "material"
                    )
                })
            }),
        "Puzzle authored data contains unrelated gameplay components",
    )?;
    Ok(
        json!({"modules":activity.modules,"components":activity.components,"audio_cues_empty":true,"custom_sources_empty":true}),
    )
}

pub fn outcome(session: &GameSession, expected: PuzzleOutcome, turns: u64) -> Result<(), String> {
    let actual = frame(session.state())?;
    require(
        actual.outcome == expected && actual.turns == turns,
        &format!("Unexpected puzzle frame: {actual:?}; expected {expected:?}, turns={turns}"),
    )
}

pub fn on_goal(state: &GameState) -> bool {
    let crate_cell = state.world.component(101, "puzzle.cell");
    let goal = state.world.component(102, "puzzle.cell");
    matches!((crate_cell, goal), (Some(left), Some(right)) if left["x"] == right["x"] && left["z"] == right["z"])
}

fn render_evidence(
    host: &mut GameHost,
    folder: &Path,
    name: &str,
    environment: &Environment,
) -> Result<module_engine_core::Frame, String> {
    let rendered = host.render(960, 540)?;
    require(
        rendered.stats.shaded_pixels > 0 && rendered.stats.visible_objects > 0,
        "Puzzle 3D rendering produced no visible geometry",
    )?;
    (environment.write_png)(&folder.join(format!("{name}.png")), &rendered)?;
    Ok(rendered)
}

pub fn hosted_workflow(
    document: &GraphDocument,
    asset_root: &Path,
    backend: &Path,
    kind: WorldKind,
    folder: &Path,
    label: &str,
    environment: &Environment,
) -> Result<(Value, GameState, Vec<u8>), String> {
    let mut host = (environment.build)(document.clone(), asset_root, backend, kind)?;
    host.set_detailed_stats(true);
    require(host.slots.is_empty(), "Puzzle initialized a Custom worker")?;
    require(
        host.presentation_id() == Some("puzzle.presentation"),
        "Puzzle presentation contribution was not selected",
    )?;
    host.step(DT, &GameInput::default())?;
    outcome(&host.session, PuzzleOutcome::Playing, 0)?;
    let initial = host.session.state().clone();
    let cell_size = board(document)?["cell_size"]
        .as_f64()
        .ok_or("Missing edited cell_size")? as f32;
    let pawn = initial
        .world
        .object(100)
        .ok_or("Initialized puzzle pawn is missing")?;
    require(
        (pawn.transform.translation[2] - cell_size).abs() < 0.00001,
        "Inspector edit did not affect the hosted source system's grid projection",
    )?;
    let initial_frame =
        render_evidence(&mut host, folder, &format!("{label}-initial"), environment)?;
    host.step(DT, &direction_input(Direction::West))?;
    outcome(&host.session, PuzzleOutcome::Playing, 0)?;
    require(
        host.session.state().world == initial.world
            && host
                .session
                .events()
                .iter()
                .any(|event| event.name == "puzzle.blocked"),
        "A blocked move changed the board or failed to emit its module event",
    )?;
    for direction in SOLUTION {
        host.step(DT, &direction_input(direction))?;
    }
    outcome(&host.session, PuzzleOutcome::Solved, 5)?;
    require(
        host.session.state().control.halted
            && on_goal(host.session.state())
            && host
                .session
                .events()
                .iter()
                .any(|event| event.name == "puzzle.solved"),
        "Solution did not halt with the crate on its goal and a solved event",
    )?;
    let solved = host.session.state().clone();
    let solved_frame = render_evidence(&mut host, folder, &format!("{label}-solved"), environment)?;
    require(
        initial_frame.rgba != solved_frame.rgba,
        "Moving puzzle geometry did not change the rendered frame",
    )?;
    require(
        host.step(DT, &direction_input(Direction::South))? == 0 && host.session.state() == &solved,
        "Solved game consumed another turn",
    )?;
    let restart = GameInput {
        restart: true,
        ..Default::default()
    };
    host.step(DT, &restart)?;
    host.step(DT, &GameInput::default())?;
    outcome(&host.session, PuzzleOutcome::Playing, 0)?;
    require(
        host.session.state() == &initial,
        "Restart did not restore the authored board and module frame",
    )?;
    let restarted = render_evidence(
        &mut host,
        folder,
        &format!("{label}-restarted"),
        environment,
    )?;
    require(
        restarted.rgba == initial_frame.rgba,
        "Restarted board rendered differently from its initial state",
    )?;
    for direction in [Direction::North, Direction::South]
        .into_iter()
        .cycle()
        .take(8)
    {
        host.step(DT, &direction_input(direction))?;
    }
    outcome(&host.session, PuzzleOutcome::Failed, 8)?;
    require(
        host.session.state().control.halted
            && host
                .session
                .events()
                .iter()
                .any(|event| event.name == "puzzle.failed"),
        "Turn limit did not halt and emit failure",
    )?;
    let failed = host.session.state().clone();
    require(
        host.step(DT, &direction_input(Direction::East))? == 0 && host.session.state() == &failed,
        "Failed game consumed another turn",
    )?;
    host.step(DT, &restart)?;
    host.step(DT, &GameInput::default())?;
    require(
        host.session.state() == &initial,
        "Restart after failure did not restore the project",
    )?;
    require(
        host.accepted_document().game == document.game
            && !host
                .session
                .state()
                .module_data
                .keys()
                .any(|key| key.starts_with("arpg.")),
        "Runtime changed the accepted authored game or introduced ARPG frame data",
    )?;
    fs::write(
        folder.join(format!("{label}-solved-state.json")),
        serde_json::to_vec_pretty(&solved).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok((
        json!({"world":format!("{kind:?}"),"presentation":host.presentation_id(),"initial_render":initial_frame.stats,"solved_render":solved_frame.stats,"edited_cell_size_applied_to_world":true,"blocked_move_preserved_turns":true,"solution_turns":5,"turn_limit_failure_turns":8,"halted_input_ignored":true,"restart_after_solve_and_failure":true,"authored_game_unchanged":true,"custom_workers":0}),
        solved,
        solved_frame.rgba,
    ))
}
