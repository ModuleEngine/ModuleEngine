use crate::{WorldCommand, WorldObject};
use bevy_ecs::reflect::ReflectComponent;
use bevy_ecs::{
    change_detection::{DetectChangesMut, MAX_CHANGE_AGE, Tick},
    prelude::{Changed, Component, Entity, Or, World},
    world::WorldId,
};
use bevy_reflect::Reflect;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Identifies built-in world storage or a separately registered source provider.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum WorldKind {
    Objects,
    Ecs,
    Custom,
}

/// Renderer-neutral indexed triangle mesh.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<[f32; 3]>,
    pub triangles: Vec<[usize; 3]>,
    /// Per-vertex UV set zero. Missing values use `[0, 0]`.
    #[serde(default)]
    pub texcoords: Vec<[f32; 2]>,
}

impl Mesh {
    /// A unit-centered cube with side length two; no renderer-owned mesh handles.
    pub fn cube() -> Self {
        Self {
            vertices: vec![
                [-1., -1., -1.],
                [1., -1., -1.],
                [1., 1., -1.],
                [-1., 1., -1.],
                [-1., -1., 1.],
                [1., -1., 1.],
                [1., 1., 1.],
                [-1., 1., 1.],
            ],
            triangles: vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [3, 7, 6],
                [3, 6, 2],
                [0, 4, 7],
                [0, 7, 3],
                [1, 2, 6],
                [1, 6, 5],
            ],
            texcoords: vec![],
        }
    }
}

/// One drawable item. Model matrices use row-major storage and column vectors.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RenderItem {
    pub id: u64,
    pub mesh: Mesh,
    pub model: [[f32; 4]; 4],
    pub color: [u8; 4],
    #[serde(default)]
    pub material: Material,
}

/// Decoded, renderer-neutral RGBA8 base-color image. Shared between snapshots.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TextureImage {
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
}

/// Opaque static base-color material. Texture RGB is sRGB; factors are linear.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Material {
    pub base_color: [f32; 4],
    pub base_color_texture: Option<std::sync::Arc<TextureImage>>,
}
impl Default for Material {
    fn default() -> Self {
        Self {
            base_color: [1.; 4],
            base_color_texture: None,
        }
    }
}

/// Extracted immutable scene; no dependency on an ECS or object-tree API.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct SceneSnapshot {
    pub items: Vec<RenderItem>,
    pub time: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    #[error("This World provider does not support portable object editing")]
    ObjectsUnsupported,
    #[error("This World provider does not support portable world commands")]
    CommandsUnsupported,
    #[error("No executable provider registered for '{0}'")]
    MissingProvider(String),
    #[error("{0}")]
    Configuration(String),
    #[error("{0}")]
    Commands(String),
    #[error("{0}")]
    Provider(#[source] Box<dyn std::error::Error + Send + Sync>),
}

impl From<WorldError> for String {
    fn from(error: WorldError) -> Self {
        error.to_string()
    }
}

/// Minimal executable source-provider boundary implemented by world authors.
pub trait WorldProvider: Send {
    /// Advance the provider. Runtime supplies finite values and nonnegative dt;
    /// negative speed is valid and reverses motion in the built-in worlds.
    fn step(&mut self, dt: f32, speed: f32);
    /// Produce neutral geometry, sorted/stable as appropriate for this provider.
    fn snapshot(&self) -> SceneSnapshot;
    /// Optional portable editing API. Existing read-only providers keep rendering
    /// unchanged and explicitly reject editing rather than silently doing nothing.
    fn objects(&self) -> Result<Vec<WorldObject>, WorldError> {
        Err(WorldError::ObjectsUnsupported)
    }
    /// Apply all operations or none. Built-ins share the same local-transform,
    /// object-ID and subtree-removal semantics with independent persistent stores.
    fn apply_commands(&mut self, _commands: &[WorldCommand]) -> Result<(), WorldError> {
        Err(WorldError::CommandsUnsupported)
    }
    /// Validate without mutation so hosts can stage mixed world/render packets.
    fn validate_commands(&self, commands: &[WorldCommand]) -> Result<(), WorldError> {
        crate::validate_world_commands(&self.objects()?, commands).map_err(WorldError::Commands)
    }
}

/// Local translation, Euler rotation in radians and scale.
#[derive(Clone, Copy, Debug, PartialEq, Component, Reflect)]
#[reflect(Component)]
pub struct Transform {
    pub translation: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: [0.; 3],
            rotation: [0.; 3],
            scale: [1.; 3],
        }
    }
}

impl Transform {
    /// Compose T * Rz * Ry * Rx * S using the renderer's matrix convention.
    pub fn matrix(self) -> [[f32; 4]; 4] {
        let [x, y, z] = self.rotation;
        let (sx, cx) = x.sin_cos();
        let (sy, cy) = y.sin_cos();
        let (sz, cz) = z.sin_cos();
        let rx = [
            [1., 0., 0., 0.],
            [0., cx, -sx, 0.],
            [0., sx, cx, 0.],
            [0., 0., 0., 1.],
        ];
        let ry = [
            [cy, 0., sy, 0.],
            [0., 1., 0., 0.],
            [-sy, 0., cy, 0.],
            [0., 0., 0., 1.],
        ];
        let rz = [
            [cz, -sz, 0., 0.],
            [sz, cz, 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ];
        let mut result = multiply_matrices(rz, multiply_matrices(ry, rx));
        for row in result.iter_mut().take(3) {
            for (column, scale) in self.scale.iter().enumerate() {
                row[column] *= scale;
            }
        }
        for (row, translation) in self.translation.iter().enumerate() {
            result[row][3] = *translation;
        }
        result
    }
}

/// Identity model matrix.
pub fn identity_matrix() -> [[f32; 4]; 4] {
    [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ]
}

/// Compose row-major matrices for hierarchical transforms.
pub fn multiply_matrices(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.; 4]; 4];
    for (r, row) in result.iter_mut().enumerate() {
        for (c, value) in row.iter_mut().enumerate() {
            *value = (0..4).map(|k| a[r][k] * b[k][c]).sum();
        }
    }
    result
}

