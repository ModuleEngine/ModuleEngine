use super::{
    Buffer, FrameWork, MaterialSet, VulkanError, VulkanRenderer, materials::MaterialUploads,
};
use ash::vk;
use module_engine_core::{Mesh, RenderItem, RenderScene, SceneSnapshot, SceneSync, SceneUpdate};
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    mem::size_of,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

pub(crate) const VERTEX_SHADER: &str = include_str!("../../../assets/gpu/resident_scene.hlsl");

/// The R1.6 receiver remains authoritative for order, contents and revisions.
/// A busy or failed submission leaves its pending edits available for retry.
pub struct ResidentScene {
    id: u64,
    scene: RenderScene,
    rebuild: bool,
    moved: Vec<usize>,
}

impl Default for ResidentScene {
    fn default() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            scene: RenderScene::default(),
            rebuild: true,
            moved: Vec::new(),
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
        let rebuild = self.rebuild
            || match &update {
                SceneUpdate::Reset { .. } => true,
                SceneUpdate::Delta { changes, .. } => changes.iter().any(|change| {
                    !matches!(
                        change.kind,
                        Edited {
                            mesh_changed: false,
                            material_changed: false,
                            ..
                        }
                    )
                }),
            };
        self.scene.apply_with_edits(update, |range| {
            if !rebuild {
                self.moved.extend(range);
            }
        })?;
        // Busy or failed submissions can accumulate deltas. The current list
        // is sufficient for reconciliation once the journal would exceed it.
        if rebuild || self.moved.len() > self.scene.snapshot()?.items.len() {
            self.require_reconciliation();
        }
        Ok(())
    }

    pub(super) fn require_reconciliation(&mut self) {
        self.rebuild = true;
        self.moved.clear();
    }

    fn accepted(&mut self) {
        self.rebuild = false;
        self.moved.clear();
    }
}

#[derive(Clone, Copy)]
struct MeshRange {
    first: u32,
    count: u32,
    extent: f32,
}

type MeshEntries = HashMap<u64, Vec<(Mesh, MeshRange)>>;

#[derive(Default)]
struct MeshPool {
    meshes: MeshEntries,
    vertex_count: usize,
    index_count: usize,
}

impl MeshPool {
    fn commit(&mut self, upload: MeshUpload) {
        self.vertex_count += upload.vertices.len();
        self.index_count += upload.indices.len();
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
}

fn positions(mesh: &Mesh) -> impl Iterator<Item = u32> + '_ {
    mesh.vertices
        .iter()
        .flatten()
        .chain(mesh.texcoords.iter().flatten())
        .map(|v| v.to_bits())
}

