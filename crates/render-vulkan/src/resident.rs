use super::{
    Buffer, FrameWork, MaterialSet, VulkanError, VulkanRenderer, materials::MaterialUploads,
};
use ash::vk;
use module_engine_core::{Mesh, RenderItem, RenderScene, SceneSnapshot, SceneSync, SceneUpdate};
use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    mem::size_of,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

pub(crate) const VERTEX_SHADER: &str = include_str!("../../../assets/gpu/resident_scene.hlsl");

pub(crate) fn culled_vertex_source() -> String {
    format!("#define MODULE_ENGINE_GPU_CULLING 1\n{VERTEX_SHADER}")
}

/// The R1.6 receiver remains authoritative for order, contents and revisions.
/// A busy or failed submission leaves its pending edits available for retry.
pub struct ResidentScene {
    id: u64,
    scene: RenderScene,
    rebuild: bool,
    moved: Vec<usize>,
    changed_groups: HashSet<u64>,
}

impl Default for ResidentScene {
    fn default() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            scene: RenderScene::default(),
            rebuild: true,
            moved: Vec::new(),
            changed_groups: HashSet::new(),
        }
    }
}

impl ResidentScene {
    pub fn sync(&self) -> SceneSync {
        self.scene.sync()
    }

    pub fn snapshot(&self) -> Result<&SceneSnapshot, String> {
        self.scene.snapshot()
    }

    pub fn apply(&mut self, update: SceneUpdate<'_>) -> Result<(), String> {
        use module_engine_core::SceneChangeKind::Edited;
        let changes = match &update {
            SceneUpdate::Reset { .. } => None,
            SceneUpdate::Delta { changes, .. } => Some(*changes),
        };
        let changes_layout = |kind| {
            !matches!(
                kind,
                Edited {
                    mesh_changed: false,
                    material_changed: false,
                    ..
                }
            )
        };
        let incremental_model = !self.rebuild
            && self.changed_groups.is_empty()
            && changes
                .is_some_and(|changes| changes.iter().all(|change| !changes_layout(change.kind)));
        self.scene.apply_with_edits(update, |range| {
            if incremental_model {
                self.moved.extend(range);
            }
        })?;
        if let Some(changes) = changes {
            if !self.rebuild && !incremental_model {
                self.changed_groups.extend(
                    changes
                        .iter()
                        .filter(|change| changes_layout(change.kind))
                        .map(|change| change.id),
                );
                self.moved.clear();
            }
        } else {
            self.require_reconciliation();
        }
        // Failed/busy submissions can accumulate edits. Reconcile against the
        // current list if the journal would exceed the size of the scene.
        if self.moved.len() + self.changed_groups.len() > self.scene.snapshot()?.items.len() {
            self.require_reconciliation();
        }
        Ok(())
    }

    pub(super) fn require_reconciliation(&mut self) {
        self.rebuild = true;
        self.moved.clear();
        self.changed_groups.clear();
    }

    fn accepted(&mut self) {
        self.rebuild = false;
        self.moved.clear();
        self.changed_groups.clear();
    }
}

#[derive(Clone, Copy, Default)]
struct MeshRange {
    first: u32,
    count: u32,
    extent: f32,
    bounds: u32,
}

type MeshEntries = HashMap<u64, Vec<(Mesh, MeshRange)>>;

#[derive(Default)]
struct MeshPool {
    meshes: MeshEntries,
    vertex_count: usize,
    index_count: usize,
    bounds_count: usize,
    corner_count: usize,
}

impl MeshPool {
    fn commit(&mut self, upload: MeshUpload) {
        self.vertex_count += upload.vertices.len();
        self.index_count += upload.indices.len();
        self.bounds_count += upload.bounds.len();
        self.corner_count += upload.corners.len();
        for (key, entries) in upload.meshes {
            self.meshes.entry(key).or_default().extend(entries);
        }
    }
}

/// Only a successful submission makes new mesh positions and indices part of
/// the device pool. A fallback or pre-submit error drops its speculative suffix.
#[derive(Default)]
struct MeshUpload {
    meshes: MeshEntries,
    vertices: Vec<[u32; 5]>,
    indices: Vec<[u32; 1]>,
    bounds: Vec<[u32; 8]>,
    corners: Vec<[u32; 1]>,
}

fn positions(mesh: &Mesh) -> impl Iterator<Item = u32> + '_ {
    mesh.vertices
        .iter()
        .flatten()
        .chain(mesh.texcoords.iter().flatten())
        .map(|v| v.to_bits())
}