/// Apply an affine model matrix to a position.
pub fn transform_point(matrix: [[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|r| {
        matrix[r][0] * p[0] + matrix[r][1] * p[1] + matrix[r][2] * p[2] + matrix[r][3]
    })
}

#[derive(Clone)]
struct Object {
    id: u64,
    local: Transform,
    angular_velocity: [f32; 3],
    color: [u8; 4],
    children: Vec<Object>,
}

/// A recursive owned hierarchy. No entity/component table is hidden underneath.
#[derive(Clone)]
pub struct ObjectsWorld {
    roots: Vec<Object>,
    time: f32,
}

impl ObjectsWorld {
    /// Construct authored objects in a real recursive store, with no demo motion.
    pub fn from_objects(objects: Vec<WorldObject>) -> Result<Self, String> {
        crate::validate_world_commands(&objects, &[])?;
        fn build(
            id: u64,
            objects: &BTreeMap<u64, WorldObject>,
            children: &BTreeMap<Option<u64>, Vec<u64>>,
        ) -> Object {
            let object = &objects[&id];
            Object {
                id,
                local: object.transform,
                color: object.color,
                angular_velocity: [0.; 3],
                children: children
                    .get(&Some(id))
                    .into_iter()
                    .flatten()
                    .map(|id| build(*id, objects, children))
                    .collect(),
            }
        }
        let objects = objects
            .into_iter()
            .map(|o| (o.id, o))
            .collect::<BTreeMap<_, _>>();
        let mut children = BTreeMap::<Option<u64>, Vec<u64>>::new();
        for object in objects.values() {
            children.entry(object.parent).or_default().push(object.id);
        }
        Ok(Self {
            roots: children
                .get(&None)
                .into_iter()
                .flatten()
                .map(|id| build(*id, &objects, &children))
                .collect(),
            time: 0.,
        })
    }
    /// Same initial visible scene as EcsWorld, stored as owned parent/child objects.
    pub fn demo() -> Self {
        Self {
            roots: vec![
                Object {
                    id: 1,
                    local: transform([-1.35, 0.0, 0.0], [0.8; 3]),
                    angular_velocity: [0.18, 0.65, 0.0],
                    color: [80, 175, 245, 255],
                    children: vec![Object {
                        id: 2,
                        local: transform([0.0, 2.0, 0.0], [0.32; 3]),
                        angular_velocity: [0.3, -0.7, 0.0],
                        color: [255, 182, 75, 255],
                        children: vec![],
                    }],
                },
                Object {
                    id: 3,
                    local: transform([1.5, -0.15, 0.2], [0.65; 3]),
                    angular_velocity: [0.25, -0.45, 0.0],
                    color: [189, 128, 240, 255],
                    children: vec![],
                },
                Object {
                    id: 4,
                    local: transform([0.0, -1.0, 0.0], [4.0, 0.08, 3.0]),
                    angular_velocity: [0.; 3],
                    color: [79, 103, 111, 255],
                    children: vec![],
                },
            ],
            time: 0.0,
        }
    }
}

impl WorldProvider for ObjectsWorld {
    fn objects(&self) -> Result<Vec<WorldObject>, WorldError> {
        fn collect(object: &Object, parent: Option<u64>, result: &mut Vec<WorldObject>) {
            result.push(WorldObject {
                id: object.id,
                parent,
                transform: object.local,
                color: object.color,
            });
            for child in &object.children {
                collect(child, Some(object.id), result);
            }
        }
        let mut objects = vec![];
        for root in &self.roots {
            collect(root, None, &mut objects);
        }
        objects.sort_by_key(|o| o.id);
        Ok(objects)
    }

    fn apply_commands(&mut self, commands: &[WorldCommand]) -> Result<(), WorldError> {
        fn velocities(object: &Object, result: &mut BTreeMap<u64, [f32; 3]>) {
            result.insert(object.id, object.angular_velocity);
            for child in &object.children {
                velocities(child, result);
            }
        }
        fn build(
            id: u64,
            objects: &BTreeMap<u64, WorldObject>,
            velocities: &BTreeMap<u64, [f32; 3]>,
            children: &BTreeMap<Option<u64>, Vec<u64>>,
        ) -> Object {
            let object = &objects[&id];
            Object {
                id,
                local: object.transform,
                color: object.color,
                angular_velocity: velocities.get(&id).copied().unwrap_or([0.; 3]),
                children: children
                    .get(&Some(id))
                    .into_iter()
                    .flatten()
                    .map(|id| build(*id, objects, velocities, children))
                    .collect(),
            }
        }
        let mut velocities_map = BTreeMap::new();
        for root in &self.roots {
            velocities(root, &mut velocities_map);
        }
        let mut objects = self.objects()?.into_iter().map(|o| (o.id, o)).collect();
        crate::control::edit_object_map(&mut objects, commands).map_err(WorldError::Commands)?;
        for command in commands {
            if let WorldCommand::Spawn { object } = command {
                velocities_map.remove(&object.id);
            }
        }
        let mut children: BTreeMap<Option<u64>, Vec<u64>> = BTreeMap::new();
        for object in objects.values() {
            children.entry(object.parent).or_default().push(object.id);
        }
        self.roots = children
            .get(&None)
            .into_iter()
            .flatten()
            .map(|id| build(*id, &objects, &velocities_map, &children))
            .collect();
        Ok(())
    }

    fn step(&mut self, dt: f32, speed: f32) {
        fn advance(object: &mut Object, amount: f32) {
            for i in 0..3 {
                object.local.rotation[i] += object.angular_velocity[i] * amount;
            }
            for child in &mut object.children {
                advance(child, amount);
            }
        }
        self.time += dt;
        for root in &mut self.roots {
            advance(root, dt * speed);
        }
    }

    fn snapshot(&self) -> SceneSnapshot {
        fn extract(object: &Object, parent: [[f32; 4]; 4], items: &mut Vec<RenderItem>) {
            let model = multiply_matrices(parent, object.local.matrix());
            items.push(RenderItem {
                id: object.id,
                mesh: Mesh::cube(),
                model,
                color: object.color,
                material: Material::default(),
            });
            for child in &object.children {
                extract(child, model, items);
            }
        }
        let mut items = vec![];
        for root in &self.roots {
            extract(root, identity_matrix(), &mut items);
        }
        items.sort_by_key(|item| item.id);
        SceneSnapshot {
            items,
            time: self.time,
        }
    }
}

#[derive(Component, Reflect)]
#[component(immutable)]
struct StableId(u64);

#[derive(Clone, Debug, Component, PartialEq, Reflect)]
struct ObjectData {
    parent: Option<u64>,
    color: [u8; 4],
    angular_velocity: [f32; 3],
    references: BTreeMap<String, u64>,
}

impl ObjectData {
    fn from_object(object: WorldObject) -> Self {
        Self {
            parent: object.parent,
            color: object.color,
            angular_velocity: [0.; 3],
            references: BTreeMap::new(),
        }
    }
}

#[derive(Component, Reflect)]
#[component(immutable)]
struct EntityLifetime(u64);

// The values still live in the lossless JSON columns. Only the two columns
// that feed the stock drawable mark this native component.
#[derive(Component)]
struct RenderBindings;

#[derive(Default)]
pub struct RenderChangeCursor {
    position: Option<RenderChangePosition>,
}

#[derive(Clone, Copy)]
struct RenderChangePosition {
    world: WorldId,
    tick: Tick,
    epoch: u64,
    writes: u64,
}

/// Candidate object IDs from committed Bevy storage. An individual candidate
/// can still have an unchanged render projection (for example, a reference edit).
pub struct WorldRenderChanges {
    pub reset: bool,
    pub changed: Vec<u64>,
    pub removed: Vec<u64>,
}

/// Canonical gameplay storage. Native object rows live in Bevy, and schema
/// component columns keep their existing stable-ID keys and lossless JSON.
#[derive(Debug)]
pub struct EcsWorld {
    rows: Arc<EcsRows>,
    staged: Option<StagedRows>,
    time: f32,
    malformed_component_reads: Arc<AtomicU64>,
}

#[derive(Debug, Default)]
struct EcsRows {
    world: World,
    index: BTreeMap<u64, Entity>,
    components: BTreeMap<String, BTreeMap<u64, serde_json::Value>>,
    change_epoch: u64,
    render_writes: u64,
    render_removals: VecDeque<(u64, u64)>,
    dropped_removal_epoch: u64,
}

#[derive(Clone, Debug)]
struct StagedRows {
    validated_base: bool,
    // None marks a removed lifetime. Existing rows are copied on first write.
    edits: BTreeMap<u64, Option<EntityRow>>,
    parent: Option<Arc<Self>>,
    unchecked_transforms: BTreeSet<u64>,
    parent_transforms_checked: bool,
}

impl StagedRows {
    fn new(validated_base: bool, parent: Option<Arc<Self>>) -> Self {
        Self {
            validated_base,
            edits: BTreeMap::new(),
            parent,
            unchecked_transforms: BTreeSet::new(),
            parent_transforms_checked: false,
        }
    }

    fn layers(&self) -> impl Iterator<Item = &Self> {
        std::iter::successors(Some(self), |layer| layer.parent.as_deref())
    }

    fn edit(&self, id: u64) -> Option<&Option<EntityRow>> {
        self.layers().find_map(|layer| layer.edits.get(&id))
    }

    fn changes(&self) -> impl Iterator<Item = (u64, &Option<EntityRow>)> {
        let mut combined = BTreeMap::new();
        let local = if self.parent.is_none() {
            Some(&self.edits)
        } else {
            // Nested enumeration needs a sorted, deduplicated view of the edits.
            for layer in self.layers() {
                for (&id, edit) in &layer.edits {
                    combined.entry(id).or_insert(edit);
                }
            }
            None
        };
        local
            .into_iter()
            .flatten()
            .map(|(&id, edit)| (id, edit))
            .chain(combined)
    }

    fn unchecked_transform_ids(&self) -> impl Iterator<Item = u64> + '_ {
        std::iter::successors(Some(self), |layer| {
            if layer.parent_transforms_checked {
                None
            } else {
                layer.parent.as_deref()
            }
        })
        .flat_map(|layer| layer.unchecked_transforms.iter().copied())
    }

    fn mark_transforms_checked(&mut self) {
        self.unchecked_transforms.clear();
        self.parent_transforms_checked = true;
    }

    fn compact(&mut self) {
        while let Some(parent) = self.parent.as_mut().and_then(Arc::get_mut) {
            if self.parent_transforms_checked {
                parent.mark_transforms_checked();
            }
            parent
                .unchecked_transforms
                .extend(std::mem::take(&mut self.unchecked_transforms));
            parent.edits.extend(std::mem::take(&mut self.edits));
            *self = std::mem::replace(parent, Self::new(self.validated_base, None));
        }
    }
}

#[derive(Clone, Debug)]
struct EntityRow {
    data: ObjectData,
    transform: Transform,
    lifetime: u64,
    components: BTreeMap<String, serde_json::Value>,
}

impl EntityRow {
    fn new(object: WorldObject) -> Self {
        Self {
            transform: object.transform,
            data: ObjectData::from_object(object),
            lifetime: new_entity_lifetime(),
            components: BTreeMap::new(),
        }
    }
}

fn new_entity_lifetime() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.try_update(
        std::sync::atomic::Ordering::Relaxed,
        std::sync::atomic::Ordering::Relaxed,
        |value| value.checked_add(1),
    )
    .expect("Entity lifetime token space exhausted")
}

impl Clone for EcsRows {
    fn clone(&self) -> Self {
        let mut world = World::new();
        let index = world
            .spawn_batch(self.index.iter().map(|(&id, &handle)| {
                (
                    StableId(id),
                    *self.world.get::<Transform>(handle).unwrap(),
                    self.world.get::<ObjectData>(handle).unwrap().clone(),
                    EntityLifetime(self.world.get::<EntityLifetime>(handle).unwrap().0),
                    RenderBindings,
                )
            }))
            .zip(self.index.keys().copied())
            .map(|(handle, id)| (id, handle))
            .collect();
        Self {
            world,
            index,
            components: self.components.clone(),
            change_epoch: 0,
            render_writes: 0,
            render_removals: VecDeque::new(),
            dropped_removal_epoch: 0,
        }
    }
}

impl EcsRows {
    fn mark_binding_changed(&mut self, id: u64, name: &str) {
        if matches!(name, "mesh" | "material") {
            self.render_writes += 1;
            self.world
                .get_mut::<RenderBindings>(self.index[&id])
                .unwrap()
                .set_changed();
        }
    }

    fn advance_removals(&mut self) {
        self.world.clear_trackers();
        self.change_epoch += 1;
        self.world.check_change_ticks();
    }

    fn record_removal(&mut self, id: u64) {
        // Bevy's removal messages age on each structural publication. Keep
        // stable IDs available to slower readers across several publications.
        const MAX_RENDER_REMOVALS: usize = 4096;
        if self.render_removals.len() == MAX_RENDER_REMOVALS {
            self.dropped_removal_epoch = self.render_removals.pop_front().unwrap().0;
        }
        self.render_removals.push_back((self.change_epoch + 1, id));
    }

    fn native<T: Component>(&self, id: u64) -> Option<&T> {
        let handle = *self.index.get(&id)?;
        debug_assert_eq!(self.world.get::<StableId>(handle).map(|id| id.0), Some(id));
        self.world.get(handle)
    }

    fn native_mut<T: Component<Mutability = bevy_ecs::component::Mutable>>(
        &mut self,
        id: u64,
    ) -> Option<&mut T> {
        let value = self
            .world
            .get_mut::<T>(*self.index.get(&id)?)
            .map(|value| value.into_inner())?;
        self.render_writes += 1;
        Some(value)
    }

    fn components_for(&self, id: u64) -> BTreeMap<String, serde_json::Value> {
        self.components
            .iter()
            .filter_map(|(name, column)| column.get(&id).map(|value| (name.clone(), value.clone())))
            .collect()
    }

