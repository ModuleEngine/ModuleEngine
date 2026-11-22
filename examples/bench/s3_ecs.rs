//! S3 inputs and the Core tick shared by the native benchmark and R1.1.
use module_engine_core::{EcsWorld, GameEntity, Transform, WorldObject};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub const STATIC: usize = 10_000;
pub const MOVING: usize = 1_000;
pub const ENTITIES: usize = 2_000;
pub const COMPONENTS: [&str; 8] = [
    "bench.motion",
    "bench.health",
    "bench.stamina",
    "bench.team",
    "bench.controller",
    "bench.collision",
    "bench.target",
    "bench.flags",
];

pub fn object(i: usize) -> WorldObject {
    // Permute cells so the moving objects are spread throughout the field.
    let cell = i * 73 % (STATIC + MOVING);
    WorldObject {
        id: (i + 1) as u64,
        parent: None,
        transform: Transform {
            translation: [
                (cell % 110) as f32 * 1.6 - 87.2,
                0.,
                (cell / 110) as f32 * 1.6 - 79.2,
            ],
            rotation: [0., (i % 19) as f32 * 0.13, 0.],
            scale: [0.62; 3],
        },
        color: [255; 4],
    }
}

pub fn entity(object: WorldObject) -> GameEntity {
    let i = object.id as usize - 1;
    GameEntity {
        object,
        components: BTreeMap::from([
            (
                COMPONENTS[0].into(),
                json!({"moving":i < MOVING,"offset":0.}),
            ),
            (COMPONENTS[1].into(), json!({"current":100,"maximum":100})),
            (COMPONENTS[2].into(), json!({"current":50,"maximum":50})),
            (COMPONENTS[3].into(), json!(i % 8)),
            (COMPONENTS[4].into(), json!({"enabled":true})),
            (COMPONENTS[5].into(), json!({"radius":0.62})),
            (COMPONENTS[6].into(), Value::Null),
            (COMPONENTS[7].into(), json!({"visible":true})),
        ]),
        references: BTreeMap::new(),
    }
}

pub fn time(frame: usize) -> f32 {
    (frame + 1) as f32 / 60.
}

pub fn offset(time: f32, id: u64) -> f32 {
    (time * 1.5 + id as f32 * 0.03125).sin() * 0.35
}

pub fn tick(world: &mut EcsWorld, frame: usize) -> Vec<u64> {
    let entities = world.entities_with(&COMPONENTS);
    let time = time(frame);
    for &id in &entities[..MOVING] {
        let motion = world.component_mut(id, COMPONENTS[0]).unwrap();
        let offset = offset(time, id);
        motion["offset"] = json!(offset);
        world.transform_mut(id).unwrap().translation[1] = offset;
    }
    entities
}
