use super::*;
use bevy_ecs::prelude::Resource;
use bevy_reflect::PartialReflect;
use module_engine_core::{AuthoredObject, AuthoredScene, EcsWorld, GameProject, Prefab};

const ENCODINGS: [Encoding; 2] = [Encoding::Dynamic, Encoding::Hybrid];

#[test]
fn s3_tick_matches_core_and_respects_membership_and_stable_order() {
    let mut seed = crate::fixture();
    let mut baseline = EcsWorld::from_entities(seed.clone()).unwrap();
    let mut stores = ENCODINGS.map(|mode| BevyStore::restore(mode, seed.clone()).unwrap());
    for frame in 0..crate::TICK_WARMUP + crate::TICK_SAMPLES {
        let expected = s3::tick(&mut baseline, frame);
        assert_eq!(expected.len(), 2_000);
        for store in &mut stores {
            let result = store.tick(frame);
            assert!(
                result
                    .iter()
                    .map(|(id, _)| *id)
                    .eq(expected.iter().copied())
            );
            if [0, 29, 30, 269].contains(&frame) {
                assert_eq!(store.snapshot(), baseline.entities_snapshot());
            }
        }
    }

    // Force the first selected ID and the 1,000th write to change after a missing component.
    seed[0].components.remove("bench.flags");
    let mut baseline = EcsWorld::from_entities(seed.clone()).unwrap();
    let expected = s3::tick(&mut baseline, 0);
    assert_eq!(expected.len(), 1_999);
    assert_eq!(expected[0], 2);
    assert_eq!(expected[s3::MOVING - 1], 1_001);
    seed.rotate_left(671);
    for mode in ENCODINGS {
        let mut store = BevyStore::restore(mode, seed.clone()).unwrap();
        assert!(
            store
                .tick(0)
                .into_iter()
                .map(|(id, _)| id)
                .eq(expected.iter().copied())
        );
        assert_eq!(store.snapshot(), baseline.entities_snapshot());
    }
}

const PARENT: u64 = 9_007_199_254_740_993;

fn authored_example() -> GameProject {
    let parent = AuthoredObject::new(WorldObject {
        id: PARENT,
        ..s3::object(0)
    });
    let mut child = AuthoredObject::new(WorldObject {
        parent: Some(PARENT),
        ..s3::object(1)
    });
    child.prefab = Some("template".into());
    child
        .overrides
        .insert("source.Widget.Exact-Key".into(), Value::Null);
    child.references.insert("Target.ID".into(), PARENT);
    child.components.insert(
        "bench.motion".into(),
        json!({"moving":true,"offset":0,"Future-Key":[null,{"UP.Per":17}]}),
    );
    let prefab = Prefab {
        color: [42, 116, 211, 255],
        components: BTreeMap::from([
            (
                "source.Widget".into(),
                json!({"Exact-Key":7,"future.field":{"KeepMe":[null,1]}}),
            ),
            ("source.Widget.Other".into(), Value::Null),
        ]),
    };
    GameProject {
        player_preferences: None,
        entry_scene: "example".into(),
        scene_assets: BTreeMap::new(),
        scenes: BTreeMap::from([(
            "example".into(),
            AuthoredScene {
                name: "Example".into(),
                objects: vec![parent, child],
            },
        )]),
        prefabs: BTreeMap::from([("template".into(), prefab)]),
        execution: Default::default(),
        audio_cues: BTreeMap::new(),
    }
}

fn save(entities: Vec<GameEntity>) -> Vec<u8> {
    let map: BTreeMap<_, _> = entities.into_iter().map(|e| (e.object.id, e)).collect();
    serde_json::to_vec(&map).unwrap()
}