    fn row(&self, id: u64) -> Option<EntityRow> {
        Some(EntityRow {
            data: self.native::<ObjectData>(id)?.clone(),
            transform: *self.native::<Transform>(id)?,
            lifetime: self.native::<EntityLifetime>(id)?.0,
            components: self.components_for(id),
        })
    }

    fn remove_row(&mut self, id: u64) -> bool {
        let Some(handle) = self.index.remove(&id) else {
            return false;
        };
        let removed = self.world.despawn(handle);
        debug_assert!(removed);
        self.record_removal(id);
        self.replace_components(id, BTreeMap::new());
        true
    }

    fn put_row(&mut self, id: u64, row: EntityRow) {
        if let Some(previous) = self.native::<EntityLifetime>(id) {
            debug_assert_eq!(previous.0, row.lifetime);
            let old_data = self.native::<ObjectData>(id).unwrap();
            if old_data != &row.data
                || old_data.angular_velocity.map(f32::to_bits)
                    != row.data.angular_velocity.map(f32::to_bits)
            {
                *self.native_mut(id).unwrap() = row.data;
            }
            if !crate::ComponentRegistry::TRANSFORM
                .same_wire_value(self.native::<Transform>(id).unwrap(), &row.transform)
            {
                *self.native_mut(id).unwrap() = row.transform;
            }
            for name in ["mesh", "material"] {
                if self.components.get(name).and_then(|column| column.get(&id))
                    != row.components.get(name)
                {
                    self.mark_binding_changed(id, name);
                }
            }
            self.replace_components(id, row.components);
            return;
        }
        let handle = self
            .world
            .spawn((
                StableId(id),
                row.transform,
                row.data,
                EntityLifetime(row.lifetime),
                RenderBindings,
            ))
            .id();
        self.render_writes += 1;
        self.index.insert(id, handle);
        for (name, value) in row.components {
            self.components.entry(name).or_default().insert(id, value);
        }
    }

    fn replace_components(&mut self, id: u64, mut values: BTreeMap<String, serde_json::Value>) {
        self.components.retain(|name, column| {
            if let Some(value) = values.remove(name) {
                column.insert(id, value);
            } else {
                column.remove(&id);
            }
            !column.is_empty()
        });
        for (name, value) in values {
            self.components.entry(name).or_default().insert(id, value);
        }
    }
}

/// An explicit clone still creates an independent Bevy world (used, for
/// example, by lifecycle scratch state). Tick checkpoints share frozen rows.
impl Clone for EcsWorld {
    fn clone(&self) -> Self {
        let mut copy = Self {
            rows: Arc::new((*self.rows).clone()),
            staged: self.staged.clone(),
            time: self.time,
            malformed_component_reads: Arc::new(AtomicU64::new(self.malformed_component_reads())),
        };
        copy.publish_staged();
        copy
    }
}

impl PartialEq for EcsWorld {
    fn eq(&self, other: &Self) -> bool {
        let materialized = self.staged.is_none() && other.staged.is_none();
        self.time == other.time
            && (!materialized || self.rows.components == other.rows.components)
            && self.entity_ids().eq(other.entity_ids())
            && self.entity_ids().all(|id| {
                self.object_data(id) == other.object_data(id)
                    && self.transform(id) == other.transform(id)
                    && (materialized || self.entity_components(id) == other.entity_components(id))
            })
    }
}

impl EcsWorld {
    pub(crate) fn checkpoint_tick(&mut self, validated_base: bool) -> Self {
        let parent = self.staged.take().map(Arc::new);
        let validated_base = parent
            .as_ref()
            .map_or(validated_base, |parent| parent.validated_base);
        let original = Self {
            rows: Arc::clone(&self.rows),
            staged: parent
                .as_ref()
                .map(|parent| StagedRows::new(validated_base, Some(Arc::clone(parent)))),
            time: self.time,
            // The diagnostic belongs to the live world, even when an attempted
            // tick rolls back its authored/gameplay changes.
            malformed_component_reads: Arc::clone(&self.malformed_component_reads),
        };
        self.staged = Some(StagedRows::new(validated_base, parent));
        original
    }

    pub(crate) fn finish_tick(&mut self, original: Self) {
        let outermost = original.staged.is_none();
        drop(original);
        if outermost {
            self.publish_staged();
        } else {
            self.compact_staged();
        }
    }

    pub(crate) fn compact_staged(&mut self) {
        if let Some(staged) = &mut self.staged {
            staged.compact();
        }
    }

    fn publish_staged(&mut self) {
        let Some(mut staged) = self.staged.take() else {
            return;
        };
        staged.compact();
        while let Some(parent) = staged.parent.take() {
            let parent = Arc::unwrap_or_clone(parent);
            for (id, edit) in parent.edits {
                staged.edits.entry(id).or_insert(edit);
            }
            staged.parent = parent.parent;
        }
        if staged.edits.is_empty() {
            return;
        }
        let rows = Arc::make_mut(&mut self.rows);
        let mut despawned = false;
        for (&id, edit) in &staged.edits {
            if edit.as_ref().is_none_or(|row| {
                rows.native::<EntityLifetime>(id)
                    .is_some_and(|previous| previous.0 != row.lifetime)
            }) {
                despawned |= rows.remove_row(id);
            }
        }
        if despawned {
            // Age removal messages before writes so the tracker update keeps
            // this publication's new and changed components visible.
            rows.advance_removals();
        }
        for (id, row) in staged
            .edits
            .into_iter()
            .filter_map(|(id, edit)| edit.map(|row| (id, row)))
        {
            rows.put_row(id, row);
        }
    }

    fn stage_row(&mut self, id: u64) -> Option<&mut EntityRow> {
        use std::collections::btree_map::Entry;
        let staged = self.staged.as_mut().unwrap();
        match staged.edits.entry(id) {
            Entry::Occupied(edit) => edit.into_mut().as_mut(),
            Entry::Vacant(edit) => {
                let row = match staged.parent.as_ref().and_then(|parent| parent.edit(id)) {
                    Some(row) => row.clone(),
                    None => self.rows.row(id),
                }?;
                edit.insert(Some(row)).as_mut()
            }
        }
    }

    fn edit(&self, id: u64) -> Option<&Option<EntityRow>> {
        self.staged.as_ref()?.edit(id)
    }

    fn spawn_object(&mut self, object: WorldObject) {
        let id = object.id;
        debug_assert!(!self.contains(id));
        let row = EntityRow::new(object);
        if let Some(staged) = &mut self.staged {
            staged.edits.insert(id, Some(row));
        } else {
            Arc::make_mut(&mut self.rows).put_row(id, row);
        }
    }

    fn object_data(&self, id: u64) -> Option<&ObjectData> {
        match self.edit(id) {
            Some(edit) => edit.as_ref().map(|row| &row.data),
            None => self.rows.native(id),
        }
    }

    fn object_data_mut(&mut self, id: u64) -> Option<&mut ObjectData> {
        if self.staged.is_some() {
            self.stage_row(id).map(|row| &mut row.data)
        } else {
            Arc::make_mut(&mut self.rows).native_mut(id)
        }
    }

    /// Construct authored rows without demo motion.
    pub fn from_objects(objects: Vec<WorldObject>) -> Result<Self, String> {
        crate::validate_world_commands(&objects, &[])?;
        let mut world = Self {
            rows: Arc::new(EcsRows::default()),
            staged: None,
            time: 0.,
            malformed_component_reads: Arc::default(),
        };
        for object in objects {
            world.spawn_object(object);
        }
        Ok(world)
    }

    pub fn from_entities(entities: Vec<crate::GameEntity>) -> Result<Self, String> {
        let mut world = Self::from_objects(
            entities
                .iter()
                .map(|entity| entity.object.clone())
                .collect(),
        )?;
        for entity in entities {
            let id = entity.object.id;
            *world.references_mut(id).unwrap() = entity.references;
            for (name, value) in entity.components {
                world.insert_component(id, name, value)?;
            }
        }
        world.validate_references()?;
        Ok(world)
    }

