//! Static scene geometry for the source-only R8.1 benchmark.

use super::{Buffer, VulkanError, VulkanRenderer};
use ash::vk;
use bytemuck::cast_slice;
use module_engine_core::SceneSnapshot;
use std::{collections::HashMap, mem::size_of_val, sync::Arc, time::Instant};

pub(super) const SOURCE: &str = include_str!("meshlet_probe.hlsl");
pub(super) const STAGES: vk::ShaderStageFlags = vk::ShaderStageFlags::from_raw(
    vk::ShaderStageFlags::TASK_EXT.as_raw() | vk::ShaderStageFlags::MESH_EXT.as_raw(),
);
const VERTICES: usize = 64;
const TRIANGLES: usize = 124;
const LANES: u32 = 32;
const MESHLETS_PER_TASK: u32 = 32;

#[derive(serde::Serialize)]
pub struct MeshletDeviceReport {
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub device_uuid: [u8; vk::UUID_SIZE],
    pub driver_version_raw: u32,
    pub api_version: u32,
    pub max_storage_buffer_range: u32,
    pub timestamp_valid_bits: u32,
    pub timestamp_period_ns: f64,
    pub extension_enabled: bool,
    pub mesh_shader: Option<MeshShaderLimits>,
}

#[derive(serde::Serialize)]
pub struct MeshShaderLimits {
    pub extension_revision: u32,
    pub task_shader: bool,
    pub mesh_shader: bool,
    pub mesh_shader_queries: bool,
    pub max_task_dispatch_x: u32,
    pub max_task_dispatch_total: u32,
    pub max_task_workgroup_size_x: u32,
    pub max_task_workgroup_invocations: u32,
    pub max_task_payload_size: u32,
    pub max_mesh_dispatch_x: u32,
    pub max_mesh_dispatch_total: u32,
    pub max_mesh_workgroup_size_x: u32,
    pub max_mesh_workgroup_invocations: u32,
    pub max_mesh_output_vertices: u32,
    pub max_mesh_output_primitives: u32,
    pub max_mesh_output_memory_size: u32,
    pub max_mesh_payload_and_output_memory_size: u32,
    pub preferred_task_workgroup_invocations: u32,
    pub preferred_mesh_workgroup_invocations: u32,
}

impl MeshletDeviceReport {
    pub(super) fn query(renderer: &VulkanRenderer) -> Result<Self, VulkanError> {
        let instance = &renderer.instance;
        let physical = renderer.physical;
        let mut identity = vk::PhysicalDeviceIDProperties::default();
        let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut identity);
        unsafe { instance.get_physical_device_properties2(physical, &mut properties) };
        let base = properties.properties;
        let extensions = unsafe { instance.enumerate_device_extension_properties(physical) }
            .map_err(VulkanError::api("read mesh shader device extensions"))?;
        let ext = extensions
            .iter()
            .find(|ext| ext.extension_name_as_c_str().ok() == Some(ash::ext::mesh_shader::NAME));
        let mesh_shader = ext.map(|ext| {
            let mut features = vk::PhysicalDeviceMeshShaderFeaturesEXT::default();
            let mut limits = vk::PhysicalDeviceMeshShaderPropertiesEXT::default();
            unsafe {
                instance.get_physical_device_features2(
                    physical,
                    &mut vk::PhysicalDeviceFeatures2::default().push_next(&mut features),
                );
                instance.get_physical_device_properties2(
                    physical,
                    &mut vk::PhysicalDeviceProperties2::default().push_next(&mut limits),
                );
            }
            MeshShaderLimits {
                extension_revision: ext.spec_version,
                task_shader: features.task_shader != 0,
                mesh_shader: features.mesh_shader != 0,
                mesh_shader_queries: features.mesh_shader_queries != 0,
                max_task_dispatch_x: limits.max_task_work_group_count[0],
                max_task_dispatch_total: limits.max_task_work_group_total_count,
                max_task_workgroup_size_x: limits.max_task_work_group_size[0],
                max_task_workgroup_invocations: limits.max_task_work_group_invocations,
                max_task_payload_size: limits.max_task_payload_size,
                max_mesh_dispatch_x: limits.max_mesh_work_group_count[0],
                max_mesh_dispatch_total: limits.max_mesh_work_group_total_count,
                max_mesh_workgroup_size_x: limits.max_mesh_work_group_size[0],
                max_mesh_workgroup_invocations: limits.max_mesh_work_group_invocations,
                max_mesh_output_vertices: limits.max_mesh_output_vertices,
                max_mesh_output_primitives: limits.max_mesh_output_primitives,
                max_mesh_output_memory_size: limits.max_mesh_output_memory_size,
                max_mesh_payload_and_output_memory_size: limits
                    .max_mesh_payload_and_output_memory_size,
                preferred_task_workgroup_invocations: limits
                    .max_preferred_task_work_group_invocations,
                preferred_mesh_workgroup_invocations: limits
                    .max_preferred_mesh_work_group_invocations,
            }
        });
        Ok(Self {
            name: renderer.name.clone(),
            vendor_id: base.vendor_id,
            device_id: base.device_id,
            device_uuid: identity.device_uuid,
            driver_version_raw: base.driver_version,
            api_version: base.api_version,
            max_storage_buffer_range: base.limits.max_storage_buffer_range,
            timestamp_valid_bits: renderer.timestamp_valid_bits,
            timestamp_period_ns: renderer.timestamp_period_ns,
            extension_enabled: renderer.meshlet_device.is_some(),
            mesh_shader,
        })
    }
}