#[test]
fn schema_json_prefab_references_and_save_bytes_survive_round_trip() {
    let project = authored_example();
    let authored_bytes = serde_json::to_vec(&project).unwrap();
    let mut resolved = project.resolve_scene("example").unwrap();
    let parent = resolved.iter_mut().find(|e| e.object.id == PARENT).unwrap();
    parent
        .components
        .insert("bench.motion".into(), json!({"offset":-0.0,"moving":false}));
    let baseline = EcsWorld::from_entities(resolved.clone()).unwrap();
    let expected = save(baseline.entities_snapshot());

    for mode in ENCODINGS {
        let world = BevyStore::restore(mode, resolved.clone()).unwrap();
        let entities = world.snapshot();
        let [child, parent] = entities.as_slice() else {
            panic!("expected two objects")
        };
        assert_eq!(parent.object.id, PARENT);
        assert_eq!(child.object.parent, Some(PARENT));
        assert_eq!(child.references["Target.ID"], PARENT);
        assert_eq!(
            child.components["source.Widget"],
            json!({"Exact-Key":null,"future.field":{"KeepMe":[null,1]}})
        );
        assert_eq!(
            child.components.get("source.Widget.Other"),
            Some(&Value::Null)
        );
        assert!(!parent.components.contains_key("source.Widget.Other"));
        assert_eq!(
            child.components["bench.motion"]["Future-Key"],
            json!([null, {"UP.Per":17}])
        );
        assert_eq!(child.components["bench.motion"]["offset"].as_u64(), Some(0));
        assert!(
            parent.components["bench.motion"]["offset"]
                .as_f64()
                .unwrap()
                .is_sign_negative()
        );
        if matches!(mode, Encoding::Hybrid) {
            let handle = world.index[&PARENT];
            let Motion::Native(native) = world.world.get::<Motion>(handle).unwrap() else {
                panic!("canonical motion must be typed");
            };
            let registry = world.world.resource::<AppTypeRegistry>().read();
            assert!(
                registry
                    .get(std::any::TypeId::of::<NativeMotion>())
                    .is_some()
            );
            let bevy_reflect::ReflectRef::Struct(data) = native.reflect_ref() else {
                panic!("expected reflected fields")
            };
            assert_eq!(
                data.field("moving").unwrap().try_downcast_ref::<bool>(),
                Some(&false)
            );
        }
        let first = save(entities);
        assert_eq!(first, expected);
        let loaded: BTreeMap<u64, GameEntity> = serde_json::from_slice(&first).unwrap();
        let rebuilt = BevyStore::restore(mode, loaded.into_values().collect()).unwrap();
        assert_eq!(first, save(rebuilt.snapshot()));
        assert_eq!(serde_json::to_vec(&project).unwrap(), authored_bytes);
    }
}

#[derive(Resource, Clone, Debug, PartialEq)]
struct TickState {
    frame: u64,
    resources: Value,
    events: Vec<Value>,
}

fn publish(last_good: &mut BevyStore, candidate: BevyStore) -> Result<(), String> {
    candidate.validate_references()?;
    *last_good = candidate;
    Ok(())
}

#[test]
fn rejected_candidate_leaves_world_handles_and_tick_state_at_last_good_version() {
    for mode in ENCODINGS {
        let seed = authored_example().resolve_scene("example").unwrap();
        let mut last_good = BevyStore::restore(mode, seed).unwrap();
        let state = TickState {
            frame: 17,
            resources: json!({"score":3,"opaque":null}),
            events: vec![json!({"id": PARENT})],
        };
        last_good.world.insert_resource(state.clone());
        let before_handles = last_good.index.clone();
        let before_entities = save(last_good.snapshot());
        let mut rejected = BevyStore::restore(mode, last_good.snapshot()).unwrap();
        rejected.world.insert_resource(state.clone());

        let child = rejected.index[&2];
        rejected
            .world
            .get_mut::<LocalTransform>(child)
            .unwrap()
            .0
            .translation = [9.; 3];
        rejected
            .world
            .get_mut::<ObjectData>(child)
            .unwrap()
            .references
            .insert("Target.ID".into(), 999);
        let mut entity = rejected.world.entity_mut(child);
        rejected
            .columns
            .insert(&mut entity, "source.Widget", json!({"other":91}));
        let mut resources = rejected.world.resource_mut::<TickState>();
        resources.frame += 1;
        resources.resources["score"] = json!(8);
        resources.events.push(json!({"id": 2}));
        assert_ne!(save(rejected.snapshot()), before_entities);
        assert_ne!(rejected.world.resource::<TickState>(), &state);
        assert!(
            publish(&mut last_good, rejected)
                .unwrap_err()
                .contains("Target.ID")
        );
        assert_eq!(save(last_good.snapshot()), before_entities);
        assert_eq!(last_good.index, before_handles);
        assert_eq!(last_good.world.resource::<TickState>(), &state);

        let mut valid = BevyStore::restore(mode, last_good.snapshot()).unwrap();
        let changed = TickState {
            frame: 18,
            ..state.clone()
        };
        valid.world.insert_resource(changed.clone());
        publish(&mut last_good, valid).unwrap();
        assert_eq!(last_good.world.resource::<TickState>(), &changed);
    }
}