    pub fn entity_ids(&self) -> impl Iterator<Item = u64> + '_ {
        let mut base = self.rows.index.keys().copied().peekable();
        let mut edits = self.staged.iter().flat_map(StagedRows::changes).peekable();
        std::iter::from_fn(move || {
            loop {
                let Some(&(id, _)) = edits.peek() else {
                    return base.next();
                };
                if base.peek().is_some_and(|base_id| *base_id < id) {
                    return base.next();
                }
                base.next_if_eq(&id);
                if edits.next().unwrap().1.is_some() {
                    return Some(id);
                }
            }
        })
    }

    pub fn contains(&self, id: u64) -> bool {
        self.edit(id)
            .map_or_else(|| self.rows.index.contains_key(&id), Option::is_some)
    }

    /// Opaque process-local identity for module-owned transient state. Tokens
    /// survive checkpoints/ordinary edits but are fresh after spawn or reload.
    pub fn lifetime(&self, id: u64) -> Option<u64> {
        match self.edit(id) {
            Some(edit) => edit.as_ref().map(|row| row.lifetime),
            None => self.rows.native::<EntityLifetime>(id).map(|value| value.0),
        }
    }

    pub fn len(&self) -> usize {
        let mut count = self.rows.index.len();
        for (id, row) in self.staged.iter().flat_map(StagedRows::changes) {
            match (self.rows.index.contains_key(&id), row.is_some()) {
                (true, false) => count -= 1,
                (false, true) => count += 1,
                _ => (),
            }
        }
        count
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entities_with(&self, required: &[&str]) -> Vec<u64> {
        if required.is_empty() {
            return self.entity_ids().collect();
        }
        let mut matches = Vec::new();
        let columns: Option<Vec<_>> = required
            .iter()
            .map(|name| self.rows.components.get(*name))
            .collect();
        if let Some(mut columns) = columns {
            let (smallest, _) = columns
                .iter()
                .enumerate()
                .min_by_key(|(_, column)| column.len())
                .unwrap();
            let base = columns.swap_remove(smallest);
            matches.extend(base.keys().copied());
            for column in columns {
                if matches.is_empty() {
                    break;
                }
                if std::ptr::eq(base, column) {
                    continue;
                }
                // Walk similarly sized ordered sets. Use keyed lookups
                // when scanning the column would visit far more keys.
                if column.len() <= matches.len().saturating_mul(4) {
                    let mut keys = column.keys().copied();
                    let mut key = keys.next();
                    matches.retain(|&id| {
                        while let Some(candidate) = key {
                            match candidate.cmp(&id) {
                                std::cmp::Ordering::Less => key = keys.next(),
                                std::cmp::Ordering::Equal => {
                                    key = keys.next();
                                    return true;
                                }
                                std::cmp::Ordering::Greater => break,
                            }
                        }
                        false
                    });
                } else {
                    matches.retain(|id| column.contains_key(id));
                }
            }
            if self.staged.is_some() {
                matches.retain(|&id| self.edit(id).is_none());
            }
        }
        if let Some(staged) = &self.staged {
            let unchanged_count = matches.len();
            matches.extend(staged.changes().filter_map(|(id, edit)| {
                let row = edit.as_ref()?;
                required
                    .iter()
                    .all(|name| row.components.contains_key(*name))
                    .then_some(id)
            }));
            if matches.len() != unchanged_count {
                matches.sort_unstable();
            }
        }
        matches
    }

    pub fn object(&self, id: u64) -> Option<WorldObject> {
        let data = self.object_data(id)?;
        Some(WorldObject {
            id,
            parent: data.parent,
            transform: *self.transform(id)?,
            color: data.color,
        })
    }

    pub fn objects_snapshot(&self) -> Vec<WorldObject> {
        self.entity_ids().filter_map(|id| self.object(id)).collect()
    }

    /// Resolve accepted local transforms in hierarchy order. Callers gathering
    /// several models can share a scratch cache within the same world view.
    pub fn render_model(&self, id: u64, cache: &mut BTreeMap<u64, [[f32; 4]; 4]>) -> [[f32; 4]; 4] {
        if let Some(model) = cache.get(&id) {
            return *model;
        }
        let parent = self
            .object_data(id)
            .unwrap()
            .parent
            .map_or_else(identity_matrix, |parent| self.render_model(parent, cache));
        let model = multiply_matrices(parent, self.transform(id).unwrap().matrix());
        cache.insert(id, model);
        model
    }

    pub(crate) fn read_render_changes(
        &mut self,
        cursor: &mut RenderChangeCursor,
    ) -> WorldRenderChanges {
        assert!(
            self.staged.is_none(),
            "render extraction requires committed ECS rows"
        );
        let rows = Arc::get_mut(&mut self.rows).expect("committed ECS has no open checkpoint");
        let world_id = rows.world.id();
        let previous = cursor.position.filter(|position| {
            position.world == world_id
                && position.epoch >= rows.dropped_removal_epoch
                && rows.change_epoch - position.epoch < u64::from(MAX_CHANGE_AGE)
        });
        // A reader with no intervening writes skips even Bevy's Changed scan.
        let changed = match previous {
            None => rows.index.keys().copied().collect(),
            Some(position) if position.writes == rows.render_writes => Vec::new(),
            Some(position) => rows.world.last_change_tick_scope(position.tick, |world| {
                let mut query = world.query_filtered::<&StableId, Or<(
                    Changed<Transform>,
                    Changed<ObjectData>,
                    Changed<RenderBindings>,
                )>>();
                query.iter(world).map(|id| id.0).collect()
            }),
        };
        let tick = rows.world.increment_change_tick();
        rows.change_epoch += 1;
        rows.world.check_change_ticks();
        cursor.position = Some(RenderChangePosition {
            world: world_id,
            tick,
            epoch: rows.change_epoch,
            writes: rows.render_writes,
        });
        let removed = previous.map_or_else(Vec::new, |position| {
            rows.render_removals
                .iter()
                .rev()
                .take_while(|(epoch, _)| *epoch > position.epoch)
                .map(|(_, id)| *id)
                .collect()
        });
        WorldRenderChanges {
            reset: previous.is_none(),
            changed,
            removed,
        }
    }

    /// Read one local transform from the current committed or staged view.
    pub fn transform(&self, id: u64) -> Option<&Transform> {
        match self.edit(id) {
            Some(edit) => edit.as_ref().map(|row| &row.transform),
            None => self.rows.native(id),
        }
    }

    pub fn transform_mut(&mut self, id: u64) -> Option<&mut Transform> {
        if let Some(staged) = &mut self.staged {
            staged.unchecked_transforms.insert(id);
            self.stage_row(id).map(|row| &mut row.transform)
        } else {
            Arc::make_mut(&mut self.rows).native_mut(id)
        }
    }

    pub fn component(&self, id: u64, name: &str) -> Option<&serde_json::Value> {
        match self.edit(id) {
            Some(edit) => edit.as_ref()?.components.get(name),
            None => self.rows.components.get(name)?.get(&id),
        }
    }

    /// Decode a live component. An absent component is a successful empty read.
    pub fn decode_component<'a, T: Deserialize<'a>>(
        &'a self,
        id: u64,
        name: &str,
    ) -> Result<Option<T>, serde_json::Error> {
        self.component(id, name)
            .map(T::deserialize)
            .transpose()
            .inspect_err(|error| {
                let count = self
                    .malformed_component_reads
                    .fetch_add(1, Ordering::Relaxed);
                if cfg!(debug_assertions) && count < 8 {
                    eprintln!("Malformed component '{name}' on object {id}: {error}");
                }
            })
    }

    pub fn malformed_component_reads(&self) -> u64 {
        self.malformed_component_reads.load(Ordering::Relaxed)
    }

    // A staged removal can empty a base column. Conservatively treat it as
    // present until publication so callers need only inspect staged writes.
    pub(crate) fn may_have_component(&self, name: &str) -> bool {
        self.rows.components.contains_key(name)
            || self.staged.as_ref().is_some_and(|staged| {
                staged.layers().any(|layer| {
                    layer
                        .edits
                        .values()
                        .flatten()
                        .any(|row| row.components.contains_key(name))
                })
            })
    }

    pub fn component_mut(&mut self, id: u64, name: &str) -> Option<&mut serde_json::Value> {
        if self.staged.is_some() {
            self.component(id, name)?;
            return self.stage_row(id)?.components.get_mut(name);
        }
        // Check before potentially cloning a shared world or notifying render
        // extraction. For ordinary values in a unique world, the mutable
        // lookup itself can report absence.
        if Arc::strong_count(&self.rows) != 1 || matches!(name, "mesh" | "material") {
            self.component(id, name)?;
        }
        let rows = Arc::make_mut(&mut self.rows);
        rows.mark_binding_changed(id, name);
        rows.components.get_mut(name)?.get_mut(&id)
    }

    /// Edit the local transform and an existing named component together.
    /// Calls `edit` in input order, including repeated IDs, and skips rows
    /// without that component. Returns the number of calls to `edit`.
    pub fn edit_transforms_with_component(
        &mut self,
        ids: &[u64],
        name: &str,
        mut edit: impl FnMut(u64, &mut Transform, &mut serde_json::Value),
    ) -> usize {
        let mut count = 0;
        if self.staged.is_some() {
            for &id in ids {
                if self.component(id, name).is_none() {
                    continue;
                }
                self.staged
                    .as_mut()
                    .unwrap()
                    .unchecked_transforms
                    .insert(id);
                let row = self.stage_row(id).unwrap();
                edit(
                    id,
                    &mut row.transform,
                    row.components.get_mut(name).unwrap(),
                );
                count += 1;
            }
            return count;
        }

        if ids.is_empty()
            || (Arc::strong_count(&self.rows) != 1
                && !self
                    .rows
                    .components
                    .get(name)
                    .is_some_and(|column| ids.iter().any(|id| column.contains_key(id))))
        {
            return 0;
        }
        let rows = Arc::make_mut(&mut self.rows);
        let Some(column) = rows.components.get_mut(name) else {
            return 0;
        };
        let render_binding = matches!(name, "mesh" | "material");
        for &id in ids {
            let Some(component) = column.get_mut(&id) else {
                continue;
            };
            let handle = rows.index[&id];
            if count == 0 {
                // Record the batch before exposing its first mutable values.
                rows.render_writes += 1;
            }
            if render_binding {
                rows.world
                    .get_mut::<RenderBindings>(handle)
                    .unwrap()
                    .set_changed();
            }
            let transform = rows
                .world
                .get_mut::<Transform>(handle)
                .unwrap()
                .into_inner();
            edit(id, transform, component);
            count += 1;
        }
        count
    }

    pub fn insert_component(
        &mut self,
        id: u64,
        name: impl Into<String>,
        value: serde_json::Value,
    ) -> Result<Option<serde_json::Value>, String> {
        let name = name.into();
        if !self.contains(id) {
            return Err(format!("Entity {id} does not exist"));
        }
        if name.trim().is_empty() {
            return Err("Component ID must be nonempty".into());
        }
        if self.staged.is_some() {
            Ok(self.stage_row(id).unwrap().components.insert(name, value))
        } else {
            let rows = Arc::make_mut(&mut self.rows);
            rows.mark_binding_changed(id, &name);
            Ok(rows.components.entry(name).or_default().insert(id, value))
        }
    }

    pub fn remove_component(&mut self, id: u64, name: &str) -> Option<serde_json::Value> {
        self.component(id, name)?;
        if self.staged.is_some() {
            return self.stage_row(id)?.components.remove(name);
        }
        let rows = Arc::make_mut(&mut self.rows);
        rows.mark_binding_changed(id, name);
        let components = &mut rows.components;
        let column = components.get_mut(name)?;
        let removed = column.remove(&id);
        if column.is_empty() {
            components.remove(name);
        }
        removed
    }

    pub fn references(&self, id: u64) -> Option<&BTreeMap<String, u64>> {
        Some(&self.object_data(id)?.references)
    }

    pub fn references_mut(&mut self, id: u64) -> Option<&mut BTreeMap<String, u64>> {
        Some(&mut self.object_data_mut(id)?.references)
    }

    fn entity_components(&self, id: u64) -> BTreeMap<String, serde_json::Value> {
        match self.edit(id) {
            Some(edit) => edit.as_ref().unwrap().components.clone(),
            None => self.rows.components_for(id),
        }
    }

    pub fn entity(&self, id: u64) -> Option<crate::GameEntity> {
        Some(crate::GameEntity {
            object: self.object(id)?,
            components: self.entity_components(id),
            references: self.references(id).unwrap().clone(),
        })
    }

    pub fn entities_snapshot(&self) -> Vec<crate::GameEntity> {
        self.entity_ids().filter_map(|id| self.entity(id)).collect()
    }

    /// Replace one entity's complete DTO without treating the update as a new
    /// lifetime. Structural validation completes before any column is changed.
    /// References may be transiently dangling while systems stage a tick; the
    /// session validates all referents before publication.
    pub fn upsert_entity(&mut self, entity: crate::GameEntity) -> Result<(), String> {
        let id = entity.object.id;
        if id == 0 || entity.components.keys().any(|name| name.trim().is_empty()) {
            return Err("Entity needs a nonzero ID and nonempty component IDs".into());
        }
        let mut objects: BTreeMap<_, _> = self
            .objects_snapshot()
            .into_iter()
            .map(|object| (object.id, object))
            .collect();
        objects.insert(id, entity.object.clone());
        crate::control::edit_object_map(&mut objects, &[])?;
        let mut row = match self.object_data(id) {
            Some(data) => EntityRow {
                data: data.clone(),
                lifetime: self.lifetime(id).unwrap(),
                transform: entity.object.transform,
                components: BTreeMap::new(),
            },
            None => EntityRow::new(entity.object.clone()),
        };
        row.data.parent = entity.object.parent;
        row.data.color = entity.object.color;
        row.data.references = entity.references;
        row.components = entity.components;
        if let Some(staged) = &mut self.staged {
            staged.edits.insert(id, Some(row));
            staged.mark_transforms_checked();
        } else {
            Arc::make_mut(&mut self.rows).put_row(id, row);
        }
        Ok(())
    }

    fn validate_entity_references(&self, id: u64) -> Result<(), String> {
        for (name, target) in self.references(id).unwrap() {
            if !self.contains(*target) {
                return Err(format!(
                    "Object {id} reference '{name}' points to missing {target}; clear references before removing their targets"
                ));
            }
        }
        Ok(())
    }

    pub fn validate_references(&self) -> Result<(), String> {
        self.entity_ids()
            .try_for_each(|id| self.validate_entity_references(id))
    }

    pub(crate) fn unchanged_since(&self, before: &Self) -> bool {
        Arc::ptr_eq(&self.rows, &before.rows)
            && self
                .staged
                .as_ref()
                .is_some_and(|staged| staged.layers().all(|layer| layer.edits.is_empty()))
    }

    pub(crate) fn validate_changes_since(&self, before: &Self) -> Result<(), String> {
        if Arc::ptr_eq(&self.rows, &before.rows)
            && let Some(staged) = &self.staged
            && staged.changes().all(|(id, edit)| {
                edit.as_ref()
                    .zip(self.rows.native::<ObjectData>(id))
                    .is_some_and(|(edit, original)| edit.data.parent == original.parent)
            })
        {
            for (id, _) in staged.changes() {
                self.validate_entity_references(id)?;
            }
            for (id, edit) in staged.changes() {
                crate::control::validate_object_transform(id, &edit.as_ref().unwrap().transform)?;
            }
            return Ok(());
        }
        self.validate_references()?;
        self.validate_objects()
    }

    pub(crate) fn validate_objects(&self) -> Result<(), String> {
        let mut objects = self
            .entity_ids()
            .map(|id| (id, self.object(id).unwrap()))
            .collect();
        crate::control::edit_object_map(&mut objects, &[])
    }

    fn remove_rows(&mut self, ids: impl IntoIterator<Item = u64>) {
        if let Some(staged) = &mut self.staged {
            for id in ids {
                staged.edits.insert(id, None);
                staged.unchecked_transforms.remove(&id);
            }
        } else {
            let rows = Arc::make_mut(&mut self.rows);
            let mut removed = false;
            for id in ids {
                removed |= rows.remove_row(id);
            }
            if removed {
                rows.advance_removals();
            }
        }
    }

    pub fn clear(&mut self) {
        if self.staged.is_some() {
            let ids = self.entity_ids().collect::<Vec<_>>();
            self.remove_rows(ids);
        } else if !self.rows.index.is_empty() {
            let rows = Arc::make_mut(&mut self.rows);
            for (id, handle) in std::mem::take(&mut rows.index) {
                let removed = rows.world.despawn(handle);
                debug_assert!(removed);
                rows.record_removal(id);
            }
            rows.advance_removals();
            rows.components.clear();
        }
    }
    /// Build the built-in demo with its original motion.
    pub fn demo() -> Self {
        let mut world = Self::from_objects(vec![
            WorldObject {
                id: 1,
                parent: None,
                transform: transform([-1.35, 0.0, 0.0], [0.8; 3]),
                color: [80, 175, 245, 255],
            },
            WorldObject {
                id: 2,
                parent: Some(1),
                transform: transform([0.0, 2.0, 0.0], [0.32; 3]),
                color: [255, 182, 75, 255],
            },
            WorldObject {
                id: 3,
                parent: None,
                transform: transform([1.5, -0.15, 0.2], [0.65; 3]),
                color: [189, 128, 240, 255],
            },
            WorldObject {
                id: 4,
                parent: None,
                transform: transform([0.0, -1.0, 0.0], [4.0, 0.08, 3.0]),
                color: [79, 103, 111, 255],
            },
        ])
        .unwrap();
        for (id, velocity) in [
            (1, [0.18, 0.65, 0.0]),
            (2, [0.3, -0.7, 0.0]),
            (3, [0.25, -0.45, 0.0]),
        ] {
            world.object_data_mut(id).unwrap().angular_velocity = velocity;
        }
        world
    }
}