impl MeshUpload {
    fn get(&mut self, pool: &MeshPool, mesh: &Mesh, limit: u64) -> Result<MeshRange, VulkanError> {
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
        let range = MeshRange {
            first,
            count: (pool.index_count + self.indices.len()) as u32 - first,
            extent: mesh
                .vertices
                .iter()
                .flatten()
                .copied()
                .filter(|v| v.is_finite())
                .map(f32::abs)
                .fold(0., f32::max),
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

#[derive(Default)]
struct Rows {
    instances: Vec<Instance>,
    colors: Vec<Color>,
    draws: Vec<Draw>,
    images: Vec<[u32; 1]>,
    triangles: usize,
    extents: Vec<f32>,
}

#[derive(Clone)]
pub(super) struct Buffers {
    pub vertices: Arc<Buffer>,
    pub indices: Arc<Buffer>,
    pub instances: Arc<Buffer>,
    pub colors: Arc<Buffer>,
    images: Arc<Buffer>,
    draws: Arc<Buffer>,
}

impl Buffers {
    pub(super) fn bind(&self) -> [&Buffer; 4] {
        [&self.vertices, &self.indices, &self.instances, &self.colors]
    }

    pub(super) fn draw(
        &self,
        device: &ash::Device,
        command: vk::CommandBuffer,
        mut count: usize,
        max_draws: u32,
    ) -> usize {
        let stride = size_of::<vk::DrawIndirectCommand>() as u32;
        let mut offset = 0;
        let mut calls = 0;
        while count != 0 {
            let batch = count.min(max_draws as usize);
            unsafe {
                device.cmd_draw_indirect(command, self.draws.buffer, offset, batch as u32, stride)
            };
            offset += batch as u64 * u64::from(stride);
            count -= batch;
            calls += 1;
        }
        calls
    }
}

enum RowsUpdate {
    Full(Rows),
    Moved(Vec<(usize, Instance)>),
    None,
}

pub(super) struct SceneUpload {
    pub buffers: Buffers,
    pub materials: Arc<MaterialSet>,
    pub uploads: MaterialUploads,
    pub triangles: usize,
    pub draws: usize,
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
        if self.scene == Some(source.id) && !source.rebuild {
            let mut rows = Vec::with_capacity(source.moved.len());
            for &i in &source.moved {
                let item = &scene.items[i];
                if !safe_model(item, self.rows.extents[i]) {
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
                draws: self.rows.draws.len(),
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
        let items = scene.items.len();
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
        let image_rows = buffer(prior.map(|b| &b.images), (items + 1) * 4, storage)?;
        let Some((materials, uploads)) = renderer.prepare_resident_materials(
            scene,
            self.materials.as_ref(),
            image_rows.clone(),
        )?
        else {
            return Ok(None);
        };
        let mut rows = Rows {
            instances: Vec::with_capacity(items),
            colors: Vec::with_capacity(items),
            draws: Vec::with_capacity(items),
            images: Vec::with_capacity(items + 1),
            triangles: 0,
            extents: Vec::with_capacity(items),
        };
        let mut meshes = MeshUpload::default();
        rows.images.push([0]);
        for (i, item) in scene.items.iter().enumerate() {
            let mesh = meshes.get(&self.pool, &item.mesh, limit)?;
            if !safe_model(item, mesh.extent) {
                return Ok(None);
            }
            rows.extents.push(mesh.extent);
            let mut row = [0; 16];
            model(&mut row, item);
            row[12] = (rows.triangles as u32 + 1).wrapping_sub(mesh.first / 3);
            row[13] = i as u32;
            rows.triangles += mesh.count as usize / 3;
            rows.instances.push(row);
            rows.colors.push(std::array::from_fn(|c| {
                (item.color[c] as f32 / 255.0 * item.material.base_color[c]).to_bits()
            }));
            rows.draws.push([mesh.count, 1, mesh.first, i as u32]);
            rows.images
                .push([materials.index(item.material.base_color_texture.as_ref())]);
        }
        let buffers = Buffers {
            vertices: buffer(
                prior.map(|b| &b.vertices),
                (self.pool.vertex_count + meshes.vertices.len()) * 20,
                storage,
            )?,
            indices: buffer(
                prior.map(|b| &b.indices),
                (self.pool.index_count + meshes.indices.len()) * 4,
                storage,
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
                vk::BufferUsageFlags::INDIRECT_BUFFER,
            )?,
            images: image_rows,
        };
        Ok(Some(SceneUpload {
            buffers,
            materials,
            uploads,
            triangles: rows.triangles,
            draws: rows.draws.len(),
            rows: RowsUpdate::Full(rows),
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
        // Submissions use one graphics queue. A prior vertex/fragment/indirect
        // read must finish before an in-place write, and the new rows must be
        // available before ANY following draw can read the shared buffers.
        let consumers = vk::PipelineStageFlags::VERTEX_SHADER
            | vk::PipelineStageFlags::FRAGMENT_SHADER
            | vk::PipelineStageFlags::DRAW_INDIRECT;
        let reads = vk::AccessFlags::SHADER_READ | vk::AccessFlags::INDIRECT_COMMAND_READ;
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
                    before
                        .filter(|b| Arc::ptr_eq(&b.instances, &after.instances))
                        .map(|_| &self.rows.instances),
                    &rows.instances,
                );
                work.material_row_write_bytes = update.changed(
                    &after.colors,
                    before
                        .filter(|b| Arc::ptr_eq(&b.colors, &after.colors))
                        .map(|_| &self.rows.colors),
                    &rows.colors,
                ) + update.changed(
                    &after.images,
                    before
                        .filter(|b| Arc::ptr_eq(&b.images, &after.images))
                        .map(|_| &self.rows.images),
                    &rows.images,
                );
                work.draw_write_bytes = update.changed(
                    &after.draws,
                    before
                        .filter(|b| Arc::ptr_eq(&b.draws, &after.draws))
                        .map(|_| &self.rows.draws),
                    &rows.draws,
                );
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
                self.rows = rows;
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
        // Arrays of initialized u32 words contain no padding.
        let data = unsafe { std::slice::from_raw_parts(words.as_ptr().cast::<u8>(), bytes) };
        let mut offset = (start * N * size_of::<u32>()) as u64;
        for chunk in data.chunks(65_536) {
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
        before: Option<&Vec<[u32; N]>>,
        after: &[[u32; N]],
    ) -> usize {
        let Some(before) = before else {
            return self.write(buffer, 0, after);
        };
        let mut bytes = 0;
        let mut cursor = 0;
        while let Some(start) = (cursor..after.len()).find(|&i| before.get(i) != after.get(i)) {
            cursor = (start + 1..after.len())
                .find(|&i| before.get(i) == after.get(i))
                .unwrap_or(after.len());
            bytes += self.write(buffer, start, &after[start..cursor]);
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