impl MeshUpload {
    fn get(
        &mut self,
        pool: &MeshPool,
        mesh: &Mesh,
        limit: u64,
        gpu_culling: bool,
    ) -> Result<MeshRange, VulkanError> {
        let mut hash = DefaultHasher::new();
        mesh.vertices.len().hash(&mut hash);
        mesh.texcoords.len().hash(&mut hash);
        for value in positions(mesh) {
            value.hash(&mut hash);
        }
        mesh.triangles.hash(&mut hash);
        let key = hash.finish();
        if let Some((_, range)) = pool
            .meshes
            .get(&key)
            .into_iter()
            .chain(self.meshes.get(&key))
            .flatten()
            .find(|(stored, _)| {
                stored.vertices.len() == mesh.vertices.len()
                    && stored.texcoords.len() == mesh.texcoords.len()
                    && stored.triangles == mesh.triangles
                    && positions(stored).eq(positions(mesh))
            })
        {
            return Ok(*range);
        }
        let valid = |index: usize| {
            mesh.vertices
                .get(index)
                .is_some_and(|p| p.iter().all(|v| v.is_finite()))
        };
        let triangles = mesh
            .triangles
            .iter()
            .filter(|tri| tri.iter().all(|i| valid(*i)));
        let cap = limit as usize;
        let vertices = pool.vertex_count + self.vertices.len();
        let indices = pool.index_count + self.indices.len();
        if vertices.saturating_add(mesh.vertices.len()) > cap / 20
            || indices.saturating_add(triangles.clone().count().saturating_mul(3)) > cap / 4
        {
            return Err(VulkanError::SceneBufferLimit);
        }
        let base = vertices as u32;
        self.vertices
            .extend(mesh.vertices.iter().enumerate().map(|(i, &[x, y, z])| {
                let [u, v] = mesh.texcoords.get(i).copied().unwrap_or_default();
                [x, y, z, u, v].map(f32::to_bits)
            }));
        let first = indices as u32;
        self.indices
            .extend(triangles.flatten().map(|i| [base + *i as u32]));
        let count = (pool.index_count + self.indices.len()) as u32 - first;
        let bounds = (pool.bounds_count + self.bounds.len()) as u32;
        if gpu_culling {
            if u64::from(bounds) * 32 + 32 > limit {
                return Err(VulkanError::SceneBufferLimit);
            }
            // Bound the same valid triangles the resident vertex stage draws.
            let mut lo = [f32::INFINITY; 3];
            let mut hi = [f32::NEG_INFINITY; 3];
            for vertex in mesh
                .triangles
                .iter()
                .filter(|tri| tri.iter().all(|i| valid(*i)))
                .flatten()
            {
                for (axis, value) in mesh.vertices[*vertex].into_iter().enumerate() {
                    lo[axis] = lo[axis].min(value);
                    hi[axis] = hi[axis].max(value);
                }
            }
            let [x, y, z] = lo;
            let [a, b, c] = hi;
            self.bounds
                .push([x, y, z, 0., a, b, c, 0.].map(f32::to_bits));
            // The color vertex shader addresses a triangle corner so it can
            // preserve the flat face normal and the published triangle ID.
            // One ascending index stream serves every mesh via vertexOffset.
            let uploaded = (pool.corner_count + self.corners.len()) as u32;
            self.corners.extend((uploaded..count).map(|index| [index]));
        }
        let range = MeshRange {
            first,
            count,
            extent: mesh
                .vertices
                .iter()
                .flatten()
                .copied()
                .filter(|v| v.is_finite())
                .map(f32::abs)
                .fold(0., f32::max),
            bounds,
        };
        self.meshes
            .entry(key)
            .or_default()
            .push((mesh.clone(), range));
        Ok(range)
    }
}

type Instance = [u32; 16];
type Color = [u32; 4];
type Draw = [u32; 4];

// The expanded stream drops individual triangles with non-finite world
// positions before assigning IDs. Use that path if any intermediate affine
// result could overflow, so raster order, IDs and submitted counts still agree.
fn safe_model(item: &RenderItem, extent: f32) -> bool {
    let extent = f64::from(extent);
    item.model[..3].iter().all(|row| {
        let [x, y, z, translation] = row.map(|value| f64::from(value.abs()));
        (x + y + z) * extent + translation <= f64::from(f32::MAX) / 2.
    })
}

fn model(row: &mut Instance, item: &RenderItem) {
    for (word, value) in row[..12]
        .iter_mut()
        .zip(item.model.iter().take(3).flatten())
    {
        *word = value.to_bits();
    }
}

// Stable within an accepted object group. R1.6 replacements preserve the
// published order of primitives even when their number changes.
type Drawable = (u64, usize);

#[derive(Clone, Copy, Default)]
struct GeometryRow {
    mesh: MeshRange,
    triangle_origin: u32,
}

#[derive(Clone, Copy)]
pub(super) struct DrawRun {
    first: usize,
    len: usize,
    object_bias: u32,
    triangle_bias: u32,
}

#[derive(Default)]
struct Rows {
    instances: Vec<Instance>,
    colors: Vec<Color>,
    draws: Vec<Draw>,
    images: Vec<[u32; 1]>,
    geometry: Vec<GeometryRow>,
    slots: HashMap<Drawable, usize>,
    order: Vec<usize>,
    runs: Arc<[DrawRun]>,
    cull_runs: Vec<[u32; 4]>,
    triangles: usize,
}

