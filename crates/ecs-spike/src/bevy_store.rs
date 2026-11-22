use bevy_ecs::{
    component::{ComponentCloneBehavior, ComponentDescriptor, ComponentId, StorageType},
    prelude::{Component, Entity, World},
    ptr::OwningPtr,
    query::{QueryBuilder, QueryState},
    reflect::AppTypeRegistry,
    world::EntityWorldMut,
};
use bevy_reflect::Reflect;
use module_engine_core::{
    GameEntity, Transform, WorldCommand, WorldObject, validate_world_commands,
};
use serde_json::{Value, json};
use std::{alloc::Layout, collections::BTreeMap};

use crate::s3;

#[cfg(test)]
#[path = "contracts.rs"]
mod contracts;

#[derive(Clone, Copy, Debug)]
pub enum Encoding {
    Dynamic,
    Hybrid,
}

#[derive(Component)]
struct StableId(u64);

#[derive(Component)]
struct LocalTransform(Transform);

#[derive(Component)]
struct ObjectData {
    parent: Option<u64>,
    color: [u8; 4],
    references: BTreeMap<String, u64>,
}

#[derive(Reflect)]
struct NativeMotion {
    moving: bool,
    offset: f64,
}

#[derive(Component)]
enum Motion {
    Native(NativeMotion),
    Json(Value),
}

impl Motion {
    fn decode(value: Value) -> Self {
        if let Some(fields) = value.as_object()
            && fields.len() == 2
            && let Some(moving) = value["moving"].as_bool()
            && let Some(number) = value["offset"].as_number()
            && number.is_f64()
        {
            return Self::Native(NativeMotion {
                moving,
                offset: number.as_f64().unwrap(),
            });
        }
        // Preserve exact keys, number representation and arbitrary schema payloads.
        Self::Json(value)
    }

    fn encode(&self) -> Value {
        match self {
            Self::Native(data) => json!({"moving": data.moving, "offset": data.offset}),
            Self::Json(data) => data.clone(),
        }
    }

    fn set_offset(&mut self, offset: f32) {
        match self {
            Self::Native(data) => data.offset = f64::from(offset),
            Self::Json(data) => data["offset"] = json!(offset),
        }
    }
}

// The registry below owns all by-ID access. Each of its IDs always represents
// one owned Value, including components whose value is an explicit JSON null.
#[derive(Default)]
struct JsonColumns(BTreeMap<String, ComponentId>);

impl JsonColumns {
    fn register(&mut self, world: &mut World, name: &str) -> ComponentId {
        *self.0.entry(name.to_owned()).or_insert_with(|| {
            // SAFETY: The descriptor only invokes this for its stored Values.
            let drop_value = |ptr: OwningPtr<'_>| unsafe { ptr.drop_as::<Value>() };
            // SAFETY: Value is Send + Sync. Its layout and destructor match.
            let descriptor = unsafe {
                ComponentDescriptor::new_with_layout(
                    name.to_owned(),
                    StorageType::Table,
                    Layout::new::<Value>(),
                    Some(drop_value),
                    true,
                    false,
                    ComponentCloneBehavior::Ignore,
                    None,
                )
            };
            world.register_component_with_descriptor(descriptor)
        })
    }

    fn insert(&self, entity: &mut EntityWorldMut, name: &str, value: Value) {
        let id = self.0[name];
        OwningPtr::make(value, |ptr| {
            // SAFETY: This registry registered id for Value in the owning world.
            unsafe { entity.insert_by_id(id, ptr) };
        });
    }

    fn snapshot(&self, entity: bevy_ecs::world::EntityRef) -> BTreeMap<String, Value> {
        self.0
            .iter()
            .filter_map(|(name, &id)| {
                let ptr = entity.get_by_id(id).ok()?;
                // SAFETY: Only this registry writes these IDs, always with Value.
                Some((name.clone(), unsafe { ptr.deref::<Value>() }.clone()))
            })
            .collect()
    }

    fn set_offset(world: &mut World, entity: Entity, id: ComponentId, offset: f32) {
        let column = world.get_mut_by_id(entity, id).unwrap();
        // SAFETY: This ID was registered for Value. MutUntyped holds exclusive access.
        let motion = unsafe { column.into_inner().deref_mut::<Value>() };
        motion["offset"] = json!(offset);
    }
}