/// A world packet validated against the candidate at the point it was issued.
/// Dropping the buffer leaves the candidate untouched. Applying it makes the
/// writes visible to following systems inside the same tick.
pub struct PreparedWorldCommands<'a> {
    world: &'a mut EcsWorld,
    removed: Vec<u64>,
    objects: BTreeMap<u64, WorldObject>,
}

impl PreparedWorldCommands<'_> {
    pub fn apply(self) {
        if !self.removed.is_empty() {
            self.world.remove_rows(self.removed);
        }
        for (id, object) in self.objects {
            if let Some(data) = self.world.object_data_mut(id) {
                data.parent = object.parent;
                data.color = object.color;
                *self.world.transform_mut(id).unwrap() = object.transform;
            } else {
                self.world.spawn_object(object);
            }
        }
        if let Some(staged) = &mut self.world.staged {
            staged.mark_transforms_checked();
        }
    }
}

impl EcsWorld {
    /// Validate and apply a portable command batch to the current ECS view.
    pub fn apply_commands(&mut self, commands: &[WorldCommand]) -> Result<(), String> {
        self.prepare_commands(commands)?.apply();
        Ok(())
    }

    pub fn prepare_commands(
        &mut self,
        commands: &[WorldCommand],
    ) -> Result<PreparedWorldCommands<'_>, String> {
        use crate::control::{
            check_world_packet_size, edit_object_map, edit_object_values, validate_object_transform,
        };

        check_world_packet_size(commands)?;
        if let Some(staged) = &self.staged
            && staged.validated_base
            && commands
                .iter()
                .all(|command| command.value_target().is_some())
        {
            let mut edits = BTreeMap::new();
            for command in commands {
                let id = command.value_target().unwrap();
                let object = edits.entry(id).or_insert_with(|| self.object(id));
                edit_object_values(command, object.as_mut())?;
            }
            let ids = staged
                .unchecked_transform_ids()
                .chain(edits.keys().copied())
                .collect::<BTreeSet<_>>();
            for id in ids {
                if let Some(transform) = edits
                    .get(&id)
                    .and_then(Option::as_ref)
                    .map(|o| &o.transform)
                    .or_else(|| self.transform(id))
                {
                    validate_object_transform(id, transform)?;
                }
            }
            let objects = edits
                .into_iter()
                .map(|(id, object)| (id, object.unwrap()))
                .collect();
            return Ok(PreparedWorldCommands {
                world: self,
                removed: Vec::new(),
                objects,
            });
        }

        let mut objects = self
            .entity_ids()
            .map(|id| (id, self.object(id).unwrap()))
            .collect();
        edit_object_map(&mut objects, commands)?;
        let spawned: BTreeSet<_> = commands
            .iter()
            .filter_map(|command| match command {
                WorldCommand::Spawn { object } => Some(object.id),
                _ => None,
            })
            .collect();
        let removed = self
            .entity_ids()
            .filter(|id| !objects.contains_key(id) || spawned.contains(id))
            .collect();
        objects.retain(|id, object| {
            spawned.contains(id)
                || self.object(*id).is_none_or(|old| {
                    old.color != object.color
                        || old.parent != object.parent
                        || !crate::ComponentRegistry::TRANSFORM
                            .same_wire_value(&old.transform, &object.transform)
                })
        });
        Ok(PreparedWorldCommands {
            world: self,
            removed,
            objects,
        })
    }
}

impl WorldProvider for EcsWorld {
    fn objects(&self) -> Result<Vec<WorldObject>, WorldError> {
        Ok(self.objects_snapshot())
    }

    fn apply_commands(&mut self, commands: &[WorldCommand]) -> Result<(), WorldError> {
        EcsWorld::apply_commands(self, commands).map_err(WorldError::Commands)
    }

    fn step(&mut self, dt: f32, speed: f32) {
        self.time += dt;
        let amount = dt * speed;
        for id in self.entity_ids().collect::<Vec<_>>() {
            let velocity = self.object_data(id).unwrap().angular_velocity;
            let transform = self.transform_mut(id).unwrap();
            for (rotation, velocity) in transform.rotation.iter_mut().zip(velocity) {
                *rotation += velocity * amount;
            }
        }
    }

