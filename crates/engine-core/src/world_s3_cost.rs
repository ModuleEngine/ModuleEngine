//! Release-only measurements of the operations used by the S3 motion loop.

use super::{EcsWorld, Transform};
use serde_json::{Value, json};
use std::{hint::black_box, sync::Arc, time::Instant};

// The SDK source audit visits this file independently of its parent module.
#[cfg(test)]
#[path = "../../../examples/bench/s3_ecs.rs"]
mod s3;

const WARMUP: usize = 30;
const SAMPLES: usize = 240;

fn measure(operation: &str, mut batch: impl FnMut(usize)) {
    for frame in 0..WARMUP {
        batch(frame);
    }
    let samples_us: Vec<_> = (WARMUP..WARMUP + SAMPLES)
        .map(|frame| {
            let start = Instant::now();
            batch(frame);
            start.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    let mut sorted = samples_us.clone();
    sorted.sort_by(f64::total_cmp);
    println!(
        "{}",
        json!({
            "operation": operation,
            "updates_per_batch": s3::MOVING,
            "warmup_batches": WARMUP,
            "mean_us": samples_us.iter().sum::<f64>() / SAMPLES as f64,
            "p95_us": sorted[(95 * SAMPLES).div_ceil(100) - 1],
            "max_us": sorted[SAMPLES - 1],
            "samples_us": samples_us,
        })
    );
}

#[test]
#[ignore = "Release timing probe, run without competing tests or builds"]
fn s3_motion_update_cost() {
    let mut world =
        EcsWorld::from_entities((0..s3::ENTITIES).map(s3::object).map(s3::entity).collect())
            .unwrap();
    measure("fixture_tick", |frame| {
        black_box(s3::tick(&mut world, frame, None));
    });
    let after_fixture = world.clone();

    let entities = world.entities_with(&s3::COMPONENTS);
    assert_eq!(entities.len(), s3::ENTITIES);
    let ids = &entities[..s3::MOVING];
    let name = s3::COMPONENTS[0];
    measure("individual_motion_updates", |frame| {
        for &id in black_box(ids) {
            let motion = world.component_mut(id, name).unwrap();
            let offset = s3::offset(s3::time(frame), id);
            motion["offset"] = json!(offset);
            world.transform_mut(id).unwrap().translation[1] = offset;
        }
    });
    assert!(
        world == after_fixture,
        "S3 batch changed the final motion values"
    );

    // Isolated operations use the same IDs, handles and JSON layout. Their
    // warm caches and black-box overhead differ from the mixed loop above.
    let handles: Vec<_> = ids.iter().map(|id| world.rows.index[id]).collect();
    let offsets: Vec<_> = ids
        .iter()
        .map(|&id| s3::offset(s3::time(WARMUP), id))
        .collect();
    let mut payloads: Vec<_> = ids
        .iter()
        .map(|&id| world.component(id, name).unwrap().clone())
        .collect();

    measure("arc_uniqueness_and_two_mutable_borrows", |_| {
        for _ in black_box(ids) {
            black_box(Arc::strong_count(&world.rows));
            black_box(Arc::make_mut(&mut world.rows));
            black_box(Arc::make_mut(&mut world.rows));
        }
    });

    let rows = Arc::get_mut(&mut world.rows).expect("standalone S3 owns its rows");
    measure("stable_id_lookup", |_| {
        for id in black_box(ids) {
            black_box(rows.index[id]);
        }
    });
    measure("bevy_transform_lookup_and_write", |_| {
        for (&handle, &offset) in black_box(&handles).iter().zip(&offsets) {
            rows.world
                .get_mut::<Transform>(handle)
                .unwrap()
                .into_inner()
                .translation[1] = black_box(offset);
        }
    });
    measure(
        "non_render_binding_check_and_transform_write_counter",
        |_| {
            for &id in black_box(ids) {
                rows.mark_binding_changed(id, black_box(name));
                *black_box(&mut rows.render_writes) += 1;
            }
        },
    );
    measure("json_column_and_row_lookup", |_| {
        for id in black_box(ids) {
            let column = rows.components.get_mut(black_box(name)).unwrap();
            black_box(column.get_mut(id).unwrap());
        }
    });
    measure("json_number_and_payload_key", |_| {
        for (motion, &offset) in black_box(&mut payloads).iter_mut().zip(&offsets) {
            motion["offset"] = json!(black_box(offset));
        }
    });
    measure("json_number_and_existing_payload_key", |_| {
        for (motion, &offset) in black_box(&mut payloads).iter_mut().zip(&offsets) {
            *motion.get_mut("offset").unwrap() = Value::from(black_box(offset));
        }
    });
}