#[derive(serde::Serialize)]
pub struct MeshletBuildReport {
    pub max_vertices_per_meshlet: usize,
    pub max_triangles_per_meshlet: usize,
    pub meshlets: u32,
    pub meshlet_vertex_references: usize,
    pub source_triangles: usize,
    pub source_vertices: usize,
    pub position_uv_bytes: usize,
    pub meshlet_header_bytes: usize,
    pub meshlet_vertex_index_bytes: usize,
    pub meshlet_primitive_bytes: usize,
    pub object_bytes: usize,
    pub build_ms: f64,
    pub allocate_and_stage_ms: f64,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct MeshletWork {
    pub task_groups: u32,
    pub meshlets_emitted: u32,
    pub meshlets_culled: u32,
    pub triangles_emitted: usize,
    pub upload_bytes: usize,
}

#[derive(Default)]
struct PackedGeometry {
    vertices: Vec<[f32; 5]>,
    headers: Vec<[u32; 5]>,
    vertex_indices: Vec<u32>,
    primitives: Vec<[u32; 2]>,
    objects: Vec<[[f32; 4]; 4]>,
}

fn resource(message: &str) -> VulkanError {
    VulkanError::Resource(format!("R8.1 meshlets: {message}"))
}

fn canonical([a, b, c]: [u32; 3]) -> [u32; 3] {
    [a, b, c].min([b, c, a]).min([c, a, b])
}

impl PackedGeometry {
    fn build(scene: &SceneSnapshot) -> Result<Self, VulkanError> {
        let mut packed = Self::default();
        // Every vertex address in the shader is a 32-bit byte offset.
        let max_vertices = (u32::MAX / 20) as usize;
        let max_triangles = (u32::MAX / 12) as usize;
        if scene.items.is_empty() || scene.items.len() > (1 << 24) {
            return Err(resource("unsupported number of drawables"));
        }
        for (ordinal, item) in scene.items.iter().enumerate() {
            let mesh = &item.mesh;
            let vertex_count = mesh.vertices.len();
            let triangle_count = mesh.triangles.len();
            if vertex_count == 0
                || triangle_count == 0
                || vertex_count > max_vertices - packed.vertices.len()
                || triangle_count > max_triangles - packed.primitives.len()
                || item.model.iter().flatten().any(|x| !x.is_finite())
                || mesh.vertices.iter().flatten().any(|x| !x.is_finite())
            {
                return Err(resource("scene geometry is empty, too large or non-finite"));
            }
            let mut faces = HashMap::with_capacity(triangle_count);
            let mut indices = Vec::with_capacity(3 * triangle_count);
            for (i, tri) in mesh.triangles.iter().enumerate() {
                if tri.iter().any(|&index| index >= vertex_count) {
                    return Err(resource("triangle references an absent vertex"));
                }
                let tri = tri.map(|index| index as u32);
                if faces.insert(canonical(tri), i).is_some() {
                    return Err(resource(
                        "the static fixture requires distinct oriented faces",
                    ));
                }
                indices.extend(tri);
            }
            let adapter = meshopt::VertexDataAdapter::new(
                cast_slice(&mesh.vertices),
                size_of::<[f32; 3]>(),
                0,
            )
            .map_err(|error| VulkanError::Resource(error.to_string()))?;
            let base_vertex = packed.vertices.len() as u32;
            let base_triangle = packed.primitives.len() as u32 + 1;
            let clusters = meshopt::build_meshlets(&indices, &adapter, VERTICES, TRIANGLES, 0.);
            for cluster in clusters.iter() {
                packed.headers.push([
                    packed.vertex_indices.len() as u32,
                    packed.primitives.len() as u32,
                    cluster.vertices.len() as u32,
                    (cluster.triangles.len() / 3) as u32,
                    ordinal as u32,
                ]);
                packed
                    .vertex_indices
                    .extend(cluster.vertices.iter().map(|&vertex| base_vertex + vertex));
                for &corners in cluster.triangles.as_chunks::<3>().0 {
                    let source = corners.map(|i| cluster.vertices[i as usize]);
                    let i = faces.remove(&canonical(source)).ok_or_else(|| {
                        resource("meshopt output changed triangle identity or winding")
                    })?;
                    // Restore the original cyclic order for the same cross product.
                    let rotation = source
                        .iter()
                        .position(|&vertex| vertex == indices[i * 3])
                        .unwrap();
                    let local = u32::from_le_bytes([
                        corners[rotation],
                        corners[(rotation + 1) % 3],
                        corners[(rotation + 2) % 3],
                        0,
                    ]);
                    packed.primitives.push([local, base_triangle + i as u32]);
                }
            }
            if !faces.is_empty() {
                return Err(resource("meshopt output omitted source triangles"));
            }
            packed
                .vertices
                .extend(mesh.vertices.iter().enumerate().map(|(i, p)| {
                    let uv = mesh.texcoords.get(i).copied().unwrap_or_default();
                    [p[0], p[1], p[2], uv[0], uv[1]]
                }));
            let [x, y, z, _] = item.model;
            let color =
                std::array::from_fn(|i| item.color[i] as f32 / 255. * item.material.base_color[i]);
            packed.objects.push([x, y, z, color]);
        }
        Ok(packed)
    }

