mod bevy_store;
#[path = "../../../examples/bench/s3_ecs.rs"]
mod s3;

use bevy_store::{BevyStore, Encoding};
use module_engine_core::{EcsWorld, GameEntity, WorldCommand};
use serde_json::{Value, json};
use std::{hint::black_box, time::Instant};

const TICK_WARMUP: usize = 30;
const TICK_SAMPLES: usize = 240;
const BATCH_WARMUP: usize = 5;
const BATCH_SAMPLES: usize = 30;

fn fixture() -> Vec<GameEntity> {
    (0..s3::ENTITIES)
        .map(|i| s3::entity(s3::object(i)))
        .collect()
}

fn measure<T>(run: impl FnOnce() -> T) -> (T, f64) {
    let start = Instant::now();
    let output = black_box(run());
    (output, start.elapsed().as_secs_f64() * 1_000.)
}

fn measure_ticks<T>(mut tick: impl FnMut(usize) -> T) -> Vec<f64> {
    for frame in 0..TICK_WARMUP {
        black_box(tick(frame));
    }
    (TICK_WARMUP..TICK_WARMUP + TICK_SAMPLES)
        .map(|frame| measure(|| tick(frame)).1)
        .collect()
}

trait BatchStore: Sized {
    fn empty(encoding: Encoding) -> Self;
    fn restore(encoding: Encoding, snapshot: Vec<GameEntity>) -> Self;
    fn spawn(&mut self, commands: &[WorldCommand], entities: Vec<GameEntity>);
    fn despawn(&mut self, commands: &[WorldCommand]);
    fn snapshot(&self) -> Vec<GameEntity>;
}

impl BatchStore for EcsWorld {
    fn empty(_: Encoding) -> Self {
        EcsWorld::from_entities(Vec::new()).unwrap()
    }

    fn restore(_: Encoding, snapshot: Vec<GameEntity>) -> Self {
        EcsWorld::from_entities(snapshot).unwrap()
    }

    fn spawn(&mut self, commands: &[WorldCommand], entities: Vec<GameEntity>) {
        self.apply_commands(commands).unwrap();
        for entity in entities {
            let id = entity.object.id;
            *self.references_mut(id).unwrap() = entity.references;
            for (key, value) in entity.components {
                self.insert_component(id, key, value).unwrap();
            }
        }
        self.validate_references().unwrap();
    }

    fn despawn(&mut self, commands: &[WorldCommand]) {
        self.apply_commands(commands).unwrap();
        self.validate_references().unwrap();
    }

    fn snapshot(&self) -> Vec<GameEntity> {
        self.entities_snapshot()
    }
}

impl BatchStore for BevyStore {
    fn empty(encoding: Encoding) -> Self {
        BevyStore::new(encoding)
    }

    fn restore(encoding: Encoding, snapshot: Vec<GameEntity>) -> Self {
        BevyStore::restore(encoding, snapshot).unwrap()
    }

    fn spawn(&mut self, commands: &[WorldCommand], entities: Vec<GameEntity>) {
        self.spawn_s3(commands, entities);
    }

    fn despawn(&mut self, commands: &[WorldCommand]) {
        self.despawn_s3(commands);
    }

    fn snapshot(&self) -> Vec<GameEntity> {
        BevyStore::snapshot(self)
    }
}

fn measure_batches<W: BatchStore>(seed: &[GameEntity], encoding: Encoding) -> Value {
    let spawn: Vec<_> = seed
        .iter()
        .map(|e| WorldCommand::Spawn {
            object: e.object.clone(),
        })
        .collect();
    let despawn: Vec<_> = seed
        .iter()
        .map(|e| WorldCommand::Remove { id: e.object.id })
        .collect();
    let mut timings = [const { Vec::new() }; 4];
    for trial in 0..BATCH_WARMUP + BATCH_SAMPLES {
        let mut world = W::empty(encoding);
        let input = seed.to_vec();
        let (_, spawn_ms) = measure(|| world.spawn(&spawn, input));
        let (snapshot, snapshot_ms) = measure(|| world.snapshot());
        let (restored, restore_ms) = measure(|| W::restore(encoding, snapshot));
        let (_, despawn_ms) = measure(|| world.despawn(&despawn));

        assert_eq!(restored.snapshot(), seed);
        assert!(world.snapshot().is_empty());
        if trial >= BATCH_WARMUP {
            for (samples, duration) in
                timings
                    .iter_mut()
                    .zip([spawn_ms, despawn_ms, snapshot_ms, restore_ms])
            {
                samples.push(duration);
            }
        }
    }
    let [spawn, despawn, snapshot, restore] = timings;
    json!({"spawn": spawn, "despawn": despawn, "snapshot": snapshot, "restore": restore})
}

fn main() -> Result<(), String> {
    let variant = std::env::args().nth(1).unwrap_or_default();
    let seed = fixture();
    let (ticks, batches) = match variant.as_str() {
        "core" => {
            let mut world = EcsWorld::from_entities(seed.clone())?;
            (
                measure_ticks(|frame| s3::tick(&mut world, frame, None)),
                measure_batches::<EcsWorld>(&seed, Encoding::Dynamic),
            )
        }
        "dynamic" | "hybrid" => {
            let encoding = if variant == "dynamic" {
                Encoding::Dynamic
            } else {
                Encoding::Hybrid
            };
            let mut world = BevyStore::restore(encoding, seed.clone())?;
            (
                measure_ticks(|frame| world.tick(frame)),
                measure_batches::<BevyStore>(&seed, encoding),
            )
        }
        _ => return Err("Usage: module-engine-ecs-spike <core|dynamic|hybrid>".into()),
    };
    println!(
        "{}",
        json!({
            "variant": variant, "entities": seed.len(), "components_per_entity": s3::COMPONENTS.len(),
            "moved_per_tick": s3::MOVING, "tick_warmup": TICK_WARMUP, "batch_warmup": BATCH_WARMUP,
            "tick_ms": ticks, "batch_ms": batches,
        })
    );
    Ok(())
}