    fn snapshot(&self) -> SceneSnapshot {
        let mut cache = BTreeMap::new();
        let items = self
            .entity_ids()
            .map(|entity| RenderItem {
                id: entity,
                mesh: Mesh::cube(),
                model: self.render_model(entity, &mut cache),
                color: self.object_data(entity).unwrap().color,
                material: Material::default(),
            })
            .collect();
        SceneSnapshot {
            items,
            time: self.time,
        }
    }
}

/// GameState's legacy wire field remains a keyed entity DTO map. Runtime
/// storage is reconstructed only after IDs, hierarchy and references validate.
pub(crate) mod entity_dto_map {
    use super::EcsWorld;
    use crate::GameEntity;
    use serde::{
        Deserializer, Serializer,
        de::{Error, MapAccess, Visitor},
    };
    use std::collections::BTreeSet;

    pub fn serialize<S: Serializer>(world: &EcsWorld, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(
            world
                .entity_ids()
                .map(|id| (id, world.entity(id).expect("Canonical entity ID"))),
        )
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<EcsWorld, D::Error> {
        struct EntityMap;
        impl<'de> Visitor<'de> for EntityMap {
            type Value = EcsWorld;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a map of stable IDs to entity DTOs")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut ids = BTreeSet::new();
                let mut entities = vec![];
                while let Some((id, entity)) = map.next_entry::<u64, GameEntity>()? {
                    if id != entity.object.id || !ids.insert(id) {
                        return Err(A::Error::custom(
                            "Entity map keys must match unique stable object IDs",
                        ));
                    }
                    entities.push(entity);
                }
                EcsWorld::from_entities(entities).map_err(A::Error::custom)
            }
        }
        deserializer.deserialize_map(EntityMap)
    }
}

fn transform(translation: [f32; 3], scale: [f32; 3]) -> Transform {
    Transform {
        translation,
        scale,
        ..Transform::default()
    }
}

#[cfg(test)]
#[cfg(not(debug_assertions))]
#[path = "../../../examples/bench/s3_ecs_cost.rs"]
mod s3_cost;

#[cfg(test)]
mod tests {
    use super::*;
    fn entity(
        id: u64,
        parent: Option<u64>,
        components: &[(&str, serde_json::Value)],
    ) -> crate::GameEntity {
        crate::GameEntity {
            object: WorldObject {
                id,
                parent,
                transform: Transform::default(),
                color: [255; 4],
            },
            components: components
                .iter()
                .map(|(name, value)| ((*name).into(), value.clone()))
                .collect(),
            references: BTreeMap::new(),
        }
    }

    #[test]
    fn nested_tick_shares_prior_edits_and_merges_only_committed_rows() {
        let mut world =
            EcsWorld::from_entities(vec![entity(1, None, &[]), entity(2, None, &[])]).unwrap();
        let committed = world.checkpoint_tick(true);
        world.transform_mut(1).unwrap().translation[0] = 1.;
        let parent = world.checkpoint_tick(false);
        let staged = world.staged.as_ref().unwrap();
        assert!(staged.edits.is_empty());
        assert!(Arc::ptr_eq(
            staged.parent.as_ref().unwrap(),
            parent.staged.as_ref().unwrap().parent.as_ref().unwrap()
        ));
        world.transform_mut(1).unwrap().translation[0] = 99.;
        world
            .apply_commands(&[WorldCommand::Remove { id: 2 }])
            .unwrap();
        assert_eq!(world.entity_ids().collect::<Vec<_>>(), [1]);
        world = parent;
        world.compact_staged();
        assert!(world.staged.as_ref().unwrap().parent.is_none());
        assert_eq!(world.object(1).unwrap().transform.translation[0], 1.);
        assert_eq!(world.entity_ids().collect::<Vec<_>>(), [1, 2]);

        let parent = world.checkpoint_tick(false);
        world.transform_mut(2).unwrap().translation[0] = 2.;
        world.finish_tick(parent);
        assert!(world.staged.as_ref().unwrap().parent.is_none());
        world.finish_tick(committed);
        assert_eq!(world.object(1).unwrap().transform.translation[0], 1.);
        assert_eq!(world.object(2).unwrap().transform.translation[0], 2.);
    }

    #[test]
    fn unchecked_transforms_survive_dropped_packets_and_clear_after_validated_apply() {
        let mut world = EcsWorld::from_entities(vec![
            entity(1, None, &[("motion", serde_json::Value::Null)]),
            entity(2, None, &[]),
        ])
        .unwrap();
        let committed = world.checkpoint_tick(true);
        assert_eq!(
            world.edit_transforms_with_component(&[2, 1], "motion", |_, transform, _| {
                transform.translation[0] = f32::NAN;
            }),
            1
        );
        let repair = [WorldCommand::SetPosition {
            id: 1,
            position: [0.; 3],
        }];
        drop(world.prepare_commands(&repair).unwrap());
        let unrelated = [WorldCommand::SetColor {
            id: 2,
            color: [0; 4],
        }];
        assert_eq!(
            world.apply_commands(&unrelated).unwrap_err(),
            "Object 1 has a nonfinite transform"
        );
        let parent = world.checkpoint_tick(false);
        world.apply_commands(&repair).unwrap();
        world.finish_tick(parent);
        assert!(
            world
                .staged
                .as_ref()
                .unwrap()
                .unchecked_transform_ids()
                .next()
                .is_none()
        );
        world.apply_commands(&unrelated).unwrap();
        world.finish_tick(committed);
        assert_eq!(world.object(1).unwrap().transform.translation, [0.; 3]);
        assert_eq!(world.object(2).unwrap().color, [0; 4]);
    }

    #[test]
    fn publishing_a_row_marks_only_changed_native_components() {
        use bevy_ecs::change_detection::DetectChanges;

        let mut world = EcsWorld::from_objects(vec![
            entity(1, None, &[]).object,
            entity(2, None, &[]).object,
            entity(3, None, &[]).object,
            entity(4, None, &[]).object,
        ])
        .unwrap();
        let handles = [1, 2, 3].map(|id| world.rows.index[&id]);
        Arc::get_mut(&mut world.rows)
            .unwrap()
            .world
            .clear_trackers();
        let committed = world.checkpoint_tick(true);
        world
            .insert_component(1, "fixture", serde_json::Value::Null)
            .unwrap();
        world.transform_mut(2).unwrap().translation[0] = -0.;
        world
            .apply_commands(&[
                WorldCommand::SetColor {
                    id: 3,
                    color: [0; 4],
                },
                WorldCommand::Remove { id: 4 },
                WorldCommand::Spawn {
                    object: entity(5, None, &[]).object,
                },
            ])
            .unwrap();
        world.finish_tick(committed);
        let rows = Arc::get_mut(&mut world.rows).unwrap();
        assert_eq!(
            handles.map(|handle| rows
                .world
                .get_mut::<Transform>(handle)
                .unwrap()
                .is_changed()),
            [false, true, false]
        );
        assert_eq!(
            handles.map(|handle| rows
                .world
                .get_mut::<ObjectData>(handle)
                .unwrap()
                .is_changed()),
            [false, false, true]
        );
        assert!(!rows.index.contains_key(&4));
        let spawned = rows.index[&5];
        assert!(rows.world.get_mut::<Transform>(spawned).unwrap().is_added());
        assert_eq!(
            world.transform(2).unwrap().translation[0].to_bits(),
            (-0.0_f32).to_bits()
        );
    }

    #[test]
    fn repeated_remove_and_clear_bound_bevy_removal_messages() {
        let retained = [false, true].map(|clear| {
            let mut world = EcsWorld::from_objects(Vec::new()).unwrap();
            for _ in 0..1024 {
                world
                    .apply_commands(&[WorldCommand::Spawn {
                        object: entity(1, None, &[]).object,
                    }])
                    .unwrap();
                if clear {
                    world.clear();
                } else {
                    world
                        .apply_commands(&[WorldCommand::Remove { id: 1 }])
                        .unwrap();
                }
                assert!(world.is_empty());
            }
            let component = world.rows.world.component_id::<StableId>().unwrap();
            world
                .rows
                .world
                .removed_components()
                .get(component)
                .unwrap()
                .len()
        });
        assert!(
            retained.into_iter().all(|count| count <= 1),
            "Remove / clear: {retained:?}"
        );
    }