impl Rows {
    fn place(&self, scene: &SceneSnapshot) -> (Vec<Drawable>, Vec<usize>) {
        let keys: Vec<_> = scene
            .items
            .iter()
            .scan((None, 0), |state, item| {
                let ordinal = if state.0 == Some(item.id) {
                    state.1 + 1
                } else {
                    0
                };
                *state = (Some(item.id), ordinal);
                Some((item.id, ordinal))
            })
            .collect();
        let mut live = vec![false; self.instances.len()];
        let placed: Vec<_> = keys
            .iter()
            .map(|key| {
                self.slots
                    .get(key)
                    .copied()
                    .inspect(|slot| live[*slot] = true)
            })
            .collect();
        let holes = live
            .iter()
            .enumerate()
            .filter_map(|(slot, &used)| (!used).then_some(slot));
        let mut free = holes.chain(live.len()..);
        let order = placed
            .into_iter()
            .map(|slot| slot.unwrap_or_else(|| free.next().unwrap()))
            .collect();
        (keys, order)
    }
}

#[derive(Clone)]
pub(super) struct Buffers {
    pub vertices: Arc<Buffer>,
    pub indices: Arc<Buffer>,
    pub instances: Arc<Buffer>,
    pub colors: Arc<Buffer>,
    images: Arc<Buffer>,
    pub draws: Arc<Buffer>,
    pub culling: Option<CullingInputs>,
}

#[derive(Clone)]
pub(super) struct CullingInputs {
    pub bounds: Arc<Buffer>,
    pub corners: Arc<Buffer>,
    pub runs: Arc<Buffer>,
}

impl Buffers {
    pub(super) fn bind(&self) -> [&Buffer; 4] {
        [&self.vertices, &self.indices, &self.instances, &self.colors]
    }

    pub(super) fn draw(
        &self,
        device: &ash::Device,
        command: vk::CommandBuffer,
        layout: vk::PipelineLayout,
        runs: &[DrawRun],
        max_draws: u32,
    ) -> usize {
        let stride = size_of::<vk::DrawIndirectCommand>() as u32;
        let mut calls = 0;
        for run in runs {
            let words = [
                run.object_bias,
                run.triangle_bias,
                super::materials::FIRST_MATERIAL_ROW.wrapping_sub(run.object_bias),
            ]
            .map(u32::to_ne_bytes);
            // The last three stock-pass constants restore the published IDs
            // and map the published object ID back to its physical material slot.
            // SAFETY: The command buffer is recording on this device. Bytes
            // 116..128 and the specified stages are covered by the bound layout.
            unsafe {
                device.cmd_push_constants(
                    command,
                    layout,
                    vk::ShaderStageFlags::VERTEX
                        | vk::ShaderStageFlags::FRAGMENT
                        | vk::ShaderStageFlags::COMPUTE,
                    116,
                    words.as_flattened(),
                );
            }
            let mut count = run.len;
            let mut offset = run.first as u64 * u64::from(stride);
            while count != 0 {
                let batch = count.min(max_draws as usize);
                // SAFETY: Recording is inside the geometry pass. This batch
                // fits the indirect buffer and device draw limit. The flight
                // retains the buffer until GPU completion.
                unsafe {
                    device.cmd_draw_indirect(
                        command,
                        self.draws.buffer,
                        offset,
                        batch as u32,
                        stride,
                    );
                }
                offset += batch as u64 * u64::from(stride);
                count -= batch;
                calls += 1;
            }
        }
        calls
    }
}

enum RowsUpdate {
    Full(Box<Rows>),
    Moved(Vec<(usize, Instance)>),
    None,
}

pub(super) struct SceneUpload {
    pub buffers: Buffers,
    pub materials: Arc<MaterialSet>,
    pub uploads: MaterialUploads,
    pub triangles: usize,
    pub(super) runs: Arc<[DrawRun]>,
    rows: RowsUpdate,
    meshes: MeshUpload,
}

#[derive(Default)]
pub(super) struct GpuScene {
    pool: MeshPool,
    buffers: Option<Buffers>,
    rows: Rows,
    materials: Option<Arc<MaterialSet>>,
    scene: Option<u64>,
}