    fn segments(&self) -> [&[u8]; 5] {
        [
            cast_slice(&self.vertices),
            cast_slice(&self.headers),
            cast_slice(&self.vertex_indices),
            cast_slice(&self.primitives),
            cast_slice(&self.objects),
        ]
    }
}

/// An immutable source and its device-bound geometry for an explicit bench draw.
pub struct MeshletScene {
    pub(super) snapshot: SceneSnapshot,
    pub(super) buffers: Arc<[Buffer; 5]>,
    pub(super) staging: Option<Arc<Buffer>>,
    pub(super) context: Arc<super::DeviceLifetime>,
    build: MeshletBuildReport,
}

impl MeshletScene {
    pub(super) fn new(
        renderer: &VulkanRenderer,
        snapshot: SceneSnapshot,
    ) -> Result<Self, VulkanError> {
        let device = MeshletDeviceReport::query(renderer)?;
        let props = device
            .mesh_shader
            .filter(|_| device.extension_enabled)
            .ok_or_else(|| resource("bench device did not enable VK_EXT_mesh_shader"))?;
        let started = Instant::now();
        let geometry = PackedGeometry::build(&snapshot)?;
        let build_ms = started.elapsed().as_secs_f64() * 1000.;
        let meshlets = geometry.headers.len() as u32;
        let task_groups = meshlets.div_ceil(MESHLETS_PER_TASK);
        if task_groups == 0
            || task_groups > props.max_task_dispatch_x.min(props.max_task_dispatch_total)
            || MESHLETS_PER_TASK > props.max_mesh_dispatch_x.min(props.max_mesh_dispatch_total)
            || LANES
                > props
                    .max_task_workgroup_invocations
                    .min(props.max_task_workgroup_size_x)
            || LANES
                > props
                    .max_mesh_workgroup_invocations
                    .min(props.max_mesh_workgroup_size_x)
            || props.max_task_payload_size < 4
            || VERTICES as u32 > props.max_mesh_output_vertices
            || TRIANGLES as u32 > props.max_mesh_output_primitives
        {
            return Err(resource(
                "workgroups or shader outputs exceed device limits",
            ));
        }
        let segments = geometry.segments();
        if segments
            .iter()
            .any(|bytes| bytes.len() as u64 > renderer.storage_buffer_limit)
        {
            return Err(resource("a geometry buffer exceeds maxStorageBufferRange"));
        }
        let started = Instant::now();
        let buffers = segments
            .iter()
            .map(|bytes| {
                renderer.device_buffer(bytes.len() as u64, vk::BufferUsageFlags::STORAGE_BUFFER)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let buffers = Arc::new(buffers.try_into().ok().unwrap());
        let staging = renderer.buffer(
            segments.iter().map(|bytes| bytes.len() as u64).sum(),
            vk::BufferUsageFlags::TRANSFER_SRC,
        )?;
        // This mapping is exclusively owned and not yet referenced by commands.
        unsafe {
            staging
                .memory
                .as_ref()
                .unwrap()
                .access(staging.size, |mapped| {
                    let mut offset = 0;
                    for bytes in segments {
                        std::ptr::copy_nonoverlapping(
                            bytes.as_ptr(),
                            mapped.add(offset),
                            bytes.len(),
                        );
                        offset += bytes.len();
                    }
                })?;
        }
        let build = MeshletBuildReport {
            max_vertices_per_meshlet: VERTICES,
            max_triangles_per_meshlet: TRIANGLES,
            meshlets,
            meshlet_vertex_references: geometry.vertex_indices.len(),
            source_triangles: geometry.primitives.len(),
            source_vertices: geometry.vertices.len(),
            position_uv_bytes: size_of_val(geometry.vertices.as_slice()),
            meshlet_header_bytes: size_of_val(geometry.headers.as_slice()),
            meshlet_vertex_index_bytes: size_of_val(geometry.vertex_indices.as_slice()),
            meshlet_primitive_bytes: size_of_val(geometry.primitives.as_slice()),
            object_bytes: size_of_val(geometry.objects.as_slice()),
            build_ms,
            allocate_and_stage_ms: started.elapsed().as_secs_f64() * 1000.,
        };
        Ok(Self {
            snapshot,
            buffers,
            staging: Some(Arc::new(staging)),
            context: renderer.context.clone(),
            build,
        })
    }

    pub fn build_report(&self) -> &MeshletBuildReport {
        &self.build
    }

    pub(super) fn work(&self) -> MeshletWork {
        MeshletWork {
            task_groups: self.build.meshlets.div_ceil(MESHLETS_PER_TASK),
            meshlets_emitted: self.build.meshlets,
            meshlets_culled: 0,
            triangles_emitted: self.build.source_triangles,
            upload_bytes: self
                .staging
                .as_ref()
                .map_or(0, |buffer| buffer.size as usize),
        }
    }

    pub(super) fn record_upload(&self, device: &ash::Device, command: vk::CommandBuffer) {
        let Some(staging) = &self.staging else { return };
        let mut offset = 0;
        unsafe {
            for buffer in self.buffers.iter() {
                device.cmd_copy_buffer(
                    command,
                    staging.buffer,
                    buffer.buffer,
                    &[vk::BufferCopy::default()
                        .src_offset(offset)
                        .size(buffer.size)],
                );
                offset += buffer.size;
            }
            device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::MESH_SHADER_EXT,
                vk::DependencyFlags::empty(),
                &[vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)],
                &[],
                &[],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use module_engine_core::{Mesh, RenderItem};

    #[test]
    fn meshlet_indices_preserve_source_ids_and_oriented_corners() {
        let mesh = Mesh::cube();
        let scene = SceneSnapshot {
            items: (1..=2)
                .map(|i| RenderItem {
                    id: i,
                    mesh: mesh.clone(),
                    model: module_engine_core::Transform::default().matrix(),
                    color: [255; 4],
                    material: Default::default(),
                })
                .collect(),
            time: 0.,
        };
        let packed = PackedGeometry::build(&scene).unwrap();
        let mut generated = Vec::new();
        for [
            first_vertex,
            first_primitive,
            vertex_count,
            primitive_count,
            object,
        ] in packed.headers
        {
            assert!((vertex_count as usize) <= VERTICES);
            assert!((primitive_count as usize) <= TRIANGLES);
            for i in first_primitive..first_primitive + primitive_count {
                let [local, id] = packed.primitives[i as usize];
                let tri = local.to_le_bytes()[..3]
                    .iter()
                    .map(|&i| packed.vertex_indices[first_vertex as usize + i as usize] as usize)
                    .collect::<Vec<_>>();
                let source = mesh.triangles[(id as usize - 1) % mesh.triangles.len()];
                assert_eq!(
                    tri,
                    source.map(|i| i + object as usize * mesh.vertices.len())
                );
                assert_eq!((id as usize - 1) / mesh.triangles.len(), object as usize);
                generated.push(id);
            }
        }
        generated.sort_unstable();
        assert_eq!(generated, (1..=24).collect::<Vec<_>>());
    }
}