    #[test]
    fn component_column_queries_and_owned_dtos_keep_one_mutable_store() {
        let mut world = EcsWorld::from_entities(vec![
            entity(
                3,
                None,
                &[
                    ("a", serde_json::json!({"value":3})),
                    ("b", serde_json::json!({})),
                ],
            ),
            entity(1, None, &[("a", serde_json::json!({"value":1}))]),
            entity(2, None, &[("b", serde_json::json!({}))]),
        ])
        .unwrap();
        assert_eq!(world.entity_ids().collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(world.entities_with(&[]), vec![1, 2, 3]);
        assert_eq!(world.entities_with(&["a"]), vec![1, 3]);
        assert_eq!(world.entities_with(&["a", "b"]), vec![3]);
        assert!(world.entities_with(&["missing", "a"]).is_empty());
        world.references_mut(1).unwrap().insert("owner".into(), 3);
        let mut dto = world.entity(1).unwrap();
        dto.components.get_mut("a").unwrap()["value"] = serde_json::json!(99);
        assert_eq!(world.component(1, "a").unwrap()["value"], 1);
        world.component_mut(1, "a").unwrap()["value"] = serde_json::json!(4);
        assert_eq!(world.entity(1).unwrap().components["a"]["value"], 4);
        let lifetime = world.lifetime(3).unwrap();
        world
            .apply_commands(&[WorldCommand::SetTransform {
                id: 3,
                transform: Transform::default(),
            }])
            .unwrap();
        assert_eq!(world.lifetime(3), Some(lifetime));
        let mut child = world.entity(3).unwrap();
        child.object.parent = Some(2);
        world.upsert_entity(child).unwrap();
        assert_eq!(world.lifetime(3), Some(lifetime));
        assert_eq!(world.entities_with(&["a", "b"]), vec![3]);
        assert_eq!(world.references(1).unwrap()["owner"], 3);
        world.remove_component(3, "a");
        world
            .insert_component(2, "a", serde_json::json!({"value":2}))
            .unwrap();
        assert_eq!(world.entities_with(&["a", "b"]), vec![2]);
        assert!(
            world
                .insert_component(999, "a", serde_json::json!({}))
                .is_err()
        );
    }

    #[test]
    fn column_intersections_follow_committed_and_nested_staged_membership() {
        let null = serde_json::Value::Null;
        let mut world = EcsWorld::from_entities(
            (1..=64)
                .rev()
                .map(|id| {
                    let mut row = entity(id, None, &[("common", null.clone())]);
                    if id % 3 != 0 {
                        row.components.insert("dense".into(), null.clone());
                    }
                    if id == 2 || id == 61 {
                        row.components.insert("rare".into(), null.clone());
                    }
                    row
                })
                .collect(),
        )
        .unwrap();
        let compare = |world: &EcsWorld| {
            for required in [
                &[][..],
                &["common"],
                &["common", "dense"],
                &["dense", "common", "dense"],
                &["common", "rare", "dense"],
                &["common", "absent"],
                &["common", "staged_only"],
            ] {
                let expected: Vec<_> = world
                    .entity_ids()
                    .filter(|&id| {
                        required
                            .iter()
                            .all(|name| world.component(id, name).is_some())
                    })
                    .collect();
                assert_eq!(world.entities_with(required), expected, "{required:?}");
            }
        };
        compare(&world);
        world.remove_component(2, "dense");
        world.insert_component(3, "rare", null.clone()).unwrap();
        compare(&world);

        let committed = world.checkpoint_tick(true);
        world
            .apply_commands(&[WorldCommand::Remove { id: 61 }])
            .unwrap();
        world
            .upsert_entity(entity(
                80,
                None,
                &[
                    ("common", null.clone()),
                    ("dense", null.clone()),
                    ("rare", null.clone()),
                ],
            ))
            .unwrap();
        world
            .insert_component(4, "staged_only", null.clone())
            .unwrap();
        compare(&world);

        let parent = world.checkpoint_tick(false);
        world.remove_component(80, "common");
        world.insert_component(5, "staged_only", null).unwrap();
        compare(&world);
        world = parent;
        world.compact_staged();
        compare(&world);
        world.finish_tick(committed);
        compare(&world);
        assert_eq!(world.entities_with(&["common", "rare", "dense"]), [80]);
        assert_eq!(world.entities_with(&["common", "staged_only"]), [4]);
    }

    #[test]
    fn missing_component_write_preserves_shared_world_and_render_tracking() {
        let null = serde_json::Value::Null;
        let mut world = EcsWorld::from_entities(vec![
            entity(1, None, &[("motion", null.clone()), ("mesh", null.clone())]),
            entity(2, None, &[]),
        ])
        .unwrap();
        let shared = world.rows.clone();
        assert!(world.component_mut(2, "motion").is_none());
        assert!(Arc::ptr_eq(&world.rows, &shared));
        drop(shared);

        let writes = world.rows.render_writes;
        *world.component_mut(1, "motion").unwrap() = serde_json::json!({"offset":0.25});
        assert!(world.component_mut(2, "mesh").is_none());
        assert_eq!(world.rows.render_writes, writes);
        *world.component_mut(1, "mesh").unwrap() = serde_json::json!({"path":"mesh"});
        assert_eq!(world.rows.render_writes, writes + 1);

        let before = *world.transform(1).unwrap();
        let checkpoint = world.checkpoint_tick(true);
        world.transform_mut(1).unwrap().translation[1] = 0.25;
        assert_eq!(
            world.transform(1),
            world.object(1).as_ref().map(|row| &row.transform)
        );
        assert_ne!(world.transform(1), Some(&before));
        assert_eq!(checkpoint.transform(1), Some(&before));
        assert!(world.component_mut(2, "motion").is_none());
        assert!(world.edit(2).is_none());
    }

    #[test]
    fn batch_edits_preserve_input_order_and_track_each_committed_transform() {
        use serde_json::{Value, json};

        let mut world = EcsWorld::from_entities(vec![
            entity(1, None, &[("motion", json!({"literal.key": null}))]),
            entity(2, None, &[("mesh", Value::Null)]),
            entity(3, None, &[("motion", Value::Null)]),
        ])
        .unwrap();
        let shared = world.rows.clone();
        assert_eq!(
            world.edit_transforms_with_component(&[2, 99], "motion", |_, _, _| unreachable!()),
            0
        );
        assert!(Arc::ptr_eq(&world.rows, &shared));
        drop(shared);

        let mut cursor = RenderChangeCursor::default();
        world.read_render_changes(&mut cursor);
        let writes = world.rows.render_writes;
        let mut ids = Vec::new();
        let count = world.edit_transforms_with_component(
            &[3, 2, 1, 3, 99],
            "motion",
            |id, transform, value| {
                ids.push(id);
                transform.translation[1] += 1.;
                value["offset"] = Value::from(transform.translation[1]);
            },
        );
        assert_eq!(count, 3);
        assert_eq!(ids, [3, 1, 3]);
        assert_eq!(world.rows.render_writes, writes + 1);
        assert_eq!(world.transform(1).unwrap().translation[1], 1.);
        assert_eq!(world.transform(2).unwrap().translation[1], 0.);
        assert_eq!(world.transform(3).unwrap().translation[1], 2.);
        assert_eq!(
            world.component(1, "motion"),
            Some(&json!({"literal.key": null, "offset": 1.}))
        );
        assert_eq!(world.component(3, "motion"), Some(&json!({"offset": 2.})));
        let mut changed = world.read_render_changes(&mut cursor).changed;
        changed.sort_unstable();
        assert_eq!(changed, [1, 3]);

        let previous_tick = cursor.position.unwrap().tick;
        world.edit_transforms_with_component(&[2], "mesh", |_, _, value| {
            *value = json!({"path": "mesh"});
        });
        let rows = Arc::get_mut(&mut world.rows).unwrap();
        let changed_bindings = rows.world.last_change_tick_scope(previous_tick, |world| {
            let mut query = world.query_filtered::<&StableId, Changed<RenderBindings>>();
            query.iter(world).map(|id| id.0).collect::<Vec<_>>()
        });
        assert_eq!(changed_bindings, [2]);
    }

    #[test]
    fn stable_index_rejects_a_stale_handle_when_bevy_reuses_its_slot() {
        let target = 9_007_199_254_740_993;
        let child = 9_007_199_254_740_994;
        let mut observer = entity(1, None, &[]);
        observer.references.insert("target".into(), target);
        let mut world = EcsWorld::from_entities(vec![
            observer,
            entity(target, None, &[("marker", serde_json::Value::Null)]),
            entity(child, Some(target), &[]),
        ])
        .unwrap();
        let handle = world.rows.index[&target];
        let lifetime = world.lifetime(target);
        let remove = WorldCommand::Remove { id: target };
        let duplicate = WorldCommand::Spawn {
            object: entity(1, None, &[]).object,
        };
        assert!(world.apply_commands(&[remove.clone(), duplicate]).is_err());
        assert_eq!(world.rows.index[&target], handle);
        assert_eq!(world.lifetime(target), lifetime);
        assert_eq!(world.object(child).unwrap().parent, Some(target));
        assert_eq!(world.references(1).unwrap()["target"], target);
        assert_eq!(
            world.component(target, "marker"),
            Some(&serde_json::Value::Null)
        );

        // Keep the child so that the target is the first slot released.
        let mut child_dto = world.entity(child).unwrap();
        child_dto.object.parent = None;
        world.upsert_entity(child_dto).unwrap();

        // Bevy 0.20 publishes freed slots in groups of 128. Fill the group
        // through ordinary spawns and removals, then spawn enough to reuse it.
        const RECYCLE_BATCH: u64 = 128;
        let fillers = 2..=1 + RECYCLE_BATCH;
        let spawns = |ids: std::ops::RangeInclusive<u64>| {
            ids.map(|id| WorldCommand::Spawn {
                object: entity(id, None, &[]).object,
            })
            .collect::<Vec<_>>()
        };
        world.apply_commands(&spawns(fillers.clone())).unwrap();
        world.apply_commands(&[remove]).unwrap();
        world
            .apply_commands(
                &fillers
                    .map(|id| WorldCommand::Remove { id })
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        let mut replacements = (u64::MAX - (RECYCLE_BATCH - 1))..=u64::MAX;
        world.apply_commands(&spawns(replacements.clone())).unwrap();
        let replacement = replacements
            .find(|id| world.rows.index[id].index() == handle.index())
            .expect("A new stable ID should reuse the deleted target's Bevy slot");
        let new_handle = world.rows.index[&replacement];
        assert_ne!(handle.generation(), new_handle.generation());
        assert!(world.rows.world.get::<StableId>(handle).is_none());
        assert!(!world.contains(target));
        assert!(
            world
                .entity(world.references(1).unwrap()["target"])
                .is_none()
        );
        assert!(world.validate_references().is_err());
        assert_eq!(world.object(replacement).unwrap().id, replacement);
        assert!(world.entities_with(&["marker"]).is_empty());
    }
    #[test]
    fn subtree_reused_ids_clear_all_columns_references_and_motion_in_one_batch() {
        let mut parent = entity(1, None, &[("marker", serde_json::json!({"old":true}))]);
        parent.references.insert("child".into(), 2);
        let mut world = EcsWorld::from_entities(vec![
            parent,
            entity(2, Some(1), &[("marker", serde_json::json!({}))]),
        ])
        .unwrap();
        world.object_data_mut(1).unwrap().angular_velocity = [1., 0., 0.];
        let old_lifetimes = [world.lifetime(1).unwrap(), world.lifetime(2).unwrap()];
        world
            .apply_commands(&[
                WorldCommand::Remove { id: 1 },
                WorldCommand::Spawn {
                    object: entity(1, None, &[]).object,
                },
                WorldCommand::Spawn {
                    object: entity(2, Some(1), &[]).object,
                },
            ])
            .unwrap();
        assert!(world.entities_with(&["marker"]).is_empty());
        assert!(world.references(1).unwrap().is_empty());
        assert!(world.references(2).unwrap().is_empty());
        assert_ne!(world.lifetime(1), Some(old_lifetimes[0]));
        assert_ne!(world.lifetime(2), Some(old_lifetimes[1]));
        world.step(1., 1.);
        assert_eq!(world.object(1).unwrap().transform.rotation, [0.; 3]);
        world
            .insert_component(2, "fresh", serde_json::json!({"value":5}))
            .unwrap();
        world.references_mut(2).unwrap().insert("parent".into(), 1);
        let before = world.clone();
        let lifetime = world.lifetime(1);
        assert!(
            world
                .apply_commands(&[
                    WorldCommand::Remove { id: 1 },
                    WorldCommand::Spawn {
                        object: entity(3, Some(999), &[]).object
                    }
                ])
                .is_err()
        );
        assert_eq!(world, before);
        assert_eq!(world.lifetime(1), lifetime);
        world.clear();
        assert_eq!(world.lifetime(1), None);
        world
            .apply_commands(&[WorldCommand::Spawn {
                object: entity(1, None, &[]).object,
            }])
            .unwrap();
        assert_ne!(world.lifetime(1), lifetime);
    }

    #[test]
    fn authored_hierarchy_constructs_independent_stores_without_demo_velocity() {
        let data = vec![
            WorldObject {
                id: 20,
                parent: Some(10),
                transform: transform([0., 2., 0.], [1.; 3]),
                color: [255; 4],
            },
            WorldObject {
                id: 10,
                parent: None,
                transform: transform([3., 0., 0.], [2.; 3]),
                color: [200; 4],
            },
        ];
        let mut object = ObjectsWorld::from_objects(data.clone()).unwrap();
        let mut ecs = EcsWorld::from_objects(data).unwrap();
        let before = object.snapshot();
        object.step(2., 10.);
        ecs.step(2., 10.);
        assert_eq!(object.objects().unwrap(), ecs.objects().unwrap());
        assert_eq!(object.snapshot(), ecs.snapshot());
        assert_eq!(before.items, object.snapshot().items);
        let duplicate = vec![
            WorldObject {
                id: 1,
                parent: None,
                transform: Transform::default(),
                color: [255; 4]
            };
            2
        ];
        assert!(ObjectsWorld::from_objects(duplicate.clone()).is_err());
        assert!(EcsWorld::from_objects(duplicate).is_err());
    }

    #[test]
    fn constructors_and_spawn_reject_zero_ids_without_publishing_changes() {
        let rejected = "Entity IDs must be nonzero";
        let parent = WorldObject {
            id: 1,
            parent: None,
            transform: Transform::default(),
            color: [255; 4],
        };
        let valid = WorldObject {
            id: u64::MAX,
            parent: Some(parent.id),
            color: [42; 4],
            ..parent.clone()
        };
        let zero = WorldObject {
            id: 0,
            ..valid.clone()
        };
        let entity = |object| crate::GameEntity {
            object,
            components: Default::default(),
            references: Default::default(),
        };
        assert_eq!(
            EcsWorld::from_entities(vec![entity(parent.clone())])
                .unwrap()
                .objects()
                .unwrap(),
            vec![parent.clone()]
        );
        let results = [
            (
                "Objects constructor",
                ObjectsWorld::from_objects(vec![parent.clone(), zero.clone()]).err(),
            ),
            (
                "ECS objects constructor",
                EcsWorld::from_objects(vec![parent.clone(), zero.clone()]).err(),
            ),
            (
                "ECS entities constructor",
                EcsWorld::from_entities(vec![entity(zero.clone())]).err(),
            ),
            (
                "portable validation of a zero input row",
                crate::validate_world_commands(
                    &[zero.clone(), parent.clone()],
                    &[WorldCommand::Remove { id: 0 }],
                )
                .err(),
            ),
        ];
        let invalid = [
            WorldCommand::SetColor {
                id: parent.id,
                color: [0; 4],
            },
            WorldCommand::Spawn { object: zero },
        ];
        let mut transient = invalid.to_vec();
        transient.push(WorldCommand::Remove { id: 0 });
        let mut unchanged = Vec::new();
        for (scenario, batch) in [
            ("Spawn", invalid.as_slice()),
            ("Spawn then Remove", transient.as_slice()),
        ] {
            for (name, mut world) in [
                (
                    "Objects",
                    Box::new(ObjectsWorld::from_objects(vec![parent.clone()]).unwrap())
                        as Box<dyn WorldProvider>,
                ),
                (
                    "ECS",
                    Box::new(EcsWorld::from_objects(vec![parent.clone()]).unwrap()),
                ),
            ] {
                let good = [WorldCommand::Spawn {
                    object: valid.clone(),
                }];
                world.validate_commands(&good).unwrap();
                world.apply_commands(&good).unwrap();
                let before = world.objects().unwrap();
                assert_eq!(before, vec![parent.clone(), valid.clone()], "{name}");
                let snapshot = world.snapshot();
                let validation = world.validate_commands(batch);
                let application = world.apply_commands(batch);
                unchanged.push((
                    name,
                    scenario,
                    validation,
                    application,
                    world.objects().unwrap() == before && world.snapshot() == snapshot,
                ));
            }
        }
        assert!(
            results
                .iter()
                .all(|(_, result)| result.as_deref() == Some(rejected))
                && unchanged
                    .iter()
                    .all(|(_, _, validation, application, preserved)| {
                        validation.as_ref().is_err_and(|error| matches!(error, WorldError::Commands(message) if message == rejected))
                            && application.as_ref().is_err_and(|error| matches!(error, WorldError::Commands(message) if message == rejected))
                            && *preserved
                    }),
            "Constructors and input validation: {results:?}\nPer provider: {unchanged:?}",
        );
    }

    #[test]
    fn independent_worlds_extract_identical_hierarchical_scene_over_time() {
        let mut objects = ObjectsWorld::demo();
        let mut ecs = EcsWorld::demo();
        assert_eq!(objects.snapshot(), ecs.snapshot());
        let initial_child = objects.snapshot().items[1].model;
        for (dt, speed) in [(0.016, 1.0), (0.031, -0.4), (0.1, 2.0), (0.0, 0.0)] {
            objects.step(dt, speed);
            ecs.step(dt, speed);
            assert_eq!(objects.snapshot(), ecs.snapshot());
        }
        assert_ne!(objects.snapshot().items[1].model, initial_child);
        assert_eq!(objects.snapshot().items.len(), 4);
    }

    #[test]
    fn portable_commands_edit_both_stores_identically_and_roll_back_invalid_batches() {
        let mut objects = ObjectsWorld::demo();
        let mut ecs = EcsWorld::demo();
        let commands = vec![
            WorldCommand::SetPosition {
                id: 1,
                position: [2., 1., 0.],
            },
            WorldCommand::SetColor {
                id: 2,
                color: [10, 220, 80, 255],
            },
            WorldCommand::Spawn {
                object: WorldObject {
                    id: 9,
                    parent: Some(1),
                    transform: Transform {
                        translation: [0., 3., 0.],
                        ..Transform::default()
                    },
                    color: [200; 4],
                },
            },
        ];
        objects.apply_commands(&commands).unwrap();
        ecs.apply_commands(&commands).unwrap();
        objects.step(0.2, 1.);
        ecs.step(0.2, 1.);
        assert_eq!(objects.objects().unwrap(), ecs.objects().unwrap());
        assert_eq!(objects.snapshot(), ecs.snapshot());
        let before = objects.snapshot();
        let bad = [
            WorldCommand::SetColor {
                id: 1,
                color: [0; 4],
            },
            WorldCommand::Translate {
                id: 999,
                delta: [1., 0., 0.],
            },
        ];
        assert!(objects.apply_commands(&bad).is_err());
        assert!(ecs.apply_commands(&bad).is_err());
        assert_eq!(before, objects.snapshot());
        assert_eq!(before, ecs.snapshot());
        objects
            .apply_commands(&[WorldCommand::Remove { id: 1 }])
            .unwrap();
        ecs.apply_commands(&[WorldCommand::Remove { id: 1 }])
            .unwrap();
        assert_eq!(
            objects
                .objects()
                .unwrap()
                .iter()
                .map(|o| o.id)
                .collect::<Vec<_>>(),
            vec![3, 4]
        );
        assert_eq!(objects.snapshot(), ecs.snapshot());
    }

    #[test]
    fn invalid_parent_cycles_and_nonfinite_values_do_not_mutate_worlds() {
        for mut world in [
            Box::new(ObjectsWorld::demo()) as Box<dyn WorldProvider>,
            Box::new(EcsWorld::demo()),
        ] {
            let before = world.snapshot();
            let bad = WorldCommand::Spawn {
                object: WorldObject {
                    id: 10,
                    parent: Some(10),
                    transform: Transform::default(),
                    color: [255; 4],
                },
            };
            assert!(world.apply_commands(&[bad]).is_err());
            assert!(
                world
                    .apply_commands(&[WorldCommand::SetPosition {
                        id: 1,
                        position: [f32::NAN, 0., 0.]
                    }])
                    .is_err()
            );
            assert_eq!(before, world.snapshot());
        }
    }
    #[test]
    fn replacing_a_removed_id_spawns_a_new_object_without_previous_motion() {
        let mut objects = ObjectsWorld::demo();
        let mut ecs = EcsWorld::demo();
        let commands = [
            WorldCommand::Remove { id: 1 },
            WorldCommand::Spawn {
                object: WorldObject {
                    id: 1,
                    parent: None,
                    transform: Transform::default(),
                    color: [255; 4],
                },
            },
        ];
        objects.apply_commands(&commands).unwrap();
        ecs.apply_commands(&commands).unwrap();
        objects.step(1., 1.);
        ecs.step(1., 1.);
        assert_eq!(
            objects.objects().unwrap()[0].transform,
            Transform::default()
        );
        assert_eq!(objects.snapshot(), ecs.snapshot());
    }
}