impl GpuScene {
    pub(super) fn prepare(
        &self,
        renderer: &VulkanRenderer,
        source: &ResidentScene,
    ) -> Result<Option<SceneUpload>, VulkanError> {
        let scene = source.snapshot().map_err(VulkanError::Resource)?;
        let incremental = self.scene == Some(source.id) && !source.rebuild;
        if incremental && source.changed_groups.is_empty() {
            let mut rows = Vec::with_capacity(source.moved.len());
            for &ordinal in &source.moved {
                let item = &scene.items[ordinal];
                let i = self.rows.order[ordinal];
                if !safe_model(item, self.rows.geometry[i].mesh.extent) {
                    return Ok(None);
                }
                let mut row = self.rows.instances[i];
                model(&mut row, item);
                if row != self.rows.instances[i] {
                    rows.push((i, row));
                }
            }
            rows.sort_unstable_by_key(|(i, _)| *i);
            rows.dedup_by_key(|(i, _)| *i);
            return Ok(Some(SceneUpload {
                buffers: self.buffers.clone().unwrap(),
                materials: self.materials.clone().unwrap(),
                uploads: MaterialUploads::default(),
                triangles: self.rows.triangles,
                runs: self.rows.runs.clone(),
                meshes: MeshUpload::default(),
                rows: if rows.is_empty() {
                    RowsUpdate::None
                } else {
                    RowsUpdate::Moved(rows)
                },
            }));
        }

        if scene.items.len() > 1 << 24 {
            return Ok(None);
        }
        let limit = renderer.storage_buffer_limit;
        if scene.items.len() as u64 * size_of::<Instance>() as u64 > limit {
            return Ok(None);
        }
        let limit = limit.min(u64::from(u32::MAX));
        let (keys, order) = self.rows.place(scene);
        let items = self
            .rows
            .instances
            .len()
            .max(order.iter().max().map_or(0, |slot| slot + 1));
        if items as u64 * size_of::<Instance>() as u64 > limit {
            return Ok(None);
        }
        let prior = self.buffers.as_ref();
        let buffer = |previous: Option<&Arc<Buffer>>, bytes: usize, usage| {
            let bytes = bytes as u64;
            if let Some(previous) = previous.filter(|buffer| buffer.size >= bytes) {
                return Ok(previous.clone());
            }
            let capacity = bytes.max(4).next_power_of_two().min(limit);
            renderer.device_buffer(capacity, usage).map(Arc::new)
        };
        let storage = vk::BufferUsageFlags::STORAGE_BUFFER;
        let index_usage = vk::BufferUsageFlags::INDEX_BUFFER;
        let indirect = vk::BufferUsageFlags::INDIRECT_BUFFER;
        let gpu_culling = renderer.capabilities.gpu_frustum_culling;
        let image_rows = buffer(prior.map(|b| &b.images), (items + 1) * 4, storage)?;
        let Some((materials, uploads)) = renderer.prepare_resident_materials(
            scene,
            self.materials.as_ref(),
            image_rows.clone(),
        )?
        else {
            return Ok(None);
        };

        // Vacant rows keep their payload until they are reused. Ordered draw
        // runs omit them, so deleting an object has no scene-buffer writes.
        let mut rows = Rows {
            instances: self.rows.instances.clone(),
            colors: self.rows.colors.clone(),
            draws: self.rows.draws.clone(),
            images: self.rows.images.clone(),
            geometry: self.rows.geometry.clone(),
            slots: HashMap::with_capacity(order.len()),
            order,
            ..Default::default()
        };
        rows.instances.resize(items, [0; 16]);
        rows.colors.resize(items, [0; 4]);
        rows.draws.resize(items, [0; 4]);
        rows.images.resize(items + 1, [0]);
        rows.geometry.resize(items, GeometryRow::default());
        let mut meshes = MeshUpload::default();
        let mut runs: Vec<DrawRun> = Vec::new();
        for (i, ((item, key), &slot)) in scene.items.iter().zip(keys).zip(&rows.order).enumerate() {
            let old = self
                .rows
                .slots
                .get(&key)
                .map(|&slot| self.rows.geometry[slot]);
            let mesh = match old {
                Some(old) if incremental && !source.changed_groups.contains(&item.id) => old.mesh,
                _ => meshes.get(&self.pool, &item.mesh, limit, gpu_culling)?,
            };
            if !safe_model(item, mesh.extent) {
                return Ok(None);
            }
            let triangle_origin = old.map_or(rows.triangles as u32, |old| old.triangle_origin);
            let object_bias = (i as u32).wrapping_sub(slot as u32);
            let triangle_bias = (rows.triangles as u32).wrapping_sub(triangle_origin);
            if let Some(run) = runs.last_mut().filter(|run| {
                run.first + run.len == slot
                    && run.object_bias == object_bias
                    && run.triangle_bias == triangle_bias
            }) {
                run.len += 1;
            } else {
                runs.push(DrawRun {
                    first: slot,
                    len: 1,
                    object_bias,
                    triangle_bias,
                });
            }
            rows.geometry[slot] = GeometryRow {
                mesh,
                triangle_origin,
            };
            rows.slots.insert(key, slot);
            let mut row = [0; 16];
            model(&mut row, item);
            row[12] = (triangle_origin + 1).wrapping_sub(mesh.first / 3);
            row[13] = slot as u32;
            row[14] = mesh.bounds;
            rows.triangles += mesh.count as usize / 3;
            rows.instances[slot] = row;
            rows.colors[slot] = std::array::from_fn(|c| {
                (item.color[c] as f32 / 255.0 * item.material.base_color[c]).to_bits()
            });
            rows.draws[slot] = [mesh.count, 1, mesh.first, slot as u32];
            rows.images[slot + 1] = [materials.index(item.material.base_color_texture.as_ref())];
        }
        if gpu_culling {
            rows.cull_runs.extend(runs.iter().map(|run| {
                let first_slot = run.first as u32;
                [
                    first_slot.wrapping_add(run.object_bias),
                    first_slot,
                    run.triangle_bias,
                    0,
                ]
            }));
        }
        rows.runs = runs.into();
        let prior_culling = prior.and_then(|b| b.culling.as_ref());
        let culling = if gpu_culling {
            Some(CullingInputs {
                bounds: buffer(
                    prior_culling.map(|b| &b.bounds),
                    (self.pool.bounds_count + meshes.bounds.len()) * 32,
                    storage,
                )?,
                corners: buffer(
                    prior_culling.map(|b| &b.corners),
                    (self.pool.corner_count + meshes.corners.len()) * 4,
                    index_usage,
                )?,
                runs: buffer(
                    prior_culling.map(|b| &b.runs),
                    rows.cull_runs.len() * 16,
                    storage,
                )?,
            })
        } else {
            None
        };
        let buffers = Buffers {
            vertices: buffer(
                prior.map(|b| &b.vertices),
                (self.pool.vertex_count + meshes.vertices.len()) * 20,
                storage,
            )?,
            indices: buffer(
                prior.map(|b| &b.indices),
                (self.pool.index_count + meshes.indices.len()) * 4,
                if gpu_culling {
                    storage | index_usage
                } else {
                    storage
                },
            )?,
            instances: buffer(
                prior.map(|b| &b.instances),
                items * size_of::<Instance>(),
                storage,
            )?,
            colors: buffer(
                prior.map(|b| &b.colors),
                items * size_of::<Color>(),
                storage,
            )?,
            draws: buffer(
                prior.map(|b| &b.draws),
                items * size_of::<Draw>(),
                if gpu_culling {
                    indirect | storage
                } else {
                    indirect
                },
            )?,
            images: image_rows,
            culling,
        };
        Ok(Some(SceneUpload {
            buffers,
            materials,
            uploads,
            triangles: rows.triangles,
            runs: rows.runs.clone(),
            rows: RowsUpdate::Full(Box::new(rows)),
            meshes,
        }))
    }