type Matches = Vec<(u64, Entity)>;
type MatchQuery = (Entity, &'static StableId);
type NativeWrite = (&'static mut LocalTransform, &'static mut Motion);
enum Writers {
    Dynamic(ComponentId),
    Hybrid(QueryState<NativeWrite>),
}

pub struct BevyStore {
    world: World,
    index: BTreeMap<u64, Entity>,
    columns: JsonColumns,
    matches: QueryState<MatchQuery>,
    writers: Writers,
}

impl BevyStore {
    pub fn new(encoding: Encoding) -> Self {
        let mut world = World::new();
        let mut columns = JsonColumns::default();
        let (motion, writers) = match encoding {
            Encoding::Dynamic => {
                let motion = columns.register(&mut world, s3::COMPONENTS[0]);
                (motion, Writers::Dynamic(motion))
            }
            Encoding::Hybrid => {
                world.init_resource::<AppTypeRegistry>();
                world
                    .resource::<AppTypeRegistry>()
                    .write()
                    .register::<NativeMotion>();
                (
                    world.register_component::<Motion>(),
                    Writers::Hybrid(world.query::<NativeWrite>()),
                )
            }
        };
        let mut required = vec![motion];
        for name in &s3::COMPONENTS[1..] {
            required.push(columns.register(&mut world, name));
        }
        let mut query = QueryBuilder::<MatchQuery>::new(&mut world);
        for id in required {
            query.with_id(id);
        }
        let matches = query.build();
        Self {
            world,
            index: BTreeMap::new(),
            columns,
            matches,
            writers,
        }
    }

    pub fn restore(encoding: Encoding, entities: Vec<GameEntity>) -> Result<Self, String> {
        let mut store = Self::new(encoding);
        let objects = entities
            .iter()
            .map(|entity| entity.object.clone())
            .collect::<Vec<_>>();
        validate_world_commands(&objects, &[])?;
        store.insert_entities(entities)?;
        store.validate_references()?;
        Ok(store)
    }

    fn insert_entities(&mut self, entities: Vec<GameEntity>) -> Result<(), String> {
        let hybrid = matches!(self.writers, Writers::Hybrid(_));
        for GameEntity {
            object,
            components,
            references,
        } in entities
        {
            if object.id == 0 || components.keys().any(|name| name.trim().is_empty()) {
                return Err("Entity needs a nonzero ID and nonempty component IDs".into());
            }
            assert!(!self.index.contains_key(&object.id));
            for name in components.keys() {
                if !(hybrid && name == s3::COMPONENTS[0]) && !self.columns.0.contains_key(name) {
                    self.columns.register(&mut self.world, name);
                }
            }
            let mut entity = self.world.spawn((
                StableId(object.id),
                LocalTransform(object.transform),
                ObjectData {
                    parent: object.parent,
                    color: object.color,
                    references,
                },
            ));
            for (name, value) in components {
                if hybrid && name == s3::COMPONENTS[0] {
                    entity.insert(Motion::decode(value));
                } else {
                    self.columns.insert(&mut entity, &name, value);
                }
            }
            self.index.insert(object.id, entity.id());
        }
        Ok(())
    }

    pub fn spawn_s3(&mut self, commands: &[WorldCommand], entities: Vec<GameEntity>) {
        // The S3 batch always starts empty and contains only complete leaf spawns.
        assert!(self.index.is_empty());
        validate_world_commands(&[], commands).unwrap();
        self.insert_entities(entities).unwrap();
        self.validate_references().unwrap();
    }

    pub fn despawn_s3(&mut self, commands: &[WorldCommand]) {
        // The S3 deletion batch contains every entity; there are no descendants or references.
        let objects = self
            .index
            .iter()
            .map(|(&id, &entity)| self.object(id, entity))
            .collect::<Vec<_>>();
        validate_world_commands(&objects, commands).unwrap();
        for command in commands {
            let WorldCommand::Remove { id } = command else {
                unreachable!()
            };
            assert!(self.world.despawn(self.index.remove(id).unwrap()));
        }
        self.validate_references().unwrap();
    }

    fn object(&self, id: u64, handle: Entity) -> WorldObject {
        let entity = self.world.entity(handle);
        let data = entity.get::<ObjectData>().unwrap();
        WorldObject {
            id,
            parent: data.parent,
            transform: entity.get::<LocalTransform>().unwrap().0,
            color: data.color,
        }
    }

    pub fn snapshot(&self) -> Vec<GameEntity> {
        self.index
            .iter()
            .map(|(&id, &handle)| {
                let entity = self.world.entity(handle);
                let mut components = self.columns.snapshot(entity);
                if let Some(motion) = entity.get::<Motion>() {
                    components.insert(s3::COMPONENTS[0].into(), motion.encode());
                }
                GameEntity {
                    object: self.object(id, handle),
                    components,
                    references: entity.get::<ObjectData>().unwrap().references.clone(),
                }
            })
            .collect()
    }

    pub fn validate_references(&self) -> Result<(), String> {
        for (&id, &handle) in &self.index {
            let data = self.world.get::<ObjectData>(handle).unwrap();
            for (key, target) in &data.references {
                if !self.index.contains_key(target) {
                    return Err(format!(
                        "Object {id} reference '{key}' points to missing {target}"
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn tick(&mut self, frame: usize) -> Matches {
        let mut matches: Matches = self
            .matches
            .iter(&self.world)
            .map(|(handle, id)| (id.0, handle))
            .collect();
        matches.sort_unstable_by_key(|&(id, _)| id);
        let time = s3::time(frame);
        match &mut self.writers {
            Writers::Dynamic(motion) => {
                for &(id, handle) in &matches[..s3::MOVING] {
                    let offset = s3::offset(time, id);
                    JsonColumns::set_offset(&mut self.world, handle, *motion, offset);
                    self.world
                        .get_mut::<LocalTransform>(handle)
                        .unwrap()
                        .0
                        .translation[1] = offset;
                }
            }
            Writers::Hybrid(query) => {
                let mut query = query.query_mut(&mut self.world);
                for &(id, handle) in &matches[..s3::MOVING] {
                    let (mut transform, mut motion) = query.get_mut(handle).unwrap();
                    let offset = s3::offset(time, id);
                    motion.set_offset(offset);
                    transform.0.translation[1] = offset;
                }
            }
        }
        matches
    }
}