    pub(super) fn record(
        &self,
        next: &SceneUpload,
        device: &ash::Device,
        command: vk::CommandBuffer,
        work: &mut FrameWork,
    ) {
        if matches!(next.rows, RowsUpdate::None) {
            return;
        }
        // Submissions use one graphics queue. A prior shader/indirect/index
        // read must finish before an in-place write, and the new rows must be
        // available before ANY following draw can read the shared buffers.
        let consumers = vk::PipelineStageFlags::VERTEX_SHADER
            | vk::PipelineStageFlags::FRAGMENT_SHADER
            | vk::PipelineStageFlags::COMPUTE_SHADER
            | vk::PipelineStageFlags::DRAW_INDIRECT
            | vk::PipelineStageFlags::VERTEX_INPUT;
        let reads = vk::AccessFlags::SHADER_READ
            | vk::AccessFlags::INDIRECT_COMMAND_READ
            | vk::AccessFlags::INDEX_READ;
        barrier(
            device,
            command,
            consumers | vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::TRANSFER,
            reads | vk::AccessFlags::TRANSFER_WRITE,
            vk::AccessFlags::TRANSFER_READ | vk::AccessFlags::TRANSFER_WRITE,
        );

        let update = BufferUpdates { device, command };
        let before = self.buffers.as_ref();
        let after = &next.buffers;
        match &next.rows {
            RowsUpdate::Full(rows) => {
                work.mesh_index_write_bytes = update.append(
                    before.map(|b| &*b.indices),
                    &after.indices,
                    &next.meshes.indices,
                    self.pool.index_count,
                );
                work.vertex_write_bytes = update.append(
                    before.map(|b| &*b.vertices),
                    &after.vertices,
                    &next.meshes.vertices,
                    self.pool.vertex_count,
                );
                work.instance_write_bytes = update.changed(
                    &after.instances,
                    before.map(|b| (&*b.instances, self.rows.instances.as_slice())),
                    &rows.instances,
                );
                work.material_row_write_bytes = update.changed(
                    &after.colors,
                    before.map(|b| (&*b.colors, self.rows.colors.as_slice())),
                    &rows.colors,
                ) + update.changed(
                    &after.images,
                    before.map(|b| (&*b.images, self.rows.images.as_slice())),
                    &rows.images,
                );
                work.draw_write_bytes = update.changed(
                    &after.draws,
                    before.map(|b| (&*b.draws, self.rows.draws.as_slice())),
                    &rows.draws,
                );
                if let Some(culling) = &after.culling {
                    let prior = before.and_then(|b| b.culling.as_ref());
                    work.culling_input_write_bytes = update.append(
                        prior.map(|b| &*b.bounds),
                        &culling.bounds,
                        &next.meshes.bounds,
                        self.pool.bounds_count,
                    ) + update.changed(
                        &culling.runs,
                        prior.map(|b| (&*b.runs, self.rows.cull_runs.as_slice())),
                        &rows.cull_runs,
                    );
                    work.mesh_index_write_bytes += update.append(
                        prior.map(|b| &*b.corners),
                        &culling.corners,
                        &next.meshes.corners,
                        self.pool.corner_count,
                    );
                }
            }
            RowsUpdate::Moved(rows) => {
                let mut data = Vec::new();
                for group in rows.chunk_by(|(a, _), (b, _)| *b == a + 1) {
                    data.clear();
                    data.extend(group.iter().map(|(_, words)| *words));
                    work.instance_write_bytes += update.write(&after.instances, group[0].0, &data);
                }
            }
            RowsUpdate::None => unreachable!(),
        }
        barrier(
            device,
            command,
            vk::PipelineStageFlags::TRANSFER,
            consumers,
            vk::AccessFlags::TRANSFER_WRITE,
            reads,
        );
    }

    pub(super) fn commit(&mut self, next: SceneUpload, source: &mut ResidentScene) {
        match next.rows {
            RowsUpdate::Full(rows) => {
                self.rows = *rows;
                self.pool.commit(next.meshes);
            }
            RowsUpdate::Moved(rows) => {
                for (i, row) in rows {
                    self.rows.instances[i] = row;
                }
            }
            RowsUpdate::None => {}
        }
        self.buffers = Some(next.buffers);
        self.materials = Some(next.materials);
        self.scene = Some(source.id);
        source.accepted();
    }

    pub(super) fn prior_buffers(&self) -> Option<Buffers> {
        self.buffers.clone()
    }
}

struct BufferUpdates<'a> {
    device: &'a ash::Device,
    command: vk::CommandBuffer,
}

impl BufferUpdates<'_> {
    fn write<const N: usize>(&self, buffer: &Buffer, start: usize, rows: &[[u32; N]]) -> usize {
        let words = rows.as_flattened();
        let bytes = std::mem::size_of_val(words);
        // SAFETY: The initialized u32 slice has no padding. The byte view
        // covers exactly that allocation's slice and is used while it is live.
        let data = unsafe { std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), bytes) };
        let mut offset = (start * N * size_of::<u32>()) as u64;
        for chunk in data.chunks(65_536) {
            // SAFETY: The command buffer is recording outside rendering.
            // The caller sized this transfer destination for the rows, and
            // the word layout gives four-byte-aligned offsets and sizes.
            unsafe {
                self.device
                    .cmd_update_buffer(self.command, buffer.buffer, offset, chunk)
            };
            offset += chunk.len() as u64;
        }
        bytes
    }

    fn append<const N: usize>(
        &self,
        before: Option<&Buffer>,
        after: &Buffer,
        rows: &[[u32; N]],
        uploaded: usize,
    ) -> usize {
        if let Some(before) = before.filter(|b| b.buffer != after.buffer && uploaded != 0) {
            let region = vk::BufferCopy::default().size((uploaded * size_of::<[u32; N]>()) as u64);
            // SAFETY: This records outside rendering. The committed prefix
            // fits both distinct transfer buffers, which the flight retains.
            unsafe {
                self.device
                    .cmd_copy_buffer(self.command, before.buffer, after.buffer, &[region])
            };
        }
        self.write(after, uploaded, rows)
    }

    fn changed<const N: usize>(
        &self,
        buffer: &Buffer,
        before: Option<(&Buffer, &[[u32; N]])>,
        after: &[[u32; N]],
    ) -> usize {
        let Some((old_buffer, before)) = before else {
            return self.write(buffer, 0, after);
        };
        let mut bytes = 0;
        let mut index = 0;
        // On capacity growth copy only unchanged regions from the old device
        // buffer. The CPU updates the other, disjoint regions exactly once.
        while index < after.len() {
            let start = index;
            let unchanged = before.get(index) == after.get(index);
            index += 1;
            while index < after.len() && (before.get(index) == after.get(index)) == unchanged {
                index += 1;
            }
            if !unchanged {
                bytes += self.write(buffer, start, &after[start..index]);
            } else if old_buffer.buffer != buffer.buffer {
                let offset = (start * size_of::<[u32; N]>()) as u64;
                let size = ((index - start) * size_of::<[u32; N]>()) as u64;
                let region = vk::BufferCopy::default()
                    .src_offset(offset)
                    .dst_offset(offset)
                    .size(size);
                // SAFETY: These unchanged rows fit both distinct transfer
                // buffers. The command buffer is recording outside rendering
                // and the flight retains both buffers through completion.
                unsafe {
                    self.device.cmd_copy_buffer(
                        self.command,
                        old_buffer.buffer,
                        buffer.buffer,
                        &[region],
                    );
                }
            }
        }
        bytes
    }
}

fn barrier(
    device: &ash::Device,
    command: vk::CommandBuffer,
    src: vk::PipelineStageFlags,
    dst: vk::PipelineStageFlags,
    available: vk::AccessFlags,
    visible: vk::AccessFlags,
) {
    let memory = vk::MemoryBarrier::default()
        .src_access_mask(available)
        .dst_access_mask(visible);
    // SAFETY: The command buffer is recording outside rendering on the
    // graphics queue, and the access masks match the supplied pipeline stages.
    unsafe {
        device.cmd_pipeline_barrier(
            command,
            src,
            dst,
            vk::DependencyFlags::empty(),
            &[memory],
            &[],
            &[],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RenderPath, default_graph};
    use module_engine_core::{
        Camera, ObjectsWorld, RenderOverrides, SceneChange, SceneChangeKind, WorldProvider,
    };
    use std::collections::BTreeMap;

    #[test]
    fn gpu_culling_draws_zero_visible_items_and_respects_the_indirect_limit() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let mut renderer = VulkanRenderer::new().unwrap();
        if !renderer.capabilities().gpu_frustum_culling {
            return;
        }
        renderer.set_culling_stats(true);
        let mut snapshot = ObjectsWorld::demo().snapshot();
        let objects = snapshot.items.len();
        assert!(objects > 1);
        for item in &mut snapshot.items {
            item.model[0][3] += 10_000.;
        }
        let mut scene = ResidentScene::default();
        scene
            .apply(SceneUpdate::Reset {
                revision: 0,
                snapshot,
            })
            .unwrap();
        let graph = default_graph(RenderPath::Forward);
        let draw = |renderer: &mut VulkanRenderer, scene: &mut ResidentScene| {
            renderer
                .render_scene(
                    super::super::SceneInput::Retained(scene),
                    Camera::default(),
                    96,
                    72,
                    &graph,
                    &BTreeMap::new(),
                    &RenderOverrides::default(),
                )
                .unwrap()
        };
        let culled = draw(&mut renderer, &mut scene);
        assert_eq!(culled.stats.shaded_pixels, 0);
        let work = renderer.last_frame_work().unwrap();
        assert_eq!(work.scene_draw_calls, 1);
        let counters = work.culling_stats.unwrap();
        assert_eq!(
            (
                counters.frustum_candidates,
                counters.occluders,
                counters.visible
            ),
            (0, 0, 0)
        );

        renderer.max_indirect_draws = 1;
        let fallback = draw(&mut renderer, &mut scene);
        let work = renderer.last_frame_work().unwrap();
        assert!(work.resident_scene);
        assert!(work.culling_mode.is_none());
        assert_eq!(work.scene_draw_calls, objects);
        assert_eq!(fallback.rgba, culled.rgba);
        assert_eq!(fallback.stats, culled.stats);
    }

    #[test]
    fn ordered_edits_keep_unrelated_gpu_rows_and_images() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let mut renderer = VulkanRenderer::new().unwrap();
        if !renderer.capabilities().resident_scene {
            return;
        }
        let mut original = ObjectsWorld::demo().snapshot();
        original.items.truncate(2);
        for (i, item) in original.items.iter_mut().enumerate() {
            item.id = (i + 1) as u64 * 20;
            item.material.base_color_texture = Some(Arc::new(module_engine_core::TextureImage {
                width: 1,
                height: 1,
                rgba8: vec![45 + i as u8 * 180, 120, 55, 255],
            }));
        }
        let mut source = ResidentScene::default();
        source
            .apply(SceneUpdate::Reset {
                revision: 0,
                snapshot: original.clone(),
            })
            .unwrap();
        let graph = default_graph(RenderPath::Forward);
        let sources = BTreeMap::new();
        let controls = RenderOverrides::default();
        let mut compare = |source: &mut ResidentScene| {
            let render = |renderer: &mut VulkanRenderer, input: super::super::SceneInput<'_>| {
                renderer
                    .render_scene(
                        input,
                        Camera::default(),
                        96,
                        64,
                        &graph,
                        &sources,
                        &controls,
                    )
                    .unwrap()
            };
            let actual = render(&mut renderer, super::super::SceneInput::Retained(source));
            let work = renderer.last_frame_work().unwrap();
            let expected = render(
                &mut renderer,
                super::super::SceneInput::Snapshot(source.snapshot().unwrap()),
            );
            assert!(work.resident_scene);
            assert_eq!(actual.rgba, expected.rgba);
            assert_eq!(actual.stats, expected.stats);
            work
        };
        compare(&mut source);
        let mut inserted = original.items[0].clone();
        inserted.id = 10;
        inserted.model[1][3] += 0.6;
        let mut replaced = original.items[0].clone();
        replaced.mesh.triangles.pop().unwrap();
        let mut moved = original.items[1].clone();
        moved.model[0][3] -= 0.4;
        let changes = [
            (
                10,
                SceneChangeKind::Spawned,
                std::slice::from_ref(&inserted),
                [64, 20, 16],
            ),
            (
                20,
                SceneChangeKind::Edited {
                    moved: false,
                    mesh_changed: true,
                    material_changed: false,
                },
                std::slice::from_ref(&replaced),
                [64, 0, 16],
            ),
            (
                40,
                SceneChangeKind::Edited {
                    moved: true,
                    mesh_changed: false,
                    material_changed: false,
                },
                std::slice::from_ref(&moved),
                [64, 0, 0],
            ),
            (10, SceneChangeKind::Removed, &[][..], [0, 0, 0]),
        ];
        for (base, (id, kind, items, expected)) in changes.into_iter().enumerate() {
            source
                .apply(SceneUpdate::Delta {
                    base: base as u64,
                    revision: base as u64 + 1,
                    time: 0.,
                    changes: &[SceneChange { id, kind, items }],
                })
                .unwrap();
            let work = compare(&mut source);
            assert_eq!(
                [
                    work.instance_write_bytes,
                    work.material_row_write_bytes,
                    work.draw_write_bytes
                ],
                expected
            );
            let repeated = compare(&mut source);
            assert_eq!(
                repeated.instance_write_bytes
                    + repeated.material_row_write_bytes
                    + repeated.draw_write_bytes,
                0
            );
        }
    }

    #[test]
    fn fallback_and_unsent_updates_keep_the_pending_pose_list_bounded() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let mut renderer = VulkanRenderer::new().unwrap();
        if !renderer.capabilities().resident_scene {
            eprintln!("Adapter does not support resident geometry");
            return;
        }
        let mut snapshot = ObjectsWorld::demo().snapshot();
        snapshot.items.sort_by_key(|item| item.id);
        snapshot.items.truncate(2);
        let graph = default_graph(RenderPath::Forward);
        let sources = BTreeMap::new();
        let controls = RenderOverrides::default();
        let mut source = ResidentScene::default();
        source
            .apply(SceneUpdate::Reset {
                revision: 0,
                snapshot: snapshot.clone(),
            })
            .unwrap();
        let render = |renderer: &mut VulkanRenderer, source: &mut ResidentScene| {
            renderer
                .render_scene(
                    super::super::SceneInput::Retained(source),
                    Camera::default(),
                    96,
                    64,
                    &graph,
                    &sources,
                    &controls,
                )
                .unwrap();
            renderer.last_frame_work().unwrap()
        };
        let apply = |source: &mut ResidentScene, revision, edits: &[RenderItem]| {
            let changes: Vec<_> = edits
                .iter()
                .map(|item| SceneChange {
                    id: item.id,
                    kind: SceneChangeKind::Edited {
                        moved: true,
                        material_changed: false,
                        mesh_changed: false,
                    },
                    items: std::slice::from_ref(item),
                })
                .collect();
            source
                .apply(SceneUpdate::Delta {
                    base: revision - 1,
                    revision,
                    time: 0.,
                    changes: &changes,
                })
                .unwrap();
            assert_eq!(source.sync(), SceneSync::At(revision));
        };
        assert!(render(&mut renderer, &mut source).resident_scene);
        let mut incoming = snapshot.items.clone();
        incoming[0].model[0] = [f32::MAX, f32::MAX, 0., f32::MAX];
        for revision in 1..=3 {
            incoming[1].model[0][3] += 0.25;
            apply(&mut source, revision, &incoming);
            assert!(!render(&mut renderer, &mut source).resident_scene);
            assert!(
                source.rebuild && source.moved.is_empty(),
                "Compatibility draws retained a stale incremental journal"
            );
        }
        apply(&mut source, 4, &snapshot.items[..1]);
        let work = render(&mut renderer, &mut source);
        assert!(work.resident_scene);
        assert_eq!(work.instance_write_bytes, 64);
        assert_eq!(work.vertex_write_bytes + work.mesh_index_write_bytes, 0);
        // Simulate repeated accepted Host deltas while the producer cannot submit.
        for revision in 5..=12 {
            incoming[1].model[0][3] += 0.25;
            apply(&mut source, revision, &incoming[1..]);
            assert!(
                source.moved.len() <= 2,
                "Unsent updates accumulated historical row indices"
            );
        }
        let work = render(&mut renderer, &mut source);
        assert!(work.resident_scene);
        assert_eq!(work.instance_write_bytes, 64);
        assert_eq!(work.vertex_write_bytes + work.mesh_index_write_bytes, 0);
    }
}
