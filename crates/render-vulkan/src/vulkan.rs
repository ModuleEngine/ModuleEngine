use crate::presentation::{
    DeviceLifetime, PresentationPool, SharedImage, VulkanPresentationTarget,
};
use crate::{
    DeviceCapabilities,
    frames::{Completion, FramePools, Timeline},
    memory::{DeviceAllocator, ResourceMemory},
    staging::StagingCursor,
    transfer::TransferQueue,
};
use crate::{VulkanError, instance::InstanceLifetime};
#[cfg(test)]
use crate::{compiler::compile_hlsl, graph::build_plan};
use crate::{
    compiler::{
        bindless_source, compile_hlsl_at, culled_fragment_source, verify_bindless_shader,
        verify_resident_vertex, verify_shader,
    },
    geometry,
    graph::{Pass, Plan, build_metadata_plan, build_plan_at},
    profiling::{PendingTimings, TimestampCapture, TimestampPool},
};
use ash::vk;
use gpu_allocator::vulkan::AllocationScheme;
use module_engine_core::{
    Camera, Frame, Graph, RenderOverrides, RenderResourceCommand, RenderStats, RenderSubmission,
    RendererPerformance, SceneSnapshot,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{CStr, CString},
    hash::{Hash, Hasher},
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    task::Poll,
    time::{Duration, Instant},
};

#[path = "materials.rs"]
mod materials;
use materials::{MaterialLayout, MaterialSet};

#[path = "resident.rs"]
pub(crate) mod resident;
pub use resident::ResidentScene;

#[path = "culling.rs"]
pub(crate) mod culling;
pub use culling::{CullingStats, SceneCulling};

#[cfg(feature = "bench")]
#[path = "meshlet_probe.rs"]
mod meshlet_probe;
#[cfg(feature = "bench")]
pub use meshlet_probe::{MeshletBuildReport, MeshletDeviceReport, MeshletScene};

pub enum SceneInput<'a> {
    Snapshot(&'a SceneSnapshot),
    Retained(&'a mut ResidentScene),
    #[cfg(feature = "bench")]
    Meshlets(&'a mut MeshletScene),
}

impl SceneInput<'_> {
    fn snapshot(&self) -> Result<&SceneSnapshot, VulkanError> {
        match self {
            Self::Snapshot(scene) => Ok(scene),
            Self::Retained(scene) => scene.snapshot().map_err(VulkanError::Resource),
            #[cfg(feature = "bench")]
            Self::Meshlets(scene) => Ok(&scene.snapshot),
        }
    }
}

static NEXT_TRACE_ID: AtomicU64 = AtomicU64::new(1);

const INCLUDE_REFRESH_INTERVAL: Duration = Duration::from_millis(250);
#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Debug)]
enum AllocationFault {
    BufferHandle,
    BufferMemory,
    ImageHandle,
    ImageMemory,
    ImageView,
}
#[cfg(test)]
#[derive(Default)]
struct AllocationCounts {
    buffers: std::sync::atomic::AtomicUsize,
    images: std::sync::atomic::AtomicUsize,
    views: std::sync::atomic::AtomicUsize,
    memory: std::sync::atomic::AtomicUsize,
}
#[cfg(test)]
impl AllocationCounts {
    fn snapshot(&self) -> [usize; 4] {
        use std::sync::atomic::Ordering::Relaxed;
        [
            self.buffers.load(Relaxed),
            self.images.load(Relaxed),
            self.views.load(Relaxed),
            self.memory.load(Relaxed),
        ]
    }
}
struct RejectedPreparation {
    graph: Graph,
    sources: BTreeMap<String, String>,
    signature: Option<Vec<String>>,
    checked: Instant,
    error: VulkanError,
}
impl RejectedPreparation {
    fn matches(&self, graph: &Graph, sources: &BTreeMap<String, String>) -> bool {
        self.graph.execution_eq(graph) && &self.sources == sources
    }
}

struct Image {
    device: ash::Device,
    image: vk::Image,
    view: vk::ImageView,
    memory: Option<ResourceMemory>,
    format: vk::Format,
    aspect: vk::ImageAspectFlags,
    layout: vk::ImageLayout,
    stage: vk::PipelineStageFlags,
    access: vk::AccessFlags,
    #[cfg(test)]
    allocations: std::sync::Arc<AllocationCounts>,
}
impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            if self.view != vk::ImageView::null() {
                self.device.destroy_image_view(self.view, None);
                #[cfg(test)]
                self.allocations
                    .views
                    .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }
            if self.image != vk::Image::null() {
                self.device.destroy_image(self.image, None);
                #[cfg(test)]
                self.allocations
                    .images
                    .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }
            #[cfg(test)]
            if self.memory.is_some() {
                self.allocations
                    .memory
                    .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}
struct Buffer {
    device: ash::Device,
    buffer: vk::Buffer,
    memory: Option<ResourceMemory>,
    size: u64,
    #[cfg(test)]
    allocations: std::sync::Arc<AllocationCounts>,
}
impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            if self.buffer != vk::Buffer::null() {
                self.device.destroy_buffer(self.buffer, None);
                #[cfg(test)]
                self.allocations
                    .buffers
                    .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }
            #[cfg(test)]
            if self.memory.is_some() {
                self.allocations
                    .memory
                    .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}
struct Pipeline {
    device: ash::Device,
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
    set_layout: vk::DescriptorSetLayout,
    compute: bool,
    bindless_materials: bool,
    geometry: Geometry,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Geometry {
    Expanded,
    Resident,
    Culled,
    #[cfg(feature = "bench")]
    Meshlets,
}

impl Geometry {
    fn meshlets(self) -> bool {
        #[cfg(feature = "bench")]
        {
            self == Self::Meshlets
        }
        #[cfg(not(feature = "bench"))]
        {
            false
        }
    }

    fn push_stages(self) -> vk::ShaderStageFlags {
        #[cfg(feature = "bench")]
        if self.meshlets() {
            return meshlet_probe::STAGES | vk::ShaderStageFlags::FRAGMENT;
        }
        vk::ShaderStageFlags::VERTEX
            | vk::ShaderStageFlags::FRAGMENT
            | vk::ShaderStageFlags::COMPUTE
    }
}

struct FrameResources {
    key: FrameResourceKey,
    images: BTreeMap<String, Image>,
    vertex: Option<Buffer>,
    staging: Option<Buffer>,
    culling: Option<culling::Frame>,
}

/// A graph-owned image's bound Vulkan range, for the synchronous benchmark.
#[cfg(feature = "bench")]
#[derive(serde::Serialize)]
pub struct GraphImageAllocation {
    pub resource: String,
    pub format: String,
    pub memory_binding: usize,
    pub offset_bytes: u64,
    pub required_bytes: u64,
}

/// Allocation identity is independent from shader/pipeline identity. Reusing a
/// pipeline under a new pass ID must still allocate its newly named resources.
#[derive(Debug, PartialEq, Eq)]
struct FrameResourceKey {
    size: [usize; 2],
    images: Vec<(String, String, bool)>,
    readback: Vec<ReadbackImage>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReadbackImage {
    resource: String,
    kind: usize,
    offset: usize,
    size: usize,
}

impl ReadbackImage {
    fn end(&self) -> usize {
        self.offset + self.size
    }
}

fn frame_readback(plan: &Plan, width: usize, height: usize, detailed: bool) -> Vec<ReadbackImage> {
    let mut readback: Vec<ReadbackImage> = Vec::new();
    let mut push = |resource, kind, texel_bytes| {
        let end = readback.last().map_or(0, ReadbackImage::end);
        readback.push(ReadbackImage {
            resource,
            kind,
            // vkCmdCopyImageToBuffer requires the texel block alignment
            // of the next image, even when prior planes have odd dimensions.
            offset: end.next_multiple_of(texel_bytes),
            size: width * height * texel_bytes,
        });
    };
    push(plan.output.clone(), 0, 4);
    if !detailed {
        return readback;
    }
    if let Some(pass) = plan
        .passes
        .iter()
        .find(|pass| pass.geometry() && pass.module != "gpu.custom")
    {
        for (name, kind, bytes) in [("depth", 1, 4), ("normals", 2, 16)] {
            if let Some(spec) = pass.manifest.outputs.iter().find(|spec| spec.name == name) {
                push(pass.resource(&spec.name), kind, bytes);
            }
        }
        if let Some(spec) = pass
            .manifest
            .outputs
            .iter()
            .find(|spec| spec.name == "color" || spec.name == "albedo")
        {
            push(pass.resource(&spec.name), 3, 16);
        }
    } else if let Some(pass) = plan.passes.iter().find(|pass| pass.geometry()) {
        push(format!("{}.__hardware_depth", pass.id), 4, 4);
    }
    readback
}
fn frame_resource_key(
    plan: &Plan,
    width: usize,
    height: usize,
    readback: &[ReadbackImage],
) -> FrameResourceKey {
    let mut images = Vec::new();
    for pass in &plan.passes {
        images.extend(
            pass.manifest
                .outputs
                .iter()
                .map(|spec| (pass.resource(&spec.name), spec.format.clone(), false)),
        );
        if pass.geometry() {
            images.push((
                format!("{}.__hardware_depth", pass.id),
                "d32float".into(),
                true,
            ));
        }
    }
    FrameResourceKey {
        size: [width, height],
        images,
        readback: readback.to_vec(),
    }
}
fn validate_frame_images(
    plan: &Plan,
    available: &BTreeSet<&str>,
    readback: &[ReadbackImage],
) -> Result<(), VulkanError> {
    let mut declared = BTreeSet::new();
    for pass in &plan.passes {
        for (_, resource) in pass
            .inputs
            .iter()
            .filter(|(spec, _)| spec.format != "scene")
        {
            if !available.contains(resource.as_str()) {
                return Err(VulkanError::Resource(format!(
                    "GPU pass {} input resource {resource} has no allocated image",
                    pass.id
                )));
            }
        }
        for spec in &pass.manifest.outputs {
            let resource = pass.resource(&spec.name);
            if !declared.insert(resource.clone()) {
                return Err(VulkanError::Resource(format!(
                    "GPU output resource identity collision: {resource}"
                )));
            }
            if !available.contains(resource.as_str()) {
                return Err(VulkanError::Resource(format!(
                    "GPU pass {} output resource {resource} has no allocated image",
                    pass.id
                )));
            }
        }
        if pass.geometry() {
            let resource = format!("{}.__hardware_depth", pass.id);
            if !declared.insert(resource.clone()) {
                return Err(VulkanError::Resource(format!(
                    "GPU depth resource identity collision: {resource}"
                )));
            }
            if !available.contains(resource.as_str()) {
                return Err(VulkanError::Resource(format!(
                    "GPU pass {} hardware depth has no allocated image",
                    pass.id
                )));
            }
        }
    }
    for ReadbackImage { resource, .. } in readback {
        if !available.contains(resource.as_str()) {
            return Err(VulkanError::Resource(format!(
                "GPU readback resource {resource} has no allocated image"
            )));
        }
    }
    Ok(())
}
struct AtlasGpu {
    keys: Vec<usize>,
    atlas: geometry::TextureAtlas,
    image: Image,
}
/// Everything referenced by the recorded command buffer remains untouched until
/// completion is confirmed. In particular uploads and evicted pipelines cannot drop.
struct ProducerFlight {
    ticket: u64,
    completion: Completion,
    serial: u64,
    work: FrameWork,
    shared: Option<Arc<SharedImage>>,
    resources: FrameResources,
    atlas: Option<AtlasGpu>,
    _upload: Option<StagingUpload>,
    materials: Option<Arc<MaterialSet>>,
    _material_upload: Option<Buffer>,
    _scene_buffers: Option<(resident::Buffers, Option<resident::Buffers>)>,
    #[cfg(feature = "bench")]
    _meshlet_buffers: Option<(Arc<[Buffer; 5]>, Option<Arc<Buffer>>)>,
    _transfer_timeline: Option<Arc<Timeline>>,
    _pipelines: Vec<Arc<Pipeline>>,
    _culling_pipelines: Option<Arc<culling::Pipelines>>,
    _context: Arc<DeviceLifetime>,
    size: [usize; 2],
    stats: RenderStats,
    timings: Option<PendingTimings>,
    cancelled: Arc<AtomicBool>,
    failure: Option<VulkanError>,
}
struct FrameSlot {
    pools: FramePools,
    pending: Option<ProducerFlight>,
    resources: Option<FrameResources>,
    atlas: Option<AtlasGpu>,
    last_serial: u64,
    timestamp_pool: Option<Arc<TimestampPool>>,
}

struct StagingUpload {
    buffer: Arc<Buffer>,
    offset: u64,
    ring_range: Option<Range<u64>>,
}

#[derive(Default)]
struct StagingRing {
    buffer: Option<Arc<Buffer>>,
    cursor: StagingCursor,
}

struct ProducerRetirement {
    slots: Vec<FrameSlot>,
    _pipelines: BTreeMap<String, Arc<Pipeline>>,
    _context: Arc<DeviceLifetime>,
}

struct OwnedShaderModule<'a>(&'a ash::Device, vk::ShaderModule);

impl Drop for OwnedShaderModule<'_> {
    fn drop(&mut self) {
        unsafe { self.0.destroy_shader_module(self.1, None) };
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_pipeline(self.pipeline, None);
            self.device.destroy_pipeline_layout(self.layout, None);
            self.device
                .destroy_descriptor_set_layout(self.set_layout, None);
        }
    }
}
/// Work recorded by the most recent successful Vulkan queue submission.
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct FrameWork {
    pub draw_calls: usize,
    /// Recorded color/G-buffer/normal-depth scene draw commands, excluding the Hi-Z seed.
    pub scene_draw_calls: usize,
    pub occluder_draw_calls: usize,
    pub culling_mode: Option<SceneCulling>,
    /// A small diagnostic readback, populated only after submission completion.
    pub culling_stats: Option<CullingStats>,
    #[cfg(feature = "bench")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meshlets: Option<meshlet_probe::MeshletWork>,
    pub vertex_write_bytes: usize,
    /// CPU-to-GPU writes to persistent mesh indices, excluding GPU-local growth copies.
    pub mesh_index_write_bytes: usize,
    pub instance_write_bytes: usize,
    pub material_row_write_bytes: usize,
    pub draw_write_bytes: usize,
    pub culling_input_write_bytes: usize,
    pub resident_scene: bool,
    pub cpu_prepare_ms: f64,
    pub cpu_record_ms: f64,
    pub cpu_queue_submit_ms: f64,
    /// Synchronous diagnostic GPU wait. Native submissions do not block here.
    pub cpu_wait_ms: f64,
    pub atlas_upload_bytes: usize,
    pub material_images: usize,
    pub material_uploaded_images: usize,
    pub material_upload_bytes: usize,
}

/// Adapter and legacy vertex-buffer limits for one known-size scene.
#[derive(Debug, serde::Serialize)]
pub struct VertexAllocation {
    pub adapter: String,
    pub driver_version_raw: u32,
    pub requested_bytes: u64,
    pub max_buffer_bytes: u64,
    pub max_single_allocation_bytes: u64,
    pub allocation_bytes: Option<u64>,
    pub heap_bytes: Option<u64>,
    pub heap_budget_bytes: Option<u64>,
    pub heap_usage_bytes: Option<u64>,
    pub blocked: Option<String>,
}

const VERTEX_BUFFER_MULTIPLIER: u64 = 2;

pub struct VulkanRenderer {
    resource_root: std::path::PathBuf,
    context: Arc<DeviceLifetime>,
    instance: ash::Instance,
    device: ash::Device,
    physical: vk::PhysicalDevice,
    memory: vk::PhysicalDeviceMemoryProperties,
    queue: vk::Queue,
    family: u32,
    presentation_target: Option<Arc<VulkanPresentationTarget>>,
    presentation_slots: PresentationPool<SharedImage>,
    slots: Vec<FrameSlot>,
    serial: u64,
    timeline: Option<Arc<Timeline>>,
    allocator: Option<Arc<DeviceAllocator>>,
    transfer: Option<TransferQueue>,
    staging: StagingRing,
    staging_alignment: u64,
    capabilities: DeviceCapabilities,
    material_layout: Option<Arc<MaterialLayout>>,
    materials: Option<Arc<MaterialSet>>,
    gpu_scene: resident::GpuScene,
    culling_pipelines: Option<Arc<culling::Pipelines>>,
    culling_mode: SceneCulling,
    culling_stats: bool,
    #[cfg(feature = "bench")]
    meshlet_device: Option<ash::ext::mesh_shader::Device>,
    storage_buffer_limit: u64,
    max_indirect_draws: u32,
    trace: Option<u64>,
    name: String,
    pipelines: BTreeMap<String, Arc<Pipeline>>,
    next_ticket: u64,
    producer_error: Option<VulkanError>,
    prepared: Option<Plan>,
    diagnostics: Vec<String>,
    detailed_stats: bool,
    frame_profiling: bool,
    timestamp_valid_bits: u32,
    timestamp_period_ns: f64,
    prepared_inputs: Option<(Graph, BTreeMap<String, String>, Instant)>,
    rejected_inputs: Option<RejectedPreparation>,
    performance: RendererPerformance,
    last_frame_work: Option<FrameWork>,
    #[cfg(test)]
    allocation_fault: Option<AllocationFault>,
    #[cfg(test)]
    allocations: std::sync::Arc<AllocationCounts>,
}
impl VulkanRenderer {
    pub fn new() -> Result<Self, VulkanError> {
        Self::new_at(crate::compiler::resource_root())
    }
    /// Bind this backend's shader compilation to one explicit package root.
    pub fn new_at(resource_root: std::path::PathBuf) -> Result<Self, VulkanError> {
        Self::new_at_target(resource_root, None, false)
    }

    /// R8.1's explicit device creation path. Normal Editor and Player
    /// constructors leave the optional mesh stages disabled.
    #[cfg(feature = "bench")]
    pub fn new_meshlet_benchmark(
        resource_root: std::path::PathBuf,
        target: Option<Arc<VulkanPresentationTarget>>,
        enable_meshlets: bool,
    ) -> Result<Self, VulkanError> {
        Self::new_at_target(resource_root, target, enable_meshlets)
    }

    #[cfg(feature = "bench")]
    pub fn build_meshlet_probe(&self, scene: SceneSnapshot) -> Result<MeshletScene, VulkanError> {
        MeshletScene::new(self, scene)
    }

    #[cfg(feature = "bench")]
    pub fn meshlet_device_report(&self) -> Result<MeshletDeviceReport, VulkanError> {
        MeshletDeviceReport::query(self)
    }

    fn new_at_target(
        resource_root: std::path::PathBuf,
        presentation_target: Option<Arc<VulkanPresentationTarget>>,
        enable_meshlets: bool,
    ) -> Result<Self, VulkanError> {
        unsafe {
            let trace = std::env::var_os("MODULE_ENGINE_VULKAN_TRACE")
                .map(|_| NEXT_TRACE_ID.fetch_add(1, Ordering::Relaxed));
            let owner = InstanceLifetime::new()?;
            let instance = owner.raw.clone();
            let candidates = instance
                .enumerate_physical_devices()
                .map_err(VulkanError::api("enumerate adapters"))?;
            let mut extension_error = None;
            let selected = candidates
                .into_iter()
                .filter_map(|physical| {
                    if let Some(target) = &presentation_target
                        && !crate::presentation::matches_adapter(
                            &instance,
                            physical,
                            &target.identity,
                        )
                    {
                        return None;
                    }
                    let properties = instance.get_physical_device_properties(physical);
                    if properties.api_version < vk::API_VERSION_1_3 {
                        return None;
                    }
                    let families = instance.get_physical_device_queue_family_properties(physical);
                    let family = families.iter().position(|p| {
                        p.queue_count != 0
                            && p.queue_flags
                                .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
                    })?;
                    let extensions = match instance.enumerate_device_extension_properties(physical)
                    {
                        Ok(extensions) => extensions,
                        Err(error) => {
                            extension_error.get_or_insert(error);
                            return None;
                        }
                    };
                    if presentation_target.is_some()
                        && !extensions.iter().any(|extension| {
                            CStr::from_ptr(extension.extension_name.as_ptr())
                                == crate::presentation::extension_name()
                        })
                    {
                        return None;
                    }
                    let capabilities = DeviceCapabilities::query(
                        &instance,
                        physical,
                        &extensions,
                        family as u32,
                        &families,
                    )?;
                    Some((
                        physical,
                        capabilities,
                        properties,
                        families[family].timestamp_valid_bits,
                    ))
                })
                .max_by_key(|(_, _, p, _)| {
                    if p.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
                        2
                    } else {
                        1
                    }
                });
            let Some((physical, capabilities, properties, timestamp_valid_bits)) = selected else {
                if let Some(source) = extension_error {
                    return Err(VulkanError::api("enumerate device extensions")(source));
                }
                return Err(if presentation_target.is_some() {
                    VulkanError::NoPresentationAdapter
                } else {
                    VulkanError::NoAdapter
                });
            };
            let name = CStr::from_ptr(properties.device_name.as_ptr())
                .to_string_lossy()
                .into_owned();
            let family = capabilities.graphics_family;
            let modern = capabilities.use_modern_device();
            let transfer_family = capabilities.dedicated_transfer_family.filter(|_| modern);
            let priorities = [1.];
            let queue_info = |family| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(family)
                    .queue_priorities(&priorities)
            };
            let mut queues = vec![queue_info(family)];
            queues.extend(transfer_family.map(queue_info));
            let mut vulkan13 = vk::PhysicalDeviceVulkan13Features::default()
                .dynamic_rendering(true)
                .synchronization2(modern);
            let bindless = capabilities.bindless_materials;
            let mut vulkan12 = vk::PhysicalDeviceVulkan12Features::default()
                .timeline_semaphore(modern)
                .draw_indirect_count(capabilities.gpu_frustum_culling)
                .descriptor_indexing(bindless)
                .shader_sampled_image_array_non_uniform_indexing(bindless)
                .runtime_descriptor_array(bindless);
            let features = vk::PhysicalDeviceFeatures::default()
                .sampler_anisotropy(bindless)
                .multi_draw_indirect(capabilities.resident_scene)
                .draw_indirect_first_instance(capabilities.resident_scene);
            let mut extensions = if presentation_target.is_some() {
                vec![crate::presentation::extension_name().as_ptr()]
            } else {
                Vec::new()
            };
            if enable_meshlets {
                if !capabilities.tier_b || !bindless {
                    return Err(VulkanError::Resource(format!(
                        "R8.1 meshlets unavailable on {name}: requires task/mesh shaders and indexed materials"
                    )));
                }
                extensions.push(ash::ext::mesh_shader::NAME.as_ptr());
            }
            let mut mesh = vk::PhysicalDeviceMeshShaderFeaturesEXT::default()
                .task_shader(true)
                .mesh_shader(true);
            let mut device_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queues)
                .enabled_extension_names(&extensions)
                .enabled_features(&features)
                .push_next(&mut vulkan13)
                .push_next(&mut vulkan12);
            if enable_meshlets {
                device_info = device_info.push_next(&mut mesh);
            }
            let device = instance
                .create_device(physical, &device_info, None)
                .map_err(|source| VulkanError::CreateDevice {
                    name: name.clone(),
                    source,
                })?;
            #[cfg(feature = "bench")]
            let meshlet_device =
                enable_meshlets.then(|| ash::ext::mesh_shader::Device::new(&instance, &device));
            let context = Arc::new(DeviceLifetime {
                instance: owner,
                device: device.clone(),
            });
            let material_layout = bindless
                .then(|| MaterialLayout::new(&context, capabilities.max_material_anisotropy))
                .transpose()?
                .map(Arc::new);
            if presentation_target.is_some() {
                crate::presentation::validate_external_format(&context, physical, 1, 1)?;
            }
            let queue = device.get_device_queue(family, 0);
            #[cfg(debug_assertions)]
            eprintln!(
                "Vulkan debug device: {name}, type={:?}, API={}.{}.{}, driver={:#x}, queue={family}, GUI interop={}",
                properties.device_type,
                vk::api_version_major(properties.api_version),
                vk::api_version_minor(properties.api_version),
                vk::api_version_patch(properties.api_version),
                properties.driver_version,
                presentation_target.is_some()
            );
            context.name(device.handle(), || format!("ModuleEngine device ({name})"));
            context.name(queue, || {
                format!("ModuleEngine graphics+compute queue {family}")
            });
            let memory = instance.get_physical_device_memory_properties(physical);
            let timeline = modern
                .then(|| Timeline::new(&context, "ModuleEngine graphics timeline"))
                .transpose()?;
            let transfer = transfer_family
                .map(|family| TransferQueue::new(&context, family))
                .transpose()?;
            let allocator = modern
                .then(|| DeviceAllocator::new(&context, &instance, physical))
                .transpose()?;
            let mut slots = Vec::new();
            for _ in 0..if modern { 2 } else { 1 } {
                slots.push(FrameSlot {
                    pools: FramePools::new(&context, family, transfer_family)?,
                    pending: None,
                    resources: None,
                    atlas: None,
                    last_serial: 0,
                    timestamp_pool: None,
                });
            }
            if let Some(trace) = trace {
                eprintln!(
                    "Vulkan device renderer={trace} adapter={name:?} driver={:#x} tier_a={} tier_b={} tier_c={} timeline={modern} bindless_materials={bindless} graphics_family={family} transfer_family={} slots={} present={}",
                    properties.driver_version,
                    capabilities.tier_a,
                    capabilities.tier_b,
                    capabilities.tier_c,
                    transfer_family.unwrap_or(family),
                    slots.len(),
                    presentation_target.is_some(),
                );
            }
            Ok(Self {
                context,
                instance,
                device,
                physical,
                memory,
                queue,
                family,
                presentation_target,
                presentation_slots: PresentationPool::default(),
                slots,
                serial: 0,
                timeline,
                allocator,
                transfer,
                staging: StagingRing::default(),
                staging_alignment: properties
                    .limits
                    .optimal_buffer_copy_offset_alignment
                    .max(4),
                capabilities,
                material_layout,
                materials: None,
                gpu_scene: resident::GpuScene::default(),
                culling_pipelines: None,
                culling_mode: SceneCulling::default(),
                culling_stats: false,
                #[cfg(feature = "bench")]
                meshlet_device,
                storage_buffer_limit: u64::from(properties.limits.max_storage_buffer_range),
                max_indirect_draws: properties.limits.max_draw_indirect_count,
                trace,
                name,
                pipelines: BTreeMap::new(),
                prepared: None,
                diagnostics: vec![],
                resource_root,
                next_ticket: 0,
                producer_error: None,
                detailed_stats: true,
                frame_profiling: false,
                timestamp_valid_bits,
                timestamp_period_ns: f64::from(properties.limits.timestamp_period),
                prepared_inputs: None,
                rejected_inputs: None,
                performance: RendererPerformance::default(),
                last_frame_work: None,
                #[cfg(test)]
                allocation_fault: None,
                #[cfg(test)]
                allocations: std::sync::Arc::default(),
            })
        }
    }
    pub fn device_name(&self) -> &str {
        &self.name
    }
    /// Installing a different GUI device is transactional: prepare a compatible
    /// backend before retiring the accepted one. Outstanding leases are never
    /// invalidated, and target churn cannot multiply the four-slot pool.
    pub fn set_presentation_target(
        &mut self,
        target: Option<Arc<VulkanPresentationTarget>>,
    ) -> Result<(), VulkanError> {
        if match (&self.presentation_target, &target) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                a.identity == b.identity && Arc::ptr_eq(&a.allocator, &b.allocator)
            }
            _ => false,
        } {
            return Ok(());
        }
        if self.presentation_slots.busy() {
            return Err(VulkanError::Presentation(
                "GPU presentation target still has leased frames".into(),
            ));
        }
        let Some(target) = target else {
            self.presentation_slots.clear();
            self.presentation_target = None;
            return Ok(());
        };
        #[cfg(feature = "bench")]
        let meshlets = self.meshlet_device.is_some();
        #[cfg(not(feature = "bench"))]
        let meshlets = false;
        let mut replacement =
            Self::new_at_target(self.resource_root.clone(), Some(target), meshlets)?;
        replacement.detailed_stats = self.detailed_stats;
        replacement.frame_profiling = self.frame_profiling;
        replacement.next_ticket = self.next_ticket;
        if let Some((graph, sources, _)) = &self.prepared_inputs {
            replacement.prepare(graph, sources)?;
        }
        *self = replacement;
        Ok(())
    }
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
    /// Player mode avoids reading depth/normal/color diagnostics every frame.
    /// Geometry submission counts remain available; visibility counts are omitted.
    pub fn set_detailed_stats(&mut self, enabled: bool) {
        self.detailed_stats = enabled;
    }
    pub fn set_frame_profiling(&mut self, enabled: bool) {
        self.frame_profiling = enabled;
    }
    pub fn performance(&self) -> RendererPerformance {
        self.performance
    }
    pub fn capabilities(&self) -> DeviceCapabilities {
        self.capabilities
    }
    pub fn frame_capacity(&self) -> usize {
        self.slots.len()
    }
    pub fn allocator_stats(&self) -> Option<[u64; 3]> {
        self.allocator.as_ref().map(|allocator| allocator.stats())
    }
    pub fn last_frame_work(&self) -> Option<FrameWork> {
        self.last_frame_work
    }

    /// Inspect the completed synchronous readback slot. Binding numbers are
    /// local to this report. Ranges on the same binding may be unioned to
    /// compare renderers that alias the storage of multiple images.
    #[cfg(feature = "bench")]
    pub fn synchronous_graph_image_allocations(&self) -> Option<Vec<GraphImageAllocation>> {
        let resources = self.slots[0].resources.as_ref()?;
        let mut bindings = BTreeMap::new();
        Some(
            resources
                .images
                .iter()
                .map(|(resource, image)| {
                    let (memory, offset_bytes) = image.memory.as_ref().unwrap().binding();
                    let next_binding = bindings.len();
                    let memory_binding = *bindings.entry(memory).or_insert(next_binding);
                    // The slot owns the image and its bound memory. This query
                    // is diagnostic and neither allocates nor submits work.
                    let required_bytes =
                        unsafe { self.device.get_image_memory_requirements(image.image).size };
                    GraphImageAllocation {
                        resource: resource.clone(),
                        format: format!("{:?}", image.format),
                        memory_binding,
                        offset_bytes,
                        required_bytes,
                    }
                })
                .collect(),
        )
    }

    pub fn set_scene_culling(&mut self, mode: SceneCulling) {
        self.culling_mode = mode;
    }

    pub fn set_culling_stats(&mut self, enabled: bool) {
        self.culling_stats = enabled;
    }

    /// Test the actual buffer size and memory type used by the current path,
    /// without allocating the buffer's backing memory or expanding geometry.
    pub fn probe_vertex_allocation(&self, triangles: usize) -> VertexAllocation {
        let requested_bytes = (triangles as u64)
            .saturating_mul((3 * geometry::VERTEX_BYTES) as u64 * VERTEX_BUFFER_MULTIPLIER)
            .max(4);
        let mut v11 = vk::PhysicalDeviceVulkan11Properties::default();
        let mut v13 = vk::PhysicalDeviceVulkan13Properties::default();
        let mut physical = vk::PhysicalDeviceProperties2::default()
            .push_next(&mut v11)
            .push_next(&mut v13);
        unsafe {
            self.instance
                .get_physical_device_properties2(self.physical, &mut physical)
        };
        let driver_version_raw = physical.properties.driver_version;
        let mut result = VertexAllocation {
            adapter: self.name.clone(),
            driver_version_raw,
            requested_bytes,
            max_buffer_bytes: v13.max_buffer_size,
            max_single_allocation_bytes: v11.max_memory_allocation_size,
            allocation_bytes: None,
            heap_bytes: None,
            heap_budget_bytes: None,
            heap_usage_bytes: None,
            blocked: None,
        };
        if requested_bytes > result.max_buffer_bytes
            || requested_bytes > result.max_single_allocation_bytes
        {
            result.blocked = Some(
                "Legacy vertex buffer exceeds the selected adapter's buffer or allocation limit"
                    .into(),
            );
            return result;
        }
        let requirements = unsafe {
            let buffer = match self.device.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(requested_bytes)
                    .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
                    .sharing_mode(vk::SharingMode::EXCLUSIVE),
                None,
            ) {
                Ok(buffer) => buffer,
                Err(error) => {
                    result.blocked = Some(format!("Create legacy vertex buffer: {error:?}"));
                    return result;
                }
            };
            let requirements = self.device.get_buffer_memory_requirements(buffer);
            self.device.destroy_buffer(buffer, None);
            requirements
        };
        result.allocation_bytes = Some(requirements.size);
        let host = vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
        let memory_type = match self.memory_type(requirements.memory_type_bits, host) {
            Ok(index) => index,
            Err(error) => {
                result.blocked = Some(error.to_string());
                return result;
            }
        };
        let heap = self.memory.memory_types[memory_type as usize].heap_index as usize;
        result.heap_bytes = Some(self.memory.memory_heaps[heap].size);
        if requirements.size > self.memory.memory_heaps[heap].size
            || requirements.size > result.max_single_allocation_bytes
        {
            result.blocked = Some(
                "Legacy vertex allocation exceeds its host-visible heap or allocation limit".into(),
            );
            return result;
        }
        let extensions = unsafe {
            self.instance
                .enumerate_device_extension_properties(self.physical)
        };
        if extensions.is_ok_and(|extensions| {
            extensions.iter().any(|extension| unsafe {
                CStr::from_ptr(extension.extension_name.as_ptr()) == ash::ext::memory_budget::NAME
            })
        }) {
            let mut budget = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
            let mut memory = vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut budget);
            unsafe {
                self.instance
                    .get_physical_device_memory_properties2(self.physical, &mut memory)
            };
            result.heap_budget_bytes = Some(budget.heap_budget[heap]);
            result.heap_usage_bytes = Some(budget.heap_usage[heap]);
            if requirements.size > budget.heap_budget[heap].saturating_sub(budget.heap_usage[heap])
            {
                result.blocked = Some(
                    "Legacy vertex allocation exceeds currently available host-visible heap budget"
                        .into(),
                );
            }
        }
        result
    }
    /// Explicit preparation always fingerprints current include contents. Only
    /// ordinary render calls coalesce unchanged-source dependency scans.
    pub fn prepare(
        &mut self,
        graph: &Graph,
        sources: &BTreeMap<String, String>,
    ) -> Result<(), VulkanError> {
        self.prepare_checked(graph, sources, false)
    }
    fn reject_preparation(
        &mut self,
        graph: &Graph,
        sources: &BTreeMap<String, String>,
        signature: Option<Vec<String>>,
        error: VulkanError,
    ) -> Result<(), VulkanError> {
        self.diagnostics = vec![error.to_string()];
        self.rejected_inputs = Some(RejectedPreparation {
            graph: graph.clone(),
            sources: sources.clone(),
            signature,
            checked: Instant::now(),
            error: error.clone(),
        });
        Err(error)
    }
    fn prepare_checked(
        &mut self,
        graph: &Graph,
        sources: &BTreeMap<String, String>,
        reuse_rejection: bool,
    ) -> Result<(), VulkanError> {
        self.performance.prepare_calls += 1;
        self.performance.plan_builds += 1;
        let started = Instant::now();
        let plan = match build_plan_at(graph, sources, &self.resource_root) {
            Ok(plan) => plan,
            Err(error) => {
                return self.reject_preparation(graph, sources, None, error);
            }
        };
        let signature = plan.passes.iter().map(pipeline_key).collect::<Vec<_>>();
        if reuse_rejection
            && let Some(rejected) = &mut self.rejected_inputs
            && rejected.matches(graph, sources)
            && rejected.signature.as_ref() == Some(&signature)
        {
            rejected.checked = Instant::now();
            self.performance.rejected_cache_hits += 1;
            return Err(rejected.error.clone());
        }
        if let Err(error) = self.prepare_plan(plan) {
            return self.reject_preparation(graph, sources, Some(signature), error);
        }
        self.prepared_inputs = Some((graph.clone(), sources.clone(), Instant::now()));
        self.rejected_inputs = None;
        self.performance.prepare_ns += started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
        self.diagnostics.clear();
        Ok(())
    }
    fn prepare_plan(&mut self, plan: Plan) -> Result<(), VulkanError> {
        if let Err(error) = schedule_overrides(&plan, &RenderOverrides::default()) {
            self.diagnostics = vec![error.to_string()];
            return Err(error);
        }
        if plan.passes.len() > 64 {
            return Err(VulkanError::PassLimit);
        }
        if self.pipelines.len() > 256 {
            let mut keep = BTreeSet::new();
            for pass in plan
                .passes
                .iter()
                .chain(self.prepared.iter().flat_map(|p| &p.passes))
            {
                let key = pipeline_key(pass);
                if pass.has_bindless_variant() && self.material_layout.is_some() {
                    keep.insert(atlas_pipeline_key(&key));
                }
                if pass.resident_geometry() && self.capabilities.resident_scene {
                    keep.insert(resident_pipeline_key(&key));
                    if self.capabilities.gpu_frustum_culling {
                        keep.insert(culled_pipeline_key(&key));
                    }
                }
                keep.insert(key);
            }
            self.pipelines.retain(|key, _| keep.contains(key));
        }
        for pass in &plan.passes {
            let key = pipeline_key(pass);
            self.prepare_pipeline(pass, key.clone(), true, Geometry::Expanded)?;
            if pass.resident_geometry() && self.capabilities.resident_scene {
                self.prepare_pipeline(pass, resident_pipeline_key(&key), true, Geometry::Resident)?;
                if self.capabilities.gpu_frustum_culling {
                    self.prepare_pipeline(pass, culled_pipeline_key(&key), true, Geometry::Culled)?;
                }
            }
        }
        if self.capabilities.gpu_frustum_culling
            && self.culling_pipelines.is_none()
            && plan.passes.iter().any(Pass::resident_geometry)
        {
            self.culling_pipelines = Some(Arc::new(culling::Pipelines::new(self)?));
        }
        self.prepared = Some(plan);
        Ok(())
    }
    fn prepare_pipeline(
        &mut self,
        pass: &Pass,
        key: String,
        allow_indexed: bool,
        geometry: Geometry,
    ) -> Result<(), VulkanError> {
        if self.pipelines.contains_key(&key) {
            return Ok(());
        }
        self.performance.pipeline_build_attempts += 1;
        let pipeline = self
            .create_pipeline(pass, allow_indexed, geometry)
            .map_err(|error| VulkanError::Pass {
                pass: pass.id.clone(),
                source: Arc::new(error),
            })?;
        if crate::compiler::dependency_fingerprint_at(&self.resource_root, &pass.source)?
            != pass.dependency_hash
        {
            return Err(VulkanError::PassDependenciesChanged(pass.id.clone()));
        }
        self.pipelines.insert(key, Arc::new(pipeline));
        self.performance.pipeline_creations += 1;
        Ok(())
    }
    fn prepare_for_render(
        &mut self,
        graph: &Graph,
        sources: &BTreeMap<String, String>,
    ) -> Result<(), VulkanError> {
        if let Some(rejected) = &self.rejected_inputs
            && rejected.matches(graph, sources)
        {
            if rejected.checked.elapsed() < INCLUDE_REFRESH_INTERVAL {
                self.performance.rejected_cache_hits += 1;
                return Err(rejected.error.clone());
            }
            return self.prepare_checked(graph, sources, true);
        }
        if self
            .prepared_inputs
            .as_ref()
            .is_some_and(|(previous, text, checked)| {
                checked.elapsed() < INCLUDE_REFRESH_INTERVAL
                    && previous.execution_eq(graph)
                    && text == sources
            })
        {
            self.performance.plan_cache_hits += 1;
            return Ok(());
        }
        self.prepare_checked(graph, sources, true)
    }
    fn memory_type(
        &self,
        bits: u32,
        required: vk::MemoryPropertyFlags,
    ) -> Result<u32, VulkanError> {
        (0..self.memory.memory_type_count)
            .find(|i| {
                bits & (1 << i) != 0
                    && self.memory.memory_types[*i as usize]
                        .property_flags
                        .contains(required)
            })
            .ok_or(VulkanError::MemoryType { bits, required })
    }
    fn allocate(
        &self,
        name: &str,
        mut requirements: vk::MemoryRequirements,
        memory_type: u32,
        linear: bool,
        dedicated: Option<AllocationScheme>,
    ) -> Result<ResourceMemory, VulkanError> {
        if let Some(allocator) = &self.allocator {
            requirements.memory_type_bits = 1 << memory_type;
            return allocator.allocate(
                name,
                requirements,
                linear,
                dedicated.unwrap_or(AllocationScheme::GpuAllocatorManaged),
            );
        }
        let mut dedicated_info = vk::MemoryDedicatedAllocateInfo::default();
        let mut info = vk::MemoryAllocateInfo::default()
            .allocation_size(requirements.size)
            .memory_type_index(memory_type);
        if let Some(scheme) = dedicated {
            dedicated_info = match scheme {
                AllocationScheme::DedicatedBuffer(buffer) => dedicated_info.buffer(buffer),
                AllocationScheme::DedicatedImage(image) => dedicated_info.image(image),
                AllocationScheme::GpuAllocatorManaged => unreachable!(),
            };
            info = info.push_next(&mut dedicated_info);
        }
        let memory = unsafe { self.device.allocate_memory(&info, None) }
            .map_err(VulkanError::api("allocate resource memory"))?;
        Ok(ResourceMemory::Standalone(self.context.clone(), memory))
    }
    fn buffer(&self, size: u64, usage: vk::BufferUsageFlags) -> Result<Buffer, VulkanError> {
        self.allocate_buffer(size, usage, false)
    }

    fn device_buffer(&self, size: u64, usage: vk::BufferUsageFlags) -> Result<Buffer, VulkanError> {
        self.allocate_buffer(
            size,
            usage | vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::TRANSFER_SRC,
            true,
        )
    }

    fn allocate_buffer(
        &self,
        size: u64,
        usage: vk::BufferUsageFlags,
        device_local: bool,
    ) -> Result<Buffer, VulkanError> {
        unsafe {
            let size = size.max(4);
            let buffer = self
                .device
                .create_buffer(
                    &vk::BufferCreateInfo::default()
                        .size(size)
                        .usage(usage)
                        .sharing_mode(vk::SharingMode::EXCLUSIVE),
                    None,
                )
                .map_err(VulkanError::api("create buffer"))?;
            self.context
                .name(buffer, || format!("ModuleEngine {usage:?} buffer"));
            #[cfg(test)]
            self.allocations
                .buffers
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let mut allocated = Buffer {
                device: self.device.clone(),
                buffer,
                memory: None,
                size,
                #[cfg(test)]
                allocations: self.allocations.clone(),
            };
            #[cfg(test)]
            self.allocation_checkpoint(AllocationFault::BufferHandle)?;
            let mut dedicated = vk::MemoryDedicatedRequirements::default();
            let mut requirements = vk::MemoryRequirements2::default().push_next(&mut dedicated);
            self.device.get_buffer_memory_requirements2(
                &vk::BufferMemoryRequirementsInfo2::default().buffer(buffer),
                &mut requirements,
            );
            let requirements = requirements.memory_requirements;
            let host =
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT;
            // CPU frame reads must prefer cached system memory. On a discrete
            // adapter the first HOST_VISIBLE type can be an uncached BAR mapping,
            // whose read bandwidth dominates a 1080p frame despite fast GPU work.
            let memory_type = if device_local {
                self.memory_type(
                    requirements.memory_type_bits,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                )?
            } else if usage.contains(vk::BufferUsageFlags::TRANSFER_DST) {
                self.memory_type(
                    requirements.memory_type_bits,
                    host | vk::MemoryPropertyFlags::HOST_CACHED,
                )
                .or_else(|_| self.memory_type(requirements.memory_type_bits, host))?
            } else {
                self.memory_type(requirements.memory_type_bits, host)?
            };
            allocated.memory = Some(
                self.allocate(
                    "buffer",
                    requirements,
                    memory_type,
                    true,
                    (dedicated.requires_dedicated_allocation != 0)
                        .then_some(AllocationScheme::DedicatedBuffer(buffer)),
                )?,
            );
            #[cfg(test)]
            self.allocations
                .memory
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            #[cfg(test)]
            self.allocation_checkpoint(AllocationFault::BufferMemory)?;
            let (memory, offset) = allocated.memory.as_ref().unwrap().binding();
            self.device
                .bind_buffer_memory(buffer, memory, offset)
                .map_err(VulkanError::api("bind buffer memory"))?;
            Ok(allocated)
        }
    }
    fn image(
        &self,
        width: u32,
        height: u32,
        format: vk::Format,
        depth: bool,
    ) -> Result<Image, VulkanError> {
        unsafe {
            let props = self
                .instance
                .get_physical_device_format_properties(self.physical, format);
            let required = if depth {
                vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT
                    | vk::FormatFeatureFlags::TRANSFER_SRC
            } else {
                vk::FormatFeatureFlags::COLOR_ATTACHMENT
                    | vk::FormatFeatureFlags::STORAGE_IMAGE
                    | vk::FormatFeatureFlags::SAMPLED_IMAGE
                    | vk::FormatFeatureFlags::TRANSFER_SRC
                    | vk::FormatFeatureFlags::TRANSFER_DST
            };
            if !props.optimal_tiling_features.contains(required) {
                return Err(VulkanError::FormatSupport {
                    adapter: self.name.clone(),
                    format,
                    required,
                });
            }
            let usage = if depth {
                vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_SRC
            } else {
                vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | vk::ImageUsageFlags::STORAGE
                    | vk::ImageUsageFlags::SAMPLED
                    | vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST
            };
            let info = vk::ImageCreateInfo::default()
                .image_type(vk::ImageType::TYPE_2D)
                .format(format)
                .extent(vk::Extent3D {
                    width,
                    height,
                    depth: 1,
                })
                .mip_levels(1)
                .array_layers(1)
                .samples(vk::SampleCountFlags::TYPE_1)
                .tiling(vk::ImageTiling::OPTIMAL)
                .usage(usage)
                .initial_layout(vk::ImageLayout::UNDEFINED);
            let aspect = if depth {
                vk::ImageAspectFlags::DEPTH
            } else {
                vk::ImageAspectFlags::COLOR
            };
            self.allocate_image(&info, aspect)
        }
    }
    fn allocate_image(
        &self,
        info: &vk::ImageCreateInfo<'_>,
        aspect: vk::ImageAspectFlags,
    ) -> Result<Image, VulkanError> {
        unsafe {
            let image = self
                .device
                .create_image(info, None)
                .map_err(VulkanError::api("create image"))?;
            #[cfg(test)]
            self.allocations
                .images
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let mut allocated = Image {
                device: self.device.clone(),
                image,
                view: vk::ImageView::null(),
                memory: None,
                format: info.format,
                aspect,
                layout: vk::ImageLayout::UNDEFINED,
                stage: vk::PipelineStageFlags::TOP_OF_PIPE,
                access: vk::AccessFlags::empty(),
                #[cfg(test)]
                allocations: self.allocations.clone(),
            };
            #[cfg(test)]
            self.allocation_checkpoint(AllocationFault::ImageHandle)?;
            let mut dedicated = vk::MemoryDedicatedRequirements::default();
            let mut requirements = vk::MemoryRequirements2::default().push_next(&mut dedicated);
            self.device.get_image_memory_requirements2(
                &vk::ImageMemoryRequirementsInfo2::default().image(image),
                &mut requirements,
            );
            let requirements = requirements.memory_requirements;
            let memory_type = self.memory_type(
                requirements.memory_type_bits,
                vk::MemoryPropertyFlags::DEVICE_LOCAL,
            )?;
            allocated.memory = Some(
                self.allocate(
                    "image",
                    requirements,
                    memory_type,
                    false,
                    (dedicated.requires_dedicated_allocation != 0)
                        .then_some(AllocationScheme::DedicatedImage(image)),
                )?,
            );
            #[cfg(test)]
            self.allocations
                .memory
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            #[cfg(test)]
            self.allocation_checkpoint(AllocationFault::ImageMemory)?;
            let (memory, offset) = allocated.memory.as_ref().unwrap().binding();
            self.device
                .bind_image_memory(image, memory, offset)
                .map_err(VulkanError::api("bind image memory"))?;
            let view = self
                .device
                .create_image_view(
                    &vk::ImageViewCreateInfo::default()
                        .image(image)
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(info.format)
                        .subresource_range(range(aspect).level_count(info.mip_levels)),
                    None,
                )
                .map_err(VulkanError::api("create image view"))?;
            allocated.view = view;
            #[cfg(test)]
            self.allocations
                .views
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            #[cfg(test)]
            self.allocation_checkpoint(AllocationFault::ImageView)?;
            Ok(allocated)
        }
    }
    #[cfg(test)]
    fn allocation_checkpoint(&self, stage: AllocationFault) -> Result<(), VulkanError> {
        if self.allocation_fault == Some(stage) {
            Err(VulkanError::Resource(format!(
                "Injected Vulkan allocation failure: {stage:?}"
            )))
        } else {
            Ok(())
        }
    }
    fn stage_atlas(&mut self, bytes: &[u8]) -> Result<StagingUpload, VulkanError> {
        let size = bytes.len() as u64;
        let (buffer, ring_range) = if let Some(timeline) = &self.timeline {
            let completed = timeline
                .counter()
                .map_err(VulkanError::api("query completed staging ranges"))?;
            if self
                .staging
                .buffer
                .as_ref()
                .is_none_or(|buffer| buffer.size < size)
            {
                let capacity = (size + self.staging_alignment)
                    .next_power_of_two()
                    .max(1024 * 1024);
                let buffer = self.buffer(capacity, vk::BufferUsageFlags::TRANSFER_SRC)?;
                self.staging = StagingRing {
                    buffer: Some(Arc::new(buffer)),
                    cursor: StagingCursor::default(),
                };
            }
            let ring = self.staging.buffer.as_ref().unwrap();
            match self
                .staging
                .cursor
                .reserve(size, ring.size, self.staging_alignment, completed)
            {
                Some(region) => (ring.clone(), Some(region)),
                // One temporary upload can belong to each pending slot.
                None => (
                    Arc::new(self.buffer(size, vk::BufferUsageFlags::TRANSFER_SRC)?),
                    None,
                ),
            }
        } else {
            (
                Arc::new(self.buffer(size, vk::BufferUsageFlags::TRANSFER_SRC)?),
                None,
            )
        };
        let offset = ring_range.as_ref().map_or(0, |range| range.start);
        unsafe {
            buffer
                .memory
                .as_ref()
                .unwrap()
                .access(buffer.size, |mapped| {
                    std::ptr::copy_nonoverlapping(
                        bytes.as_ptr(),
                        mapped.add(offset as usize),
                        bytes.len(),
                    );
                })?;
        }
        Ok(StagingUpload {
            buffer,
            offset,
            ring_range,
        })
    }
    fn create_pipeline(
        &self,
        pass: &Pass,
        allow_indexed: bool,
        geometry: Geometry,
    ) -> Result<Pipeline, VulkanError> {
        unsafe {
            let compute = pass.manifest.stage == "compute";
            let profile = if compute { "cs_6_0" } else { "ps_6_0" };
            let bindless = self
                .material_layout
                .as_ref()
                .filter(|_| allow_indexed && pass.has_bindless_variant());
            let variant = bindless.map(|_| {
                if geometry == Geometry::Culled {
                    culled_fragment_source(pass)
                } else {
                    bindless_source(pass)
                }
            });
            let source = variant.as_deref().unwrap_or(&pass.source);
            #[cfg(feature = "bench")]
            let (source, profile) = if geometry.meshlets() {
                (meshlet_probe::SOURCE, "ps_6_5")
            } else {
                (source, profile)
            };
            let shader =
                compile_hlsl_at(&self.resource_root, source, &pass.manifest.entry, profile)?;
            if bindless.is_some() {
                verify_bindless_shader(pass, &shader.words)?;
            } else {
                verify_shader(pass, &shader.words, false)?;
            }
            if pass.module == "gpu.custom" && pass.has_bindless_variant() {
                if bindless.is_some() {
                    let compatibility = compile_hlsl_at(
                        &self.resource_root,
                        &pass.source,
                        &pass.manifest.entry,
                        profile,
                    )?;
                    verify_shader(pass, &compatibility.words, false)?;
                } else {
                    let indexed = compile_hlsl_at(
                        &self.resource_root,
                        &bindless_source(pass),
                        &pass.manifest.entry,
                        profile,
                    )?;
                    verify_bindless_shader(pass, &indexed.words)?;
                }
            }
            // Compile geometry stages before allocating Vulkan handles.
            let mut geometry_artifacts = Vec::with_capacity(2);
            if !compute && !geometry.meshlets() {
                let culled_source;
                let vertex_source = if geometry == Geometry::Culled {
                    culled_source = resident::culled_vertex_source();
                    &culled_source
                } else if geometry == Geometry::Resident {
                    resident::VERTEX_SHADER
                } else if pass.manifest.vertex_entry.is_some() {
                    pass.source.as_str()
                } else if pass.geometry() {
                    include_str!("../../../assets/gpu/forward.hlsl")
                } else {
                    include_str!("../../../assets/gpu/fullscreen.hlsl")
                };
                let entry = pass.manifest.vertex_entry.as_deref().unwrap_or("mainVS");
                let artifact =
                    compile_hlsl_at(&self.resource_root, vertex_source, entry, "vs_6_0")?;
                if geometry != Geometry::Expanded {
                    verify_resident_vertex(&artifact.words, geometry == Geometry::Culled)?;
                } else {
                    verify_shader(pass, &artifact.words, true)?;
                }
                geometry_artifacts.push((vk::ShaderStageFlags::VERTEX, entry, "vs_6_0", artifact));
            }
            #[cfg(feature = "bench")]
            if geometry.meshlets() {
                for (stage, entry, profile) in [
                    (vk::ShaderStageFlags::TASK_EXT, "mainTask", "as_6_5"),
                    (vk::ShaderStageFlags::MESH_EXT, "mainMesh", "ms_6_5"),
                ] {
                    let artifact = compile_hlsl_at(
                        &self.resource_root,
                        meshlet_probe::SOURCE,
                        entry,
                        profile,
                    )?;
                    geometry_artifacts.push((stage, entry, profile, artifact));
                }
            }
            let inputs = pass
                .manifest
                .inputs
                .iter()
                .filter(|s| s.format != "scene")
                .count()
                + usize::from(pass.manifest.scene_materials && bindless.is_none());
            let descriptor_count = inputs
                + if compute {
                    pass.manifest.outputs.len()
                } else {
                    0
                };
            let mut bindings: Vec<_> = (0..descriptor_count)
                .map(|i| {
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(i as u32)
                        .descriptor_type(if i < inputs {
                            vk::DescriptorType::SAMPLED_IMAGE
                        } else {
                            vk::DescriptorType::STORAGE_IMAGE
                        })
                        .descriptor_count(1)
                        .stage_flags(if compute {
                            vk::ShaderStageFlags::COMPUTE
                        } else {
                            vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT
                        })
                })
                .collect();
            if geometry != Geometry::Expanded {
                let last_binding = if geometry == Geometry::Culled || geometry.meshlets() {
                    5
                } else {
                    4
                };
                let stages = if geometry.meshlets() {
                    vk::ShaderStageFlags::MESH_EXT
                } else {
                    vk::ShaderStageFlags::VERTEX
                };
                bindings.extend((1..=last_binding).map(|binding| {
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(binding)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .descriptor_count(1)
                        .stage_flags(stages)
                }));
            }
            let set_layout = self
                .device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                    None,
                )
                .map_err(VulkanError::api("create descriptor set layout"))?;
            let mut pipeline = Pipeline {
                device: self.device.clone(),
                pipeline: vk::Pipeline::null(),
                layout: vk::PipelineLayout::null(),
                set_layout,
                compute,
                bindless_materials: bindless.is_some(),
                geometry,
            };
            self.context
                .name(set_layout, || format!("{} descriptor layout", pass.id));
            let mut layouts = vec![set_layout];
            layouts.extend(bindless.map(|material| material.layout));
            let push = [vk::PushConstantRange {
                stage_flags: geometry.push_stages(),
                offset: 0,
                size: 128,
            }];
            pipeline.layout = self
                .device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&layouts)
                        .push_constant_ranges(&push),
                    None,
                )
                .map_err(VulkanError::api("create pipeline layout"))?;
            let layout = pipeline.layout;
            self.context
                .name(layout, || format!("{} pipeline layout", pass.id));
            let module = self
                .device
                .create_shader_module(
                    &vk::ShaderModuleCreateInfo::default().code(&shader.words),
                    None,
                )
                .map_err(VulkanError::api("create shader module"))?;
            let module = OwnedShaderModule(&self.device, module);
            self.context.name(module.1, || {
                format!("{} {profile} {}", pass.id, pass.manifest.entry)
            });
            let entry = CString::new(pass.manifest.entry.as_str()).map_err(|source| {
                VulkanError::ShaderEntry {
                    stage: "Shader",
                    source,
                }
            })?;
            let stage = vk::PipelineShaderStageCreateInfo::default()
                .stage(if compute {
                    vk::ShaderStageFlags::COMPUTE
                } else {
                    vk::ShaderStageFlags::FRAGMENT
                })
                .module(module.1)
                .name(&entry);
            let result = if compute {
                self.device
                    .create_compute_pipelines(
                        vk::PipelineCache::null(),
                        &[vk::ComputePipelineCreateInfo::default()
                            .stage(stage)
                            .layout(layout)],
                        None,
                    )
                    .map(|p| p[0])
                    .map_err(|(partial, source)| {
                        for handle in partial {
                            self.device.destroy_pipeline(handle, None);
                        }
                        VulkanError::api("create compute pipeline")(source)
                    })
            } else {
                let geometry_modules = geometry_artifacts
                    .iter()
                    .map(|(stage, entry, profile, artifact)| {
                        let module = self
                            .device
                            .create_shader_module(
                                &vk::ShaderModuleCreateInfo::default().code(&artifact.words),
                                None,
                            )
                            .map_err(VulkanError::api("create geometry shader module"))?;
                        let module = OwnedShaderModule(&self.device, module);
                        self.context
                            .name(module.1, || format!("{} {profile} {entry}", pass.id));
                        let entry =
                            CString::new(*entry).map_err(|source| VulkanError::ShaderEntry {
                                stage: "Geometry",
                                source,
                            })?;
                        Ok((*stage, entry, module))
                    })
                    .collect::<Result<Vec<_>, VulkanError>>()?;
                let stages: Vec<_> = geometry_modules
                    .iter()
                    .map(|(stage, entry, module)| {
                        vk::PipelineShaderStageCreateInfo::default()
                            .stage(*stage)
                            .module(module.1)
                            .name(entry)
                    })
                    .chain(std::iter::once(stage))
                    .collect();
                let vertex_bindings = [vk::VertexInputBindingDescription {
                    binding: 0,
                    stride: 72,
                    input_rate: vk::VertexInputRate::VERTEX,
                }];
                let attributes = [
                    vk::VertexInputAttributeDescription {
                        location: 0,
                        binding: 0,
                        format: vk::Format::R32G32B32_SFLOAT,
                        offset: 0,
                    },
                    vk::VertexInputAttributeDescription {
                        location: 1,
                        binding: 0,
                        format: vk::Format::R32G32B32_SFLOAT,
                        offset: 12,
                    },
                    vk::VertexInputAttributeDescription {
                        location: 2,
                        binding: 0,
                        format: vk::Format::R32G32B32A32_SFLOAT,
                        offset: 24,
                    },
                    vk::VertexInputAttributeDescription {
                        location: 3,
                        binding: 0,
                        format: vk::Format::R32G32_SFLOAT,
                        offset: 40,
                    },
                    vk::VertexInputAttributeDescription {
                        location: 4,
                        binding: 0,
                        format: vk::Format::R32G32_SFLOAT,
                        offset: 48,
                    },
                    vk::VertexInputAttributeDescription {
                        location: 5,
                        binding: 0,
                        format: vk::Format::R32G32B32A32_SFLOAT,
                        offset: 56,
                    },
                ];
                let vertex_input = if pass.geometry() && geometry == Geometry::Expanded {
                    vk::PipelineVertexInputStateCreateInfo::default()
                        .vertex_binding_descriptions(&vertex_bindings)
                        .vertex_attribute_descriptions(&attributes)
                } else {
                    vk::PipelineVertexInputStateCreateInfo::default()
                };
                let assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                    .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
                let viewport = vk::PipelineViewportStateCreateInfo::default()
                    .viewport_count(1)
                    .scissor_count(1);
                let raster = vk::PipelineRasterizationStateCreateInfo::default()
                    .polygon_mode(vk::PolygonMode::FILL)
                    .cull_mode(vk::CullModeFlags::NONE)
                    .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
                    .line_width(1.);
                let multi = vk::PipelineMultisampleStateCreateInfo::default()
                    .rasterization_samples(vk::SampleCountFlags::TYPE_1);
                let depth = vk::PipelineDepthStencilStateCreateInfo::default()
                    .depth_test_enable(pass.geometry())
                    .depth_write_enable(pass.geometry())
                    .depth_compare_op(vk::CompareOp::LESS);
                let attachments: Vec<_> = pass
                    .manifest
                    .outputs
                    .iter()
                    .map(|_| {
                        vk::PipelineColorBlendAttachmentState::default()
                            .color_write_mask(vk::ColorComponentFlags::RGBA)
                    })
                    .collect();
                let blend =
                    vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);
                let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
                let dynamic =
                    vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
                let formats: Vec<_> = pass
                    .manifest
                    .outputs
                    .iter()
                    .map(|s| format(&s.format))
                    .collect::<Result<_, _>>()?;
                let mut rendering = vk::PipelineRenderingCreateInfo::default()
                    .color_attachment_formats(&formats)
                    .depth_attachment_format(if pass.geometry() {
                        vk::Format::D32_SFLOAT
                    } else {
                        vk::Format::UNDEFINED
                    });
                let mut info = vk::GraphicsPipelineCreateInfo::default()
                    .stages(&stages)
                    .viewport_state(&viewport)
                    .rasterization_state(&raster)
                    .multisample_state(&multi)
                    .depth_stencil_state(&depth)
                    .color_blend_state(&blend)
                    .dynamic_state(&dynamic)
                    .layout(layout)
                    .push_next(&mut rendering);
                if !geometry.meshlets() {
                    info = info
                        .vertex_input_state(&vertex_input)
                        .input_assembly_state(&assembly);
                }
                self.device
                    .create_graphics_pipelines(vk::PipelineCache::null(), &[info], None)
                    .map(|p| p[0])
                    .map_err(|(partial, source)| {
                        for handle in partial {
                            self.device.destroy_pipeline(handle, None);
                        }
                        VulkanError::api("create graphics pipeline")(source)
                    })
            };
            pipeline.pipeline = result?;
            self.context.name(pipeline.pipeline, || {
                format!("{} ({}) pipeline", pass.id, pass.module)
            });
            Ok(pipeline)
        }
    }
    #[allow(clippy::too_many_arguments)] // Public integration boundary fixed by the shared renderer contract.
    pub fn render(
        &mut self,
        scene: &SceneSnapshot,
        camera: Camera,
        width: usize,
        height: usize,
        graph: &Graph,
        sources: &BTreeMap<String, String>,
        overrides: &RenderOverrides,
    ) -> Result<Frame, VulkanError> {
        self.render_scene(
            SceneInput::Snapshot(scene),
            camera,
            width,
            height,
            graph,
            sources,
            overrides,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_scene(
        &mut self,
        scene: SceneInput<'_>,
        camera: Camera,
        width: usize,
        height: usize,
        graph: &Graph,
        sources: &BTreeMap<String, String>,
        overrides: &RenderOverrides,
    ) -> Result<Frame, VulkanError> {
        if self.slots.iter().any(|slot| slot.pending.is_some()) {
            return Err(VulkanError::ProducerBusy);
        }
        self.render_output(
            scene, camera, width, height, graph, sources, overrides, None, 0,
        )
        .and_then(|frame| frame.ok_or(VulkanError::NoReadback))
    }
    /// Render to one of four leased shared images, without host pixel transfer.
    /// Backpressure does no GPU work and leaves portable resource commands pending.
    #[allow(clippy::too_many_arguments)]
    pub fn render_presented(
        &mut self,
        scene: &SceneSnapshot,
        camera: Camera,
        width: usize,
        height: usize,
        graph: &Graph,
        sources: &BTreeMap<String, String>,
        overrides: &RenderOverrides,
    ) -> Result<RenderSubmission, VulkanError> {
        self.render_scene_presented(
            SceneInput::Snapshot(scene),
            camera,
            width,
            height,
            graph,
            sources,
            overrides,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render_scene_presented(
        &mut self,
        scene: SceneInput<'_>,
        camera: Camera,
        width: usize,
        height: usize,
        graph: &Graph,
        sources: &BTreeMap<String, String>,
        overrides: &RenderOverrides,
    ) -> Result<RenderSubmission, VulkanError> {
        if width == 0
            || height == 0
            || width > 8192
            || height > 8192
            || width
                .checked_mul(height)
                .is_none_or(|pixels| pixels > 16_777_216)
        {
            return Err(VulkanError::Viewport);
        }
        let Some(target) = self.presentation_target.clone() else {
            // No presentation target is the explicit legacy/readback mode used
            // by headless callers. A rejected requested target never falls back.
            return self
                .render_scene(scene, camera, width, height, graph, sources, overrides)
                .map(RenderSubmission::Ready);
        };
        if let Some(error) = &self.producer_error {
            return Err(error.clone());
        }
        for index in 0..self.slots.len() {
            if let Some(flight) = &self.slots[index].pending
                && flight.cancelled.load(Ordering::Acquire)
            {
                let _ = self.poll_presented(flight.ticket)?;
            }
        }
        let Some(index) = self.slots.iter().position(|slot| slot.pending.is_none()) else {
            self.performance.gpu_busy_skips += 1;
            return Ok(RenderSubmission::Busy);
        };
        let size = [width as u32, height as u32];
        let context = self.context.clone();
        let physical = self.physical;
        let memory = self.memory;
        let Some(slot) = self.presentation_slots.reserve(size, || {
            let shared = target
                .allocator
                .allocate(size[0], size[1])
                .map_err(VulkanError::Presentation)?;
            SharedImage::import(context, physical, &memory, size[0], size[1], shared)
        })?
        else {
            self.performance.gpu_busy_skips += 1;
            return Ok(RenderSubmission::Busy);
        };
        self.render_output(
            scene,
            camera,
            width,
            height,
            graph,
            sources,
            overrides,
            Some(&slot),
            index,
        )?;
        Ok(RenderSubmission::Pending(self.next_ticket))
    }

    pub(crate) fn cancellation(&self, ticket: u64) -> Option<Arc<AtomicBool>> {
        self.slots
            .iter()
            .filter_map(|slot| slot.pending.as_ref())
            .find(|flight| flight.ticket == ticket)
            .map(|flight| flight.cancelled.clone())
    }
    /// Only this exact request can complete; polling never submits or waits.
    pub fn poll_presented(&mut self, ticket: u64) -> Result<Option<Frame>, VulkanError> {
        if ticket == 0 || ticket > self.next_ticket {
            return Err(VulkanError::UnknownTicket(ticket));
        }
        if let Some(error) = &self.producer_error {
            return Err(error.clone());
        }
        let Some(slot) = self.slots.iter_mut().find(|slot| {
            slot.pending
                .as_ref()
                .is_some_and(|flight| flight.ticket == ticket)
        }) else {
            return Ok(None);
        };
        let flight = slot.pending.as_mut().unwrap();
        self.performance.gpu_completion_polls += 1;
        match flight.completion.ready() {
            Ok(false) => return Ok(None),
            Err(error) => {
                let error = VulkanError::ProducerUnconfirmed {
                    operation: "producer completion",
                    source: error,
                };
                flight.failure = Some(error.clone());
                self.producer_error = Some(error.clone());
                return Err(error);
            }
            Ok(true) => {}
        }
        let mut flight = slot.pending.take().unwrap();
        if let Some(trace) = self.trace {
            eprintln!(
                "Vulkan frame complete renderer={trace} serial={} ticket={ticket}",
                flight.serial
            );
        }
        let shared = flight
            .shared
            .as_ref()
            .expect("native producer shared image");
        shared.completed();
        if let Some(culling) = &flight.resources.culling {
            flight.work.culling_stats = culling.completed()?;
        }
        let frame = if flight.cancelled.load(Ordering::Acquire) {
            None
        } else {
            self.performance.gpu_presentation_frames += 1;
            self.performance.gpu_presentation_bytes += (flight.size[0] * flight.size[1] * 4) as u64;
            Some(Frame {
                width: flight.size[0],
                height: flight.size[1],
                rgba: Vec::new(),
                gpu: Some(shared.frame()),
                stats: flight.stats,
                timings: flight
                    .timings
                    .as_ref()
                    .map(|timings| Box::new(timings.completed())),
            })
        };
        self.last_frame_work = Some(flight.work);
        slot.resources = Some(flight.resources);
        slot.atlas = flight.atlas;
        Ok(frame)
    }
    pub fn discard_presented(&mut self, ticket: u64) -> Result<(), VulkanError> {
        if ticket == 0 || ticket > self.next_ticket {
            return Err(VulkanError::UnknownTicket(ticket));
        }
        if let Some(flight) = self
            .slots
            .iter()
            .filter_map(|slot| slot.pending.as_ref())
            .find(|flight| flight.ticket == ticket)
        {
            flight.cancelled.store(true, Ordering::Release);
        }
        // Do not wait and do not fabricate completion, even on a fast device.
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn render_output(
        &mut self,
        mut input: SceneInput<'_>,
        camera: Camera,
        width: usize,
        height: usize,
        graph: &Graph,
        sources: &BTreeMap<String, String>,
        overrides: &RenderOverrides,
        presentation: Option<&Arc<SharedImage>>,
        index: usize,
    ) -> Result<Option<Frame>, VulkanError> {
        if let Some(error) = &self.producer_error {
            return Err(error.clone());
        }
        if width == 0
            || height == 0
            || width > 8192
            || height > 8192
            || width.checked_mul(height).is_none_or(|p| p > 16_777_216)
        {
            return Err(VulkanError::Viewport);
        }
        let serial = self
            .serial
            .checked_add(1)
            .ok_or(VulkanError::TicketExhausted)?;
        let previous = self.slots[index].last_serial;
        if let Some(trace) = self.trace {
            let completed = self
                .timeline
                .as_ref()
                .and_then(|timeline| timeline.counter().ok());
            eprintln!(
                "Vulkan frame begin renderer={trace} serial={serial} slot={index} previous={previous} latest={} completed={completed:?}",
                self.serial
            );
        }
        let prepare_started = Instant::now();
        self.performance.render_calls += 1;
        self.prepare_for_render(graph, sources)?;
        let plan = self.prepared.as_ref().unwrap().clone();
        let operations = schedule_overrides(&plan, overrides)?;
        let command = self.slots[index].pools.graphics.command;
        let descriptor_pool = self.slots[index].pools.descriptors;
        let active_passes = || {
            plan.passes
                .iter()
                .filter(|pass| pass_enabled(pass, overrides))
        };
        #[cfg(feature = "bench")]
        let meshlets = match &input {
            SceneInput::Meshlets(source) => {
                if self.meshlet_device.is_none() || !Arc::ptr_eq(&self.context, &source.context) {
                    return Err(VulkanError::Resource(
                        "R8.1 meshlet geometry belongs to another device".into(),
                    ));
                }
                let mut passes = active_passes().filter(|pass| pass.geometry());
                let pass = passes
                    .next()
                    .filter(|pass| pass.module == "gpu.forward" && passes.next().is_none())
                    .ok_or_else(|| {
                        VulkanError::Resource("R8.1 uses one stock Forward scene pass".into())
                    })?;
                self.prepare_pipeline(
                    pass,
                    meshlet_pipeline_key(&pipeline_key(pass)),
                    true,
                    Geometry::Meshlets,
                )?;
                Some(&**source)
            }
            _ => None,
        };
        #[cfg(feature = "bench")]
        let meshlet_frame = meshlets.is_some();
        #[cfg(not(feature = "bench"))]
        let meshlet_frame = false;
        let resident = match &input {
            SceneInput::Retained(source)
                if self.capabilities.resident_scene
                    && active_passes().any(Pass::geometry)
                    && active_passes()
                        .filter(|pass| pass.geometry())
                        .all(Pass::resident_geometry) =>
            {
                match self.gpu_scene.prepare(self, source) {
                    Err(VulkanError::SceneBufferLimit) => None,
                    other => other?,
                }
            }
            _ => None,
        };
        let scene = input.snapshot()?;
        let objects = scene.items.len();
        let wants_indexed =
            self.material_layout.is_some() && active_passes().any(Pass::has_bindless_variant);
        let indexed = if resident.is_none() && wants_indexed {
            self.prepare_materials(scene)?
        } else {
            None
        };
        if meshlet_frame && indexed.is_none() {
            return Err(VulkanError::Resource(
                "R8.1 could not bind the indexed Forward materials".into(),
            ));
        }
        let atlas_overflow_fallback = resident.is_none() && wants_indexed && indexed.is_none();
        let needs_atlas = active_passes().any(|pass| {
            pass.manifest.scene_materials
                && (indexed.is_none() && resident.is_none() || !pass.has_bindless_variant())
        });
        let (materials, material_uploads) = indexed.map_or_else(
            || (None, materials::MaterialUploads::default()),
            |(materials, uploads)| (Some(materials), uploads),
        );
        let mut atlas = None;
        let mut atlas_needs_upload = false;
        if needs_atlas {
            let keys = geometry::texture_keys(scene);
            atlas = self.slots[index].atlas.take().filter(|a| a.keys == keys);
            if atlas.is_none() {
                let decoded = geometry::texture_atlas(scene)?;
                let image = self.image(
                    decoded.width,
                    decoded.height,
                    vk::Format::R8G8B8A8_UNORM,
                    false,
                )?;
                self.context.name(image.image, || "Base-color atlas".into());
                self.context
                    .name(image.view, || "Base-color atlas view".into());
                atlas = Some(AtlasGpu {
                    keys,
                    atlas: decoded,
                    image,
                });
                atlas_needs_upload = true;
            }
        }
        if atlas_overflow_fallback {
            for pass in active_passes().filter(|pass| pass.has_bindless_variant()) {
                self.prepare_pipeline(
                    pass,
                    atlas_pipeline_key(&pipeline_key(pass)),
                    false,
                    Geometry::Expanded,
                )?;
            }
        }
        let no_rects = BTreeMap::new();
        let rects = atlas.as_ref().map_or(&no_rects, |a| &a.atlas.rects);
        let (vertex_bytes, submitted) = match &resident {
            Some(resident) => (Vec::new(), resident.triangles),
            #[cfg(feature = "bench")]
            None if meshlet_frame => (
                Vec::new(),
                meshlets.unwrap().build_report().source_triangles,
            ),
            None => geometry::legacy_vertices(scene, rects),
        };
        let readback = if presentation.is_some() {
            Vec::new()
        } else {
            frame_readback(&plan, width, height, self.detailed_stats)
        };
        let resource_key = frame_resource_key(&plan, width, height, &readback);
        let mut reused = self.slots[index]
            .resources
            .take()
            .filter(|r| r.key == resource_key);
        let mut vertex = reused
            .as_mut()
            .and_then(|resources| resources.vertex.take());
        let mut scene_culling = if resident
            .as_ref()
            .is_some_and(|r| r.buffers.culling.is_some())
        {
            culling::Frame::prepare(
                self,
                reused
                    .as_mut()
                    .and_then(|resources| resources.culling.take()),
                [width as u32, height as u32],
                objects,
            )?
        } else {
            None
        };
        if resident.is_none() && !meshlet_frame {
            if vertex
                .as_ref()
                .is_none_or(|buffer| buffer.size < vertex_bytes.len() as u64)
            {
                vertex = Some(self.buffer(
                    vertex_bytes.len() as u64 * VERTEX_BUFFER_MULTIPLIER,
                    vk::BufferUsageFlags::VERTEX_BUFFER,
                )?);
            }
            let buffer = vertex.as_ref().unwrap();
            // SAFETY: The staging data is separate from the mapped buffer and
            // fits its allocation. The frame slot is available for CPU writes
            // because any previous GPU submission using this slot completed.
            unsafe {
                buffer
                    .memory
                    .as_ref()
                    .unwrap()
                    .access(buffer.size, |mapped| {
                        std::ptr::copy_nonoverlapping(
                            vertex_bytes.as_ptr(),
                            mapped,
                            vertex_bytes.len(),
                        );
                    })?;
            }
        }
        let mut images = reused
            .as_mut()
            .map(|r| std::mem::take(&mut r.images))
            .unwrap_or_default();
        if images.is_empty() {
            for pass in &plan.passes {
                for spec in &pass.manifest.outputs {
                    let id = pass.resource(&spec.name);
                    let image =
                        self.image(width as u32, height as u32, format(&spec.format)?, false)?;
                    self.context.name(image.image, || id.clone());
                    self.context.name(image.view, || format!("{id} view"));
                    images.insert(id, image);
                }
                if pass.geometry() {
                    let id = format!("{}.__hardware_depth", pass.id);
                    let image =
                        self.image(width as u32, height as u32, vk::Format::D32_SFLOAT, true)?;
                    self.context.name(image.image, || id.clone());
                    self.context.name(image.view, || format!("{id} view"));
                    images.insert(id, image);
                }
            }
        }
        validate_frame_images(
            &plan,
            &images.keys().map(String::as_str).collect(),
            &readback,
        )?;
        let new_staging = if reused.is_none() && presentation.is_none() {
            let last = readback.last().expect("final color readback");
            Some(self.buffer(last.end() as u64, vk::BufferUsageFlags::TRANSFER_DST)?)
        } else {
            None
        };
        let staging = new_staging.as_ref().or_else(|| {
            reused
                .as_ref()
                .and_then(|resources| resources.staging.as_ref())
        });
        let mut frame_work = FrameWork {
            vertex_write_bytes: vertex_bytes.len(),
            resident_scene: resident.is_some(),
            #[cfg(feature = "bench")]
            meshlets: meshlets.map(MeshletScene::work),
            culling_mode: scene_culling.as_ref().map(culling::Frame::mode),
            atlas_upload_bytes: if atlas_needs_upload {
                atlas.as_ref().unwrap().atlas.rgba.len()
            } else {
                0
            },
            ..Default::default()
        };
        let atlas_upload = atlas_needs_upload
            .then(|| self.stage_atlas(&atlas.as_ref().unwrap().atlas.rgba))
            .transpose()?;
        let uploads = resident
            .as_ref()
            .map_or(&material_uploads, |resident| &resident.uploads);
        let bound_materials = resident
            .as_ref()
            .map(|r| &r.materials)
            .or(materials.as_ref());
        frame_work.material_images = bound_materials.map_or(0, |set| set.image_count());
        frame_work.material_uploaded_images = uploads.image_count();
        frame_work.material_upload_bytes = uploads.bytes;
        let material_upload = uploads.stage(self)?;
        let culling_scope = u32::from(scene_culling.is_some());
        let capture = self.frame_profiling.then(|| {
            TimestampCapture::new(
                &mut self.slots[index].timestamp_pool,
                &self.context,
                self.timestamp_valid_bits,
                self.timestamp_period_ns,
                scene_culling
                    .as_ref()
                    .map(|_| ("Scene culling".into(), false))
                    .into_iter()
                    .chain(
                        plan.passes
                            .iter()
                            .map(|pass| (pass.id.clone(), !pass_enabled(pass, overrides))),
                    )
                    .collect(),
            )
        });
        let timestamps = capture.as_ref().and_then(|capture| capture.as_ref().ok());
        frame_work.cpu_prepare_ms = prepare_started.elapsed().as_secs_f64() * 1000.;
        let recording_started = Instant::now();
        unsafe {
            self.device
                .reset_descriptor_pool(descriptor_pool, vk::DescriptorPoolResetFlags::empty())
                .map_err(VulkanError::api("reset descriptor pool"))?;
        }
        self.slots[index].pools.graphics.begin()?;
        if let Some(timestamps) = timestamps {
            timestamps.reset(command);
        }
        if let Some(resident) = &resident {
            self.gpu_scene
                .record(resident, &self.device, command, &mut frame_work);
        }
        #[cfg(feature = "bench")]
        if let Some(meshlets) = meshlets {
            meshlets.record_upload(&self.device, command);
        }
        if let Some(staging) = &material_upload {
            uploads.record(&self.device, command, staging);
        }
        let split_upload = atlas_upload.is_some() && self.transfer.is_some();
        if let (Some(atlas), Some(upload)) = (&mut atlas, &atlas_upload) {
            let region = vk::BufferImageCopy::default()
                .buffer_offset(upload.offset)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image_extent(vk::Extent3D {
                    width: atlas.atlas.width,
                    height: atlas.atlas.height,
                    depth: 1,
                });
            if let Some(transfer) = &self.transfer {
                crate::transfer::record_upload(
                    &self.device,
                    self.slots[index].pools.transfer.as_ref().unwrap(),
                    command,
                    upload.buffer.buffer,
                    atlas.image.image,
                    region,
                    [transfer.family, self.family],
                )?;
                atlas.image.layout = vk::ImageLayout::GENERAL;
                atlas.image.stage = vk::PipelineStageFlags::FRAGMENT_SHADER;
                atlas.image.access = vk::AccessFlags::SHADER_READ;
            } else {
                transition(
                    &self.device,
                    command,
                    &mut atlas.image,
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::AccessFlags::TRANSFER_WRITE,
                );
                unsafe {
                    self.device.cmd_copy_buffer_to_image(
                        command,
                        upload.buffer.buffer,
                        atlas.image.image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[region],
                    );
                }
            }
        }
        if let Some(atlas) = &mut atlas
            && !split_upload
        {
            transition(
                &self.device,
                command,
                &mut atlas.image,
                vk::ImageLayout::GENERAL,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                vk::AccessFlags::SHADER_READ,
            );
        }
        if let Some(timestamps) = timestamps {
            timestamps.begin(command, 0);
        }
        if let Some(culling) = &mut scene_culling {
            if let Some(timestamps) = timestamps {
                timestamps.begin(command, 1);
            }
            culling.record(
                &self.device,
                command,
                descriptor_pool,
                self.culling_pipelines.as_ref().unwrap(),
                resident.as_ref().unwrap(),
                geometry::camera_constants(camera, width, height),
                [width as u32, height as u32],
                objects as u32,
            )?;
            if culling.mode() == SceneCulling::HiZ {
                frame_work.occluder_draw_calls = 1;
                frame_work.draw_calls += 1;
            }
            if let Some(timestamps) = timestamps {
                timestamps.end(command, 1);
            }
        }
        for (index, pass) in plan.passes.iter().enumerate() {
            if let Some(timestamps) = timestamps {
                timestamps.begin(command, index as u32 + 1 + culling_scope);
            }
            let enabled = pass_enabled(pass, overrides);
            let key = pipeline_key(pass);
            let key = if resident.is_some() && enabled && pass.resident_geometry() {
                if scene_culling.is_some() {
                    culled_pipeline_key(&key)
                } else {
                    resident_pipeline_key(&key)
                }
            } else if atlas_overflow_fallback && enabled && pass.has_bindless_variant() {
                atlas_pipeline_key(&key)
            } else {
                key
            };
            #[cfg(feature = "bench")]
            let key = if meshlet_frame && enabled && pass.geometry() {
                meshlet_pipeline_key(&key)
            } else {
                key
            };
            let pipeline = &self.pipelines[&key];
            let compute = pipeline.compute;
            let stage = if compute {
                vk::PipelineStageFlags::COMPUTE_SHADER
            } else {
                vk::PipelineStageFlags::FRAGMENT_SHADER | vk::PipelineStageFlags::VERTEX_SHADER
            };
            if !enabled {
                let from = &pass
                    .inputs
                    .iter()
                    .find(|(s, _)| s.format != "scene")
                    .unwrap()
                    .1;
                let to = pass.resource(&pass.manifest.outputs[0].name);
                copy_image(
                    &self.device,
                    command,
                    &mut images,
                    from,
                    &to,
                    width as u32,
                    height as u32,
                )?;
            } else {
                for (_, resource) in pass.inputs.iter().filter(|(s, _)| s.format != "scene") {
                    transition(
                        &self.device,
                        command,
                        images.get_mut(resource).unwrap(),
                        vk::ImageLayout::GENERAL,
                        stage,
                        vk::AccessFlags::SHADER_READ,
                    );
                }
                for spec in &pass.manifest.outputs {
                    let image = images.get_mut(&pass.resource(&spec.name)).unwrap();
                    transition(
                        &self.device,
                        command,
                        image,
                        if compute {
                            vk::ImageLayout::GENERAL
                        } else {
                            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL
                        },
                        if compute {
                            vk::PipelineStageFlags::COMPUTE_SHADER
                        } else {
                            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                        },
                        if compute {
                            vk::AccessFlags::SHADER_WRITE
                        } else {
                            vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                        },
                    );
                }
                let material_input = pass.manifest.scene_materials && !pipeline.bindless_materials;
                let descriptor_images: Vec<_> = pass
                    .inputs
                    .iter()
                    .filter(|(s, _)| s.format != "scene")
                    .map(|(_, name)| name.clone())
                    .chain(if material_input {
                        vec!["__material_atlas".into()]
                    } else {
                        vec![]
                    })
                    .chain(if compute {
                        pass.manifest
                            .outputs
                            .iter()
                            .map(|s| pass.resource(&s.name))
                            .collect::<Vec<_>>()
                    } else {
                        vec![]
                    })
                    .collect();
                let set = unsafe {
                    self.device
                        .allocate_descriptor_sets(
                            &vk::DescriptorSetAllocateInfo::default()
                                .descriptor_pool(descriptor_pool)
                                .set_layouts(&[pipeline.set_layout]),
                        )
                        .map_err(VulkanError::api("allocate descriptor set"))?[0]
                };
                let image_infos: Vec<_> = descriptor_images
                    .iter()
                    .map(|name| {
                        vk::DescriptorImageInfo::default()
                            .image_view(if name == "__material_atlas" {
                                atlas.as_ref().unwrap().image.view
                            } else {
                                images[name].view
                            })
                            .image_layout(vk::ImageLayout::GENERAL)
                    })
                    .collect();
                let input_count = pass
                    .inputs
                    .iter()
                    .filter(|(s, _)| s.format != "scene")
                    .count()
                    + usize::from(material_input);
                let mut writes: Vec<_> = image_infos
                    .iter()
                    .enumerate()
                    .map(|(i, info)| {
                        vk::WriteDescriptorSet::default()
                            .dst_set(set)
                            .dst_binding(i as u32)
                            .descriptor_type(if i < input_count {
                                vk::DescriptorType::SAMPLED_IMAGE
                            } else {
                                vk::DescriptorType::STORAGE_IMAGE
                            })
                            .image_info(std::slice::from_ref(info))
                    })
                    .collect();
                let buffer_infos = resident
                    .as_ref()
                    .filter(|_| pipeline.geometry != Geometry::Expanded)
                    .map(|r| {
                        r.buffers.bind().map(|buffer| {
                            vk::DescriptorBufferInfo::default()
                                .buffer(buffer.buffer)
                                .range(buffer.size)
                        })
                    });
                let buffer_infos = buffer_infos.as_ref().map(|infos| infos.as_slice());
                #[cfg(feature = "bench")]
                let meshlet_infos: Option<[_; 5]> = meshlets
                    .filter(|_| pipeline.geometry.meshlets())
                    .map(|meshlets| {
                        std::array::from_fn(|i| {
                            let buffer = &meshlets.buffers[i];
                            vk::DescriptorBufferInfo::default()
                                .buffer(buffer.buffer)
                                .range(buffer.size)
                        })
                    });
                #[cfg(feature = "bench")]
                let buffer_infos =
                    buffer_infos.or(meshlet_infos.as_ref().map(|infos| infos.as_slice()));
                if let Some(infos) = buffer_infos {
                    writes.extend(infos.iter().enumerate().map(|(index, info)| {
                        vk::WriteDescriptorSet::default()
                            .dst_set(set)
                            .dst_binding(index as u32 + 1)
                            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                            .buffer_info(std::slice::from_ref(info))
                    }));
                }
                let visible_info;
                if pipeline.geometry == Geometry::Culled {
                    let culling = scene_culling.as_ref().unwrap();
                    let buffer = culling.visible_buffer();
                    visible_info = vk::DescriptorBufferInfo::default()
                        .buffer(buffer.buffer)
                        .range(buffer.size);
                    writes.push(
                        vk::WriteDescriptorSet::default()
                            .dst_set(set)
                            .dst_binding(5)
                            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                            .buffer_info(std::slice::from_ref(&visible_info)),
                    );
                }
                let mut constants = geometry::camera_constants(camera, width, height);
                for (i, name) in pass.manifest.parameters.iter().enumerate() {
                    let configured = overrides
                        .parameters
                        .get(&pass.id)
                        .and_then(|p| p.get(name))
                        .or_else(|| pass.params.get(name));
                    constants[16 + i] = if name == "mode" {
                        match configured.and_then(|v| v.as_str()) {
                            Some("aces") => 1.,
                            Some("reinhard") => 0.,
                            None => configured.and_then(|v| v.as_f64()).unwrap_or(1.) as f32,
                            Some(other) => {
                                return Err(VulkanError::Overrides(format!(
                                    "Unknown tone mapping mode {other}"
                                )));
                            }
                        }
                    } else {
                        configured
                            .and_then(|v| v.as_f64())
                            .unwrap_or(if name == "shift" { 0. } else { 1. })
                            as f32
                    };
                }
                if pipeline.bindless_materials {
                    constants[31] = f32::from_bits(materials::FIRST_MATERIAL_ROW);
                }
                if pipeline.geometry == Geometry::Culled {
                    constants[29] =
                        f32::from_bits(scene_culling.as_ref().unwrap().visible_offset());
                }
                #[cfg(feature = "bench")]
                if pipeline.geometry.meshlets() {
                    constants[28] = f32::from_bits(meshlets.unwrap().build_report().meshlets);
                }
                let mut push_bytes = Vec::with_capacity(128);
                for value in constants {
                    push_bytes.extend_from_slice(&value.to_ne_bytes());
                }
                let bind_point = if compute {
                    vk::PipelineBindPoint::COMPUTE
                } else {
                    vk::PipelineBindPoint::GRAPHICS
                };
                unsafe {
                    self.device.update_descriptor_sets(&writes, &[]);
                    self.device
                        .cmd_bind_pipeline(command, bind_point, pipeline.pipeline);
                    self.device.cmd_bind_descriptor_sets(
                        command,
                        bind_point,
                        pipeline.layout,
                        0,
                        &[set],
                        &[],
                    );
                    if pipeline.bindless_materials {
                        self.device.cmd_bind_descriptor_sets(
                            command,
                            bind_point,
                            pipeline.layout,
                            1,
                            &[bound_materials.unwrap().set],
                            &[],
                        );
                    }
                    self.device.cmd_push_constants(
                        command,
                        pipeline.layout,
                        pipeline.geometry.push_stages(),
                        0,
                        &push_bytes,
                    );
                    if compute {
                        self.device.cmd_dispatch(
                            command,
                            (width as u32).div_ceil(pass.manifest.workgroup[0]),
                            (height as u32).div_ceil(pass.manifest.workgroup[1]),
                            1,
                        );
                    } else {
                        let attachments: Vec<_> = pass
                            .manifest
                            .outputs
                            .iter()
                            .map(|spec| {
                                vk::RenderingAttachmentInfo::default()
                                    .image_view(images[&pass.resource(&spec.name)].view)
                                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                                    .load_op(vk::AttachmentLoadOp::CLEAR)
                                    .store_op(vk::AttachmentStoreOp::STORE)
                                    .clear_value(vk::ClearValue {
                                        color: vk::ClearColorValue {
                                            float32: if spec.semantic == "hdr_linear"
                                                || spec.semantic == "albedo_linear"
                                            {
                                                [0.02, 0.03, 0.055, 0.]
                                            } else {
                                                [0.; 4]
                                            },
                                        },
                                    })
                            })
                            .collect();
                        let mut rendering = vk::RenderingInfo::default()
                            .render_area(vk::Rect2D {
                                offset: vk::Offset2D { x: 0, y: 0 },
                                extent: vk::Extent2D {
                                    width: width as u32,
                                    height: height as u32,
                                },
                            })
                            .layer_count(1)
                            .color_attachments(&attachments);
                        let depth_attachment;
                        if pass.geometry() {
                            let depth = images
                                .get_mut(&format!("{}.__hardware_depth", pass.id))
                                .unwrap();
                            transition(
                                &self.device,
                                command,
                                depth,
                                vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                                vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS
                                    | vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                                vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ
                                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                            );
                            depth_attachment = vk::RenderingAttachmentInfo::default()
                                .image_view(depth.view)
                                .image_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL)
                                .load_op(vk::AttachmentLoadOp::CLEAR)
                                .store_op(vk::AttachmentStoreOp::STORE)
                                .clear_value(vk::ClearValue {
                                    depth_stencil: vk::ClearDepthStencilValue {
                                        depth: 1.,
                                        stencil: 0,
                                    },
                                });
                            rendering = rendering.depth_attachment(&depth_attachment);
                        }
                        self.device.cmd_begin_rendering(command, &rendering);
                        self.device.cmd_set_viewport(
                            command,
                            0,
                            &[vk::Viewport {
                                x: 0.,
                                y: 0.,
                                width: width as f32,
                                height: height as f32,
                                min_depth: 0.,
                                max_depth: 1.,
                            }],
                        );
                        self.device.cmd_set_scissor(
                            command,
                            0,
                            &[vk::Rect2D {
                                offset: vk::Offset2D { x: 0, y: 0 },
                                extent: vk::Extent2D {
                                    width: width as u32,
                                    height: height as u32,
                                },
                            }],
                        );
                        let draws = match pipeline.geometry {
                            #[cfg(feature = "bench")]
                            Geometry::Meshlets => {
                                self.meshlet_device.as_ref().unwrap().cmd_draw_mesh_tasks(
                                    command,
                                    frame_work.meshlets.unwrap().task_groups,
                                    1,
                                    1,
                                );
                                1
                            }
                            Geometry::Culled => {
                                scene_culling.as_ref().unwrap().draw(
                                    &self.device,
                                    command,
                                    &resident.as_ref().unwrap().buffers,
                                    objects as u32,
                                );
                                1
                            }
                            Geometry::Resident => {
                                let resident = resident.as_ref().unwrap();
                                resident.buffers.draw(
                                    &self.device,
                                    command,
                                    pipeline.layout,
                                    &resident.runs,
                                    self.max_indirect_draws,
                                )
                            }
                            Geometry::Expanded => {
                                if pass.geometry() {
                                    self.device.cmd_bind_vertex_buffers(
                                        command,
                                        0,
                                        &[vertex.as_ref().unwrap().buffer],
                                        &[0],
                                    );
                                    self.device
                                        .cmd_draw(command, (submitted * 3) as u32, 1, 0, 0);
                                } else {
                                    self.device.cmd_draw(command, 3, 1, 0, 0);
                                }
                                1
                            }
                        };
                        frame_work.draw_calls += draws;
                        if pass.geometry() {
                            frame_work.scene_draw_calls += draws;
                        }
                        self.device.cmd_end_rendering(command);
                    }
                }
            }
            if let Some(commands) = operations.get(&index) {
                for operation in commands {
                    match operation {
                        RenderResourceCommand::ClearResource { name, value } => {
                            let image = images.get_mut(name).unwrap();
                            transition(
                                &self.device,
                                command,
                                image,
                                vk::ImageLayout::GENERAL,
                                vk::PipelineStageFlags::TRANSFER,
                                vk::AccessFlags::TRANSFER_WRITE,
                            );
                            unsafe {
                                self.device.cmd_clear_color_image(
                                    command,
                                    image.image,
                                    vk::ImageLayout::GENERAL,
                                    &vk::ClearColorValue { float32: *value },
                                    &[range(image.aspect)],
                                );
                            }
                        }
                        RenderResourceCommand::CopyResource { from, to } => copy_image(
                            &self.device,
                            command,
                            &mut images,
                            from,
                            to,
                            width as u32,
                            height as u32,
                        )?,
                    }
                }
            }
            if let Some(timestamps) = timestamps {
                timestamps.end(command, index as u32 + 1 + culling_scope);
            }
        }
        if let Some(timestamps) = timestamps {
            timestamps.end(command, 0);
        }
        if let Some(shared) = presentation {
            let output = images
                .get_mut(&plan.output)
                .ok_or(VulkanError::Presentation(
                    "GPU presentation output has no allocated image".into(),
                ))?;
            if output.format != vk::Format::R8G8B8A8_UNORM {
                return Err(VulkanError::Presentation(
                    "GPU presentation requires RGBA8_UNORM graph output".into(),
                ));
            }
            transition(
                &self.device,
                command,
                output,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::PipelineStageFlags::TRANSFER,
                vk::AccessFlags::TRANSFER_READ,
            );
            shared.record_copy(command, output.image, self.family);
        }
        for plane in &readback {
            let image = images.get_mut(&plane.resource).unwrap();
            transition(
                &self.device,
                command,
                image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                vk::PipelineStageFlags::TRANSFER,
                vk::AccessFlags::TRANSFER_READ,
            );
            let region = vk::BufferImageCopy::default()
                .buffer_offset(plane.offset as u64)
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: image.aspect,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image_extent(vk::Extent3D {
                    width: width as u32,
                    height: height as u32,
                    depth: 1,
                });
            unsafe {
                self.device.cmd_copy_image_to_buffer(
                    command,
                    image.image,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    staging.unwrap().buffer,
                    &[region],
                );
            }
        }
        unsafe {
            // Make transfer writes available and visible to host reads. Waiting
            // for the queue alone is not the graph's memory dependency contract.
            if presentation.is_none() {
                let staging = staging.unwrap();
                let host_read = vk::BufferMemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::HOST_READ)
                    .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                    .buffer(staging.buffer)
                    .offset(0)
                    .size(staging.size);
                self.device.cmd_pipeline_barrier(
                    command,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::HOST,
                    vk::DependencyFlags::empty(),
                    &[],
                    &[host_read],
                    &[],
                );
            }
            self.device
                .end_command_buffer(command)
                .map_err(VulkanError::api("end command buffer"))?;
        }
        frame_work.cpu_record_ms = recording_started.elapsed().as_secs_f64() * 1000.;
        let timings = capture.map(|capture| PendingTimings {
            capture,
            record_ms: frame_work.cpu_record_ms,
            submit_ms: None,
        });
        let resources = if let Some(mut resources) = reused {
            resources.images = images;
            resources.vertex = vertex;
            resources.culling = scene_culling;
            resources
        } else {
            FrameResources {
                key: resource_key,
                images,
                vertex,
                staging: new_staging,
                culling: scene_culling,
            }
        };
        let ticket = if presentation.is_some() {
            self.next_ticket
                .checked_add(1)
                .ok_or(VulkanError::TicketExhausted)?
        } else {
            0
        };
        let completion = Completion::new(&self.context, self.timeline.as_ref(), serial)?;
        let transfer = self.transfer.as_ref().filter(|_| split_upload);
        let staging_range = atlas_upload
            .as_ref()
            .and_then(|upload| upload.ring_range.clone());
        let transfer_command =
            transfer.map(|_| self.slots[index].pools.transfer.as_ref().unwrap().command);
        // Install ownership BEFORE submission. Any submit/completion failure
        // leaves this entire bundle pinned, including pools and upload buffers.
        self.slots[index].pending = Some(ProducerFlight {
            ticket,
            completion,
            serial,
            work: frame_work,
            shared: presentation.cloned(),
            resources,
            atlas,
            _upload: atlas_upload,
            materials: bound_materials.cloned(),
            _material_upload: material_upload,
            _scene_buffers: resident
                .as_ref()
                .map(|r| (r.buffers.clone(), self.gpu_scene.prior_buffers())),
            #[cfg(feature = "bench")]
            _meshlet_buffers: meshlets.map(|scene| (scene.buffers.clone(), scene.staging.clone())),
            _transfer_timeline: transfer.map(|queue| queue.timeline.clone()),
            _pipelines: self.pipelines.values().cloned().collect(),
            _culling_pipelines: self.culling_pipelines.clone(),
            _context: self.context.clone(),
            size: [width, height],
            stats: RenderStats {
                objects,
                submitted_triangles: submitted,
                ..Default::default()
            },
            timings,
            cancelled: Arc::new(AtomicBool::new(false)),
            failure: None,
        });
        let submit_started = Instant::now();
        let flight = self.slots[index].pending.as_mut().unwrap();
        let result = if let (Some(queue), Some(command)) = (transfer, transfer_command) {
            queue
                .timeline
                .submit(queue.queue, command, serial, None)
                .inspect(|()| {
                    if let Some(trace) = self.trace {
                        eprintln!(
                            "Vulkan transfer submit renderer={trace} serial={serial} slot={index}"
                        );
                    }
                })
        } else {
            Ok(())
        }
        .and_then(|()| {
            flight.completion.submit(
                self.queue,
                command,
                transfer.map(|queue| queue.timeline.as_ref()),
            )
        });
        if let Err(error) = result {
            let error = VulkanError::ProducerUnconfirmed {
                operation: "producer submit",
                source: error,
            };
            flight.failure = Some(error.clone());
            self.producer_error = Some(error.clone());
            return Err(error);
        }
        // New image uploads and all users are on the same graphics queue.
        // An earlier flight keeps its immutable descriptor generation alive.
        frame_work.cpu_queue_submit_ms = submit_started.elapsed().as_secs_f64() * 1000.;
        flight.work = frame_work;
        if let Some(timings) = &mut flight.timings {
            timings.submit_ms = Some(frame_work.cpu_queue_submit_ms);
        }
        #[cfg(feature = "bench")]
        if let SceneInput::Meshlets(scene) = &mut input {
            scene.staging = None;
        }
        if let Some(resident) = resident {
            let SceneInput::Retained(source) = &mut input else {
                unreachable!()
            };
            self.gpu_scene.commit(resident, source);
        } else {
            if let SceneInput::Retained(source) = &mut input {
                // A compatibility draw leaves the resident buffers untouched.
                source.require_reconciliation();
            }
            self.materials = flight.materials.clone();
        }
        self.performance.mesh_upload_bytes +=
            (frame_work.vertex_write_bytes * usize::from(frame_work.resident_scene)
                + frame_work.mesh_index_write_bytes) as u64;
        self.performance.instance_upload_bytes += frame_work.instance_write_bytes as u64;
        self.performance.material_row_upload_bytes += frame_work.material_row_write_bytes as u64;
        self.serial = serial;
        self.slots[index].last_serial = serial;
        if let Some(range) = staging_range {
            self.staging.cursor.commit(serial, range);
        }
        if let Some(trace) = self.trace {
            let pending = self
                .slots
                .iter()
                .filter(|slot| slot.pending.is_some())
                .count();
            eprintln!(
                "Vulkan frame submit renderer={trace} serial={serial} ticket={ticket} slot={index} previous={previous} pending={pending} timeline={}",
                self.timeline.is_some()
            );
        }
        let flight = self.slots[index].pending.as_mut().unwrap();
        self.last_frame_work = Some(frame_work);
        if presentation.is_some() {
            self.next_ticket = ticket;
            self.performance.gpu_submissions += 1;
            return Ok(None);
        }
        // Explicit CPU capture/readback is the only render call that waits.
        if let Some(trace) = self.trace {
            eprintln!(
                "Vulkan frame wait renderer={trace} reason=diagnostic serial={serial} latest={serial}"
            );
        }
        let wait_started = Instant::now();
        if let Err(error) = flight.completion.wait() {
            let error = VulkanError::ProducerUnconfirmed {
                operation: "diagnostic completion",
                source: error,
            };
            flight.failure = Some(error.clone());
            self.producer_error = Some(error.clone());
            return Err(error);
        }
        frame_work.cpu_wait_ms = wait_started.elapsed().as_secs_f64() * 1000.;
        let slot = &mut self.slots[index];
        let flight = slot.pending.take().unwrap();
        if let Some(culling) = &flight.resources.culling {
            frame_work.culling_stats = culling.completed()?;
        }
        self.last_frame_work = Some(frame_work);
        let staging = flight
            .resources
            .staging
            .as_ref()
            .expect("CPU readback allocation");
        let mut data = vec![0u8; staging.size as usize];
        unsafe {
            staging
                .memory
                .as_ref()
                .unwrap()
                .access(staging.size, |mapped| {
                    std::ptr::copy_nonoverlapping(mapped, data.as_mut_ptr(), data.len());
                })?;
        }
        self.performance.cpu_readback_bytes += staging.size;
        let mut stats = RenderStats {
            objects,
            submitted_triangles: submitted,
            ..Default::default()
        };
        let mut triangles = BTreeSet::new();
        let mut objects = BTreeSet::new();
        for plane in &readback {
            let bytes = &data[plane.offset..plane.end()];
            match plane.kind {
                1 => {
                    stats.shaded_pixels = bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .filter(|v| f32::from_ne_bytes(**v) > 0.)
                        .count()
                }
                4 => {
                    stats.shaded_pixels = bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .filter(|v| f32::from_ne_bytes(**v) < 1.)
                        .count()
                }
                2 | 3 => {
                    for pixel in bytes.as_chunks::<16>().0 {
                        let id = f32::from_ne_bytes(pixel[12..16].try_into().unwrap()) as u32;
                        if id > 0 {
                            if plane.kind == 2 {
                                objects.insert(id);
                            } else {
                                triangles.insert(id);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        stats.triangles = triangles.len();
        stats.rasterized_triangles = triangles.len();
        stats.visible_objects = objects.len();
        // Reuse the readback allocation as output instead of copying RGBA twice.
        data.truncate(width * height * 4);
        let frame = Frame {
            width,
            height,
            rgba: data,
            stats,
            gpu: None,
            timings: flight
                .timings
                .as_ref()
                .map(|timings| Box::new(timings.completed())),
        };
        slot.resources = Some(flight.resources);
        slot.atlas = flight.atlas;
        Ok(Some(frame))
    }
}
impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        let bundle = ProducerRetirement {
            slots: std::mem::take(&mut self.slots),
            _pipelines: std::mem::take(&mut self.pipelines),
            _context: self.context.clone(),
        };
        self.presentation_slots.clear();
        if bundle.slots.iter().any(|slot| slot.pending.is_some()) {
            let trace = self.trace;
            let mut pinned = std::mem::ManuallyDrop::new(bundle);
            crate::retirement::retire(move || {
                let flights = pinned.slots.iter().filter_map(|slot| slot.pending.as_ref());
                if let Some(error) = flights.clone().find_map(|flight| flight.failure.clone()) {
                    return Poll::Ready(Err(error));
                }
                let last = flights.max_by_key(|flight| flight.serial).unwrap();
                match last.completion.ready() {
                    Ok(false) => Poll::Pending,
                    Err(source) => Poll::Ready(Err(VulkanError::ProducerUnconfirmed {
                        operation: "producer retirement",
                        source,
                    })),
                    Ok(true) => {
                        if let Some(trace) = trace {
                            eprintln!(
                                "Vulkan retirement complete renderer={trace} serial={}",
                                last.serial
                            );
                        }
                        // Completion of the last graphics signal also covers
                        // its transfer dependency and all earlier submissions.
                        unsafe { std::mem::ManuallyDrop::drop(&mut pinned) };
                        Poll::Ready(Ok(()))
                    }
                }
            });
        }
    }
}
#[cfg(test)]
mod content_player_tests {
    use super::*;
    use crate::{RenderPath, default_graph, precompile_graph_at};
    use module_engine_core::{ObjectsWorld, WorldProvider};
    #[test]
    fn actual_imported_uv_texture_changes_forward_and_deferred_gpu_pixels_and_cache_is_reused() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut content = module_engine_content::ContentLibrary::new(&root).unwrap();
        let mut scene = ObjectsWorld::demo().snapshot();
        scene.items[0].color = [255; 4];
        content
            .enrich_snapshot(
                &mut scene,
                &BTreeMap::from([(1, "assets/models/demo_crystal.glb".into())]),
            )
            .unwrap();
        let mut renderer = VulkanRenderer::new_at(root).unwrap();
        for path in [RenderPath::Forward, RenderPath::Deferred] {
            let graph = default_graph(path);
            let textured = renderer
                .render(
                    &scene,
                    Camera::default(),
                    160,
                    120,
                    &graph,
                    &BTreeMap::new(),
                    &RenderOverrides::default(),
                )
                .unwrap();
            let repeat = renderer
                .render(
                    &scene,
                    Camera::default(),
                    160,
                    120,
                    &graph,
                    &BTreeMap::new(),
                    &RenderOverrides::default(),
                )
                .unwrap();
            assert_eq!(textured.rgba, repeat.rgba);
            assert!(textured.stats.shaded_pixels > 100);
            if renderer.capabilities().resident_scene {
                let mut retained = ResidentScene::default();
                retained
                    .apply(module_engine_core::SceneUpdate::Reset {
                        revision: 0,
                        snapshot: scene.clone(),
                    })
                    .unwrap();
                let resident = renderer
                    .render_scene(
                        SceneInput::Retained(&mut retained),
                        Camera::default(),
                        160,
                        120,
                        &graph,
                        &BTreeMap::new(),
                        &RenderOverrides::default(),
                    )
                    .unwrap();
                assert!(renderer.last_frame_work().unwrap().resident_scene);
                assert_eq!(textured.rgba, resident.rgba, "{path:?}");
                assert_eq!(textured.stats, resident.stats);
            }

            if let Some(transfer) = renderer.transfer.take() {
                renderer.slots[0].atlas = None;
                let graphics_upload = renderer
                    .render(
                        &scene,
                        Camera::default(),
                        160,
                        120,
                        &graph,
                        &BTreeMap::new(),
                        &RenderOverrides::default(),
                    )
                    .unwrap();
                assert_eq!(textured.rgba, graphics_upload.rgba);
                renderer.transfer = Some(transfer);
            }
            let mut plain = scene.clone();
            plain.items[0].material.base_color_texture = None;
            let untextured = renderer
                .render(
                    &plain,
                    Camera::default(),
                    160,
                    120,
                    &graph,
                    &BTreeMap::new(),
                    &RenderOverrides::default(),
                )
                .unwrap();
            assert!(
                textured
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(untextured.rgba.as_chunks::<4>().0.iter())
                    .filter(|(a, b)| a != b)
                    .count()
                    > 100,
                "Material UV/image must affect actual GPU output"
            );
        }
    }
    #[test]
    fn gpu_scene_capacity_fallback_does_not_keep_unsubmitted_meshes() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut renderer = VulkanRenderer::new_at(root).unwrap();
        if !renderer.capabilities().resident_scene {
            eprintln!("Adapter does not support resident geometry");
            return;
        }
        // Two cube meshes fit; a third would overflow either pool buffer.
        renderer.storage_buffer_limit = 400;
        let scene = ObjectsWorld::demo().snapshot();
        let original = scene.items[0].clone();
        let item = |id, offset| {
            let mut item = original.clone();
            item.id = id;
            item.mesh.vertices[0][1] += offset;
            item
        };
        let first = vec![item(1, 0.)];
        let oversized = vec![item(1, 0.1), item(2, 0.2)];
        let next = vec![item(1, 0.), item(2, 0.3)];
        let graph = default_graph(RenderPath::Forward);
        let sources = BTreeMap::new();
        let controls = RenderOverrides::default();
        let mut retained = ResidentScene::default();
        let mut render = |renderer: &mut VulkanRenderer, revision, items| {
            retained
                .apply(module_engine_core::SceneUpdate::Reset {
                    revision,
                    snapshot: SceneSnapshot { items, time: 0. },
                })
                .unwrap();
            renderer
                .render_scene(
                    SceneInput::Retained(&mut retained),
                    Camera::default(),
                    96,
                    64,
                    &graph,
                    &sources,
                    &controls,
                )
                .unwrap()
        };
        render(&mut renderer, 0, first);
        assert!(renderer.last_frame_work().unwrap().resident_scene);
        render(&mut renderer, 1, oversized);
        assert!(!renderer.last_frame_work().unwrap().resident_scene);
        let actual = render(&mut renderer, 2, next.clone());
        let work = renderer.last_frame_work().unwrap();
        assert!(work.resident_scene, "Unsubmitted meshes exhausted the pool");
        assert_eq!(work.vertex_write_bytes, 8 * 20);
        assert_eq!(work.mesh_index_write_bytes, 36 * 4);
        let reference = renderer
            .render(
                &SceneSnapshot {
                    items: next.clone(),
                    time: 0.,
                },
                Camera::default(),
                96,
                64,
                &graph,
                &sources,
                &controls,
            )
            .unwrap();
        assert_eq!(actual.rgba, reference.rgba);

        // A sparse model delta that can overflow the world-point evaluation
        // must use the expanded compatibility stream and preserve its counts.
        let mut extreme = next.clone();
        extreme[0].model[0] = [f32::MAX, f32::MAX, 0., f32::MAX];
        for (revision, items) in [(3, &extreme), (4, &next)] {
            retained
                .apply(module_engine_core::SceneUpdate::Delta {
                    base: revision - 1,
                    revision,
                    time: 0.,
                    changes: &[module_engine_core::SceneChange {
                        id: items[0].id,
                        kind: module_engine_core::SceneChangeKind::Edited {
                            moved: true,
                            material_changed: false,
                            mesh_changed: false,
                        },
                        items: &items[..1],
                    }],
                })
                .unwrap();
            let result = renderer
                .render_scene(
                    SceneInput::Retained(&mut retained),
                    Camera::default(),
                    96,
                    64,
                    &graph,
                    &sources,
                    &controls,
                )
                .unwrap();
            let work = renderer.last_frame_work().unwrap();
            assert_eq!(work.resident_scene, revision == 4);
            let reference = renderer
                .render(
                    &SceneSnapshot {
                        items: items.clone(),
                        time: 0.,
                    },
                    Camera::default(),
                    96,
                    64,
                    &graph,
                    &sources,
                    &controls,
                )
                .unwrap();
            assert_eq!(result.rgba, reference.rgba);
            assert_eq!(result.stats, reference.stats);
            if revision == 4 {
                assert_eq!(work.vertex_write_bytes, 0);
                assert_eq!(work.mesh_index_write_bytes, 0);
                assert_eq!(work.instance_write_bytes, 0);
            }
        }
    }

    #[test]
    fn sealed_custom_root_eol_variants_render_equal_without_dxc_and_content_changes_fail() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let package = parent.join(format!(
            "module-native-sealed-eol-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&package).unwrap();
        let source = format!(
            "// unique fixture {}\n{}",
            package.file_name().unwrap().to_string_lossy(),
            crate::graph::builtin_source("gpu.fxaa").replace("\r\n", "\n")
        );
        let mut graph = default_graph(RenderPath::Forward);
        let fxaa = graph
            .nodes
            .iter_mut()
            .find(|node| node.module == "gpu.fxaa")
            .unwrap();
        fxaa.module = "gpu.custom".into();
        fxaa.params = serde_json::json!({
            "source_id": "eol-fxaa",
            "manifest": crate::graph::builtin_manifest("gpu.fxaa").unwrap()
        });
        let sources = BTreeMap::from([("eol-fxaa".into(), source.clone())]);
        let dxc_initial = crate::compiler::shader_performance().dxc_invocations;
        precompile_graph_at(
            &graph,
            &sources,
            &crate::compiler::resource_root(),
            &package,
        )
        .unwrap();
        assert!(
            crate::compiler::shader_performance().dxc_invocations > dxc_initial,
            "Unique Custom source must exercise real DXC before sealing"
        );
        let dxc_sealed = crate::compiler::shader_performance().dxc_invocations;
        let scene = ObjectsWorld::demo().snapshot();
        let mut baseline = None;
        for source in [
            &source,
            &source.replace('\n', "\r\n"),
            &source.replacen('\n', "\r\n", 1),
        ] {
            let mut renderer = VulkanRenderer::new_at(package.clone()).unwrap();
            renderer.set_detailed_stats(false);
            let frame = renderer
                .render(
                    &scene,
                    Camera::default(),
                    96,
                    72,
                    &graph,
                    &BTreeMap::from([("eol-fxaa".into(), source.to_string())]),
                    &RenderOverrides::default(),
                )
                .unwrap();
            assert_eq!(frame.rgba.len(), 96 * 72 * 4);
            if let Some(baseline) = &baseline {
                assert_eq!(
                    &frame.rgba, baseline,
                    "Root EOL must preserve actual sealed GPU pixels"
                );
            } else {
                baseline = Some(frame.rgba);
            }
        }
        assert!(source.contains("0.0833"));
        let changed = BTreeMap::from([("eol-fxaa".into(), source.replacen("0.0833", "0.1833", 1))]);
        let error = VulkanRenderer::new_at(package.clone())
            .unwrap()
            .prepare(&graph, &changed)
            .unwrap_err();
        assert!(
            error.to_string().contains("Precompiled Player shader")
                && error.to_string().contains("re-export"),
            "{error}"
        );
        let key = crate::compiler::shader_key(&package, &source, "main", "cs_6_0").unwrap();
        std::fs::remove_file(package.join(format!(".spirv/{key:016x}.spv"))).unwrap();
        let error = VulkanRenderer::new_at(package.clone())
            .unwrap()
            .prepare(&graph, &sources)
            .unwrap_err();
        assert!(
            error.to_string().contains("Precompiled Player shader")
                && error.to_string().contains("re-export"),
            "{error}"
        );
        // Native checks run serially; sealed consumers must not compile on PATH.
        assert_eq!(
            crate::compiler::shader_performance().dxc_invocations,
            dxc_sealed
        );
        assert!(!package.join(".work").exists());
        let cleanup = package.canonicalize().unwrap();
        assert_eq!(cleanup, package);
        assert_eq!(cleanup.parent(), Some(parent.as_path()));
        std::fs::remove_dir_all(cleanup).unwrap();
    }

    #[test]
    fn sealed_player_renders_from_exported_spirv_without_dxc_and_missing_artifact_fails() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let package = root.join(format!(
            ".work/precompiled-player-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&package).unwrap();
        let graph = default_graph(RenderPath::Forward);
        let sources = BTreeMap::new();
        let scene = ObjectsWorld::demo().snapshot();
        let count = precompile_graph_at(&graph, &sources, &root, &package).unwrap();
        assert!(count > 3);
        assert!(!package.join("tools/dxc/dxc.exe").exists());
        let sealed_invocations = crate::shader_performance().dxc_invocations;
        let mut renderer = VulkanRenderer::new_at(package.clone()).unwrap();
        renderer.set_detailed_stats(false);
        let frame = renderer
            .render(
                &scene,
                Camera::default(),
                96,
                72,
                &graph,
                &sources,
                &RenderOverrides::default(),
            )
            .unwrap();
        assert_eq!(frame.rgba.len(), 96 * 72 * 4);
        if renderer.capabilities().gpu_frustum_culling {
            let mut retained = ResidentScene::default();
            retained
                .apply(module_engine_core::SceneUpdate::Reset {
                    revision: 0,
                    snapshot: scene.clone(),
                })
                .unwrap();
            let culled = renderer
                .render_scene(
                    SceneInput::Retained(&mut retained),
                    Camera::default(),
                    96,
                    72,
                    &graph,
                    &sources,
                    &RenderOverrides::default(),
                )
                .unwrap();
            assert!(renderer.last_frame_work().unwrap().culling_mode.is_some());
            assert_eq!(culled.rgba, frame.rgba);
        }
        assert_eq!(
            crate::shader_performance().dxc_invocations,
            sealed_invocations
        );
        let indexed = renderer.capabilities().bindless_materials;
        if indexed {
            use crate::capabilities::MATERIAL_IMAGE_COUNT;
            let mut overflow = scene.clone();
            let template = &scene.items[0];
            overflow.items = (0..MATERIAL_IMAGE_COUNT)
                .map(|i| {
                    let mut item = template.clone();
                    item.id = u64::from(i) + 1;
                    item.material.base_color_texture =
                        Some(Arc::new(module_engine_core::TextureImage {
                            width: 1,
                            height: 1,
                            rgba8: vec![i as u8, (i >> 8) as u8, 160, 255],
                        }));
                    item
                })
                .collect();
            let render = |renderer: &mut VulkanRenderer, scene| {
                renderer
                    .render(
                        scene,
                        Camera::default(),
                        96,
                        72,
                        &graph,
                        &sources,
                        &RenderOverrides::default(),
                    )
                    .unwrap()
            };
            let legacy = render(&mut renderer, &overflow);
            assert_ne!(legacy.rgba, frame.rgba);
            let work = renderer.last_frame_work().unwrap();
            assert_eq!(
                work.atlas_upload_bytes,
                (MATERIAL_IMAGE_COUNT as usize + 1) * 4
            );
            assert_eq!(work.material_images, 0);
            assert_eq!(render(&mut renderer, &overflow).rgba, legacy.rgba);
            assert_eq!(renderer.last_frame_work().unwrap().atlas_upload_bytes, 0);
            assert_eq!(render(&mut renderer, &scene).rgba, frame.rgba);
            assert_eq!(renderer.last_frame_work().unwrap().atlas_upload_bytes, 0);
        }
        drop(renderer);
        let plan = build_plan_at(&graph, &sources, &package).unwrap();
        let pass = plan
            .passes
            .iter()
            .find(|p| p.module == "gpu.forward")
            .unwrap();
        let fragment = if indexed {
            bindless_source(pass)
        } else {
            pass.source.clone()
        };
        let key = crate::compiler::shader_key(&package, &fragment, &pass.manifest.entry, "ps_6_0")
            .unwrap();
        std::fs::remove_file(package.join(format!(".spirv/{key:016x}.spv"))).unwrap();
        assert!(
            VulkanRenderer::new_at(package.clone())
                .unwrap()
                .prepare(&graph, &sources)
                .is_err()
        );
        std::fs::remove_dir_all(package).unwrap();
    }
}
fn pipeline_key(pass: &Pass) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    pass.source.hash(&mut h);
    pass.dependency_hash.hash(&mut h);
    serde_json::to_string(&pass.manifest).unwrap().hash(&mut h);
    format!("{:016x}", h.finish())
}
fn resident_pipeline_key(default: &str) -> String {
    format!("{default}:resident")
}

fn culled_pipeline_key(default: &str) -> String {
    format!("{default}:culled")
}

#[cfg(feature = "bench")]
fn meshlet_pipeline_key(default: &str) -> String {
    format!("{default}:meshlets")
}
fn atlas_pipeline_key(default: &str) -> String {
    format!("atlas/{default}")
}
fn format(name: &str) -> Result<vk::Format, VulkanError> {
    match name {
        "rgba32float" => Ok(vk::Format::R32G32B32A32_SFLOAT),
        "r32float" => Ok(vk::Format::R32_SFLOAT),
        "rgba8unorm" => Ok(vk::Format::R8G8B8A8_UNORM),
        _ => Err(VulkanError::Resource(format!("Unknown GPU format {name}"))),
    }
}
fn range(aspect: vk::ImageAspectFlags) -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange {
        aspect_mask: aspect,
        base_mip_level: 0,
        level_count: 1,
        base_array_layer: 0,
        layer_count: 1,
    }
}
fn transition(
    device: &ash::Device,
    command: vk::CommandBuffer,
    image: &mut Image,
    layout: vk::ImageLayout,
    stage: vk::PipelineStageFlags,
    access: vk::AccessFlags,
) {
    let barrier = vk::ImageMemoryBarrier::default()
        .src_access_mask(image.access)
        .dst_access_mask(access)
        .old_layout(image.layout)
        .new_layout(layout)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .image(image.image)
        .subresource_range(range(image.aspect));
    unsafe {
        device.cmd_pipeline_barrier(
            command,
            image.stage,
            stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
    }
    image.layout = layout;
    image.stage = stage;
    image.access = access;
}
fn copy_image(
    device: &ash::Device,
    command: vk::CommandBuffer,
    images: &mut BTreeMap<String, Image>,
    from: &str,
    to: &str,
    width: u32,
    height: u32,
) -> Result<(), VulkanError> {
    if from == to {
        return Err(VulkanError::Resource(
            "Resource copy cannot alias itself".into(),
        ));
    }
    let source = images
        .get_mut(from)
        .ok_or_else(|| VulkanError::Resource(format!("Unknown GPU resource {from}")))?;
    transition(
        device,
        command,
        source,
        vk::ImageLayout::GENERAL,
        vk::PipelineStageFlags::TRANSFER,
        vk::AccessFlags::TRANSFER_READ,
    );
    let image = source.image;
    let format = source.format;
    let target = images
        .get_mut(to)
        .ok_or_else(|| VulkanError::Resource(format!("Unknown GPU resource {to}")))?;
    if target.format != format {
        return Err(VulkanError::Resource(
            "Resource copy requires identical image formats".into(),
        ));
    }
    transition(
        device,
        command,
        target,
        vk::ImageLayout::GENERAL,
        vk::PipelineStageFlags::TRANSFER,
        vk::AccessFlags::TRANSFER_WRITE,
    );
    let layer = vk::ImageSubresourceLayers {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        mip_level: 0,
        base_array_layer: 0,
        layer_count: 1,
    };
    let region = vk::ImageCopy::default()
        .src_subresource(layer)
        .dst_subresource(layer)
        .extent(vk::Extent3D {
            width,
            height,
            depth: 1,
        });
    unsafe {
        device.cmd_copy_image(
            command,
            image,
            vk::ImageLayout::GENERAL,
            target.image,
            vk::ImageLayout::GENERAL,
            &[region],
        );
    }
    Ok(())
}
fn schedule_overrides(
    plan: &Plan,
    overrides: &RenderOverrides,
) -> Result<BTreeMap<usize, Vec<RenderResourceCommand>>, VulkanError> {
    for (id, params) in &overrides.parameters {
        let pass = plan
            .passes
            .iter()
            .find(|p| &p.id == id)
            .ok_or_else(|| VulkanError::Overrides(format!("Unknown render pass {id}")))?;
        for (name, value) in params {
            if !pass.manifest.parameters.contains(name)
                || !value.as_f64().is_some_and(|v| {
                    v.is_finite()
                        && v.abs() <= f32::MAX as f64
                        && (name != "mode" || v == 0. || v == 1.)
                })
            {
                return Err(VulkanError::Overrides(format!(
                    "Pass {id}: unsupported/nonfinite scalar parameter {name}"
                )));
            }
        }
    }
    for id in overrides.enabled.keys() {
        if !plan.passes.iter().any(|pass| &pass.id == id) {
            return Err(VulkanError::Overrides(format!("Unknown render pass {id}")));
        }
    }
    for pass in &plan.passes {
        let id = &pass.id;
        if !pass_enabled(pass, overrides) {
            let input = pass
                .inputs
                .iter()
                .find(|(s, _)| s.format != "scene")
                .map(|(s, _)| s);
            let output = pass.manifest.outputs.first();
            if pass.manifest.outputs.len() != 1
                || !input
                    .zip(output)
                    .is_some_and(|(a, b)| a.format == b.format && a.semantic == b.semantic)
            {
                return Err(VulkanError::Overrides(format!(
                    "Pass {id} cannot be disabled: no type-preserving single-image bypass"
                )));
            }
        }
    }
    let mut producers = BTreeMap::new();
    let mut consumers = BTreeMap::<String, usize>::new();
    for (index, pass) in plan.passes.iter().enumerate() {
        for output in &pass.manifest.outputs {
            producers.insert(pass.resource(&output.name), (index, output));
        }
        for (_, input) in &pass.inputs {
            consumers.entry(input.clone()).or_insert(index);
        }
    }
    let mut schedule = BTreeMap::<usize, Vec<RenderResourceCommand>>::new();
    let mut last = 0;
    for command in &overrides.resource_commands {
        let (needed, destination) = match command {
            RenderResourceCommand::ClearResource { name, value } => {
                if !value.iter().all(|v| v.is_finite()) {
                    return Err(VulkanError::Overrides(
                        "Resource clear requires finite values".into(),
                    ));
                }
                (
                    producers
                        .get(name)
                        .ok_or_else(|| VulkanError::Overrides(format!("Unknown resource {name}")))?
                        .0,
                    name,
                )
            }
            RenderResourceCommand::CopyResource { from, to } => {
                if from == to {
                    return Err(VulkanError::Overrides(
                        "Resource copy must use distinct images".into(),
                    ));
                }
                let a = producers
                    .get(from)
                    .ok_or_else(|| VulkanError::Overrides(format!("Unknown resource {from}")))?;
                let b = producers
                    .get(to)
                    .ok_or_else(|| VulkanError::Overrides(format!("Unknown resource {to}")))?;
                if a.1.format != b.1.format || a.1.semantic != b.1.semantic {
                    return Err(VulkanError::Overrides(
                        "Resource copy requires matching format and semantic".into(),
                    ));
                }
                (a.0.max(b.0), to)
            }
        };
        last = last.max(needed);
        if consumers
            .get(destination)
            .is_some_and(|first| *first <= last)
        {
            return Err(VulkanError::Overrides(
                "Ordered resource operation would happen after its destination was consumed".into(),
            ));
        }
        schedule.entry(last).or_default().push(command.clone());
    }
    Ok(schedule)
}

/// Validate portable render controls using only graph resource/parameter metadata.
/// No shader compilation, world mutation, Vulkan work, or source replacement occurs.
pub fn validate_overrides(graph: &Graph, overrides: &RenderOverrides) -> Result<(), VulkanError> {
    RenderGraphValidator::new(graph)?.validate(overrides)
}
pub fn validate_overrides_at(
    graph: &Graph,
    overrides: &RenderOverrides,
    _resource_root: &std::path::Path,
) -> Result<(), VulkanError> {
    RenderGraphValidator::new(graph)?.validate(overrides)
}

/// Reuse one metadata plan to validate current settings, every advertised
/// preset, and live Rust controls. No sources, includes or GPU are consulted.
pub struct RenderGraphValidator {
    plan: Plan,
}
impl RenderGraphValidator {
    pub fn new(graph: &Graph) -> Result<Self, VulkanError> {
        Ok(Self {
            plan: build_metadata_plan(graph)?,
        })
    }
    pub fn validate(&self, overrides: &RenderOverrides) -> Result<(), VulkanError> {
        schedule_overrides(&self.plan, overrides).map(|_| ())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RenderPath, custom_compute_manifest, default_graph, demo_custom_compute_source};
    use module_engine_core::{ObjectsWorld, WorldProvider, make_edge};
    #[test]
    fn failed_partial_native_allocations_release_owned_handles_and_device_recovers() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let mut renderer = VulkanRenderer::new().unwrap();
        let counts = renderer.allocations.clone();
        let initial = counts.snapshot();
        // Exercise standalone allocations even on a device that offers the allocator.
        let allocator = renderer.allocator.take();
        for allocator in allocator.into_iter().map(Some).chain([None]) {
            renderer.allocator = allocator;
            for fault in [
                AllocationFault::BufferHandle,
                AllocationFault::BufferMemory,
                AllocationFault::ImageHandle,
                AllocationFault::ImageMemory,
                AllocationFault::ImageView,
            ] {
                renderer.allocation_fault = Some(fault);
                let error = match fault {
                    AllocationFault::BufferHandle | AllocationFault::BufferMemory => renderer
                        .buffer(4096, vk::BufferUsageFlags::VERTEX_BUFFER)
                        .err(),
                    _ => renderer
                        .image(32, 32, vk::Format::R8G8B8A8_UNORM, false)
                        .err(),
                }
                .expect("injected allocation point must reject the partial object");
                assert!(
                    error
                        .to_string()
                        .contains("Injected Vulkan allocation failure"),
                    "{error}"
                );
                assert_eq!(
                    counts.snapshot(),
                    initial,
                    "no handle or device-memory allocation may survive {fault:?}"
                );
            }
        }
        renderer.allocation_fault = None;
        let frame = renderer
            .render(
                &ObjectsWorld::demo().snapshot(),
                Camera::default(),
                96,
                64,
                &default_graph(RenderPath::Forward),
                &BTreeMap::new(),
                &RenderOverrides::default(),
            )
            .unwrap();
        assert_eq!(frame.rgba.len(), 96 * 64 * 4);
        assert!(counts.snapshot().into_iter().all(|count| count > 0));
        drop(renderer);
        assert_eq!(
            counts.snapshot(),
            [0; 4],
            "successful retained resources also release exactly once"
        );
    }
    fn rename_pass(graph: &mut Graph, previous: &str, next: &str) {
        graph
            .nodes
            .iter_mut()
            .find(|node| node.id == previous)
            .unwrap()
            .id = next.into();
        for edge in &mut graph.edges {
            if edge.from_node == previous {
                edge.from_node = next.into();
            }
            if edge.to_node == previous {
                edge.to_node = next.into();
            }
        }
    }
    #[test]
    fn detailed_readback_aligns_float4_planes_on_odd_viewports() {
        let plan = build_metadata_plan(&default_graph(RenderPath::Forward)).unwrap();
        let planes = frame_readback(&plan, 7, 5, true);
        assert_eq!(
            planes
                .iter()
                .map(|plane| (plane.offset, plane.size))
                .collect::<Vec<_>>(),
            [(0, 140), (140, 140), (288, 560), (848, 560)],
        );
    }

    #[test]
    fn frame_allocation_identity_includes_resource_ids_readback_and_viewport() {
        let graph = default_graph(RenderPath::Forward);
        let plan = build_metadata_plan(&graph).unwrap();
        let readback = frame_readback(&plan, 96, 64, true);
        let key = frame_resource_key(&plan, 96, 64, &readback);
        let mut renamed = graph.clone();
        rename_pass(&mut renamed, "ao", "gpu_ssao_added");
        let renamed_plan = build_metadata_plan(&renamed).unwrap();
        assert_eq!(
            plan.passes.iter().map(pipeline_key).collect::<Vec<_>>(),
            renamed_plan
                .passes
                .iter()
                .map(pipeline_key)
                .collect::<Vec<_>>(),
            "this was the insufficient old resource cache key"
        );
        let renamed_readback = frame_readback(&renamed_plan, 96, 64, true);
        assert_ne!(
            key,
            frame_resource_key(&renamed_plan, 96, 64, &renamed_readback)
        );
        let available = key
            .images
            .iter()
            .map(|(name, _, _)| name.as_str())
            .collect();
        let error =
            validate_frame_images(&renamed_plan, &available, &renamed_readback).unwrap_err();
        assert!(
            error.to_string().contains("gpu_ssao_added.color")
                && error.to_string().contains("allocated image"),
            "{error}"
        );
        assert_ne!(
            key,
            frame_resource_key(&plan, 96, 64, &frame_readback(&plan, 96, 64, false))
        );
        assert_ne!(
            key,
            frame_resource_key(&plan, 128, 72, &frame_readback(&plan, 128, 72, true))
        );
        let mut same_allocations = plan.clone();
        same_allocations
            .passes
            .iter_mut()
            .find(|pass| pass.id == "tonemap")
            .unwrap()
            .source
            .push_str("\n// shader-only edit\n");
        assert_eq!(
            key,
            frame_resource_key(&same_allocations, 96, 64, &readback),
            "a new shader must not force new image allocations"
        );
    }
    #[test]
    fn gpu_resource_cache_renames_ssao_geometry_and_output_without_stale_images() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let scene = ObjectsWorld::demo().snapshot();
        let camera = Camera::default();
        for path in [RenderPath::Forward, RenderPath::Deferred] {
            let mut renderer = VulkanRenderer::new().unwrap();
            let mut graph = default_graph(path);
            let sources = BTreeMap::new();
            let controls = RenderOverrides::default();
            let original = renderer
                .render(&scene, camera, 96, 64, &graph, &sources, &controls)
                .unwrap();
            let pipeline_count = renderer.performance.pipeline_creations;
            for (previous, next) in [
                ("ao", "gpu_ssao_added"),
                ("geometry", "geometry_added"),
                ("encode", "encode_added"),
            ] {
                rename_pass(&mut graph, previous, next);
                let frame = renderer
                    .render(&scene, camera, 96, 64, &graph, &sources, &controls)
                    .unwrap();
                assert_eq!(
                    frame.rgba, original.rgba,
                    "resource rename must preserve actual GPU pixels"
                );
                assert_eq!(
                    renderer.performance.pipeline_creations, pipeline_count,
                    "resource IDs change while pipeline identities remain reusable"
                );
                let resources = renderer.slots[0].resources.as_ref().unwrap();
                assert!(
                    resources
                        .images
                        .keys()
                        .any(|resource| resource.starts_with(&format!("{next}.")))
                );
                assert!(
                    !resources
                        .images
                        .keys()
                        .any(|resource| resource.starts_with(&format!("{previous}.")))
                );
                assert_eq!(
                    resources.staging.as_ref().unwrap().size,
                    resources.key.readback.last().unwrap().end() as u64
                );
            }
            renderer.set_detailed_stats(false);
            let resized = renderer
                .render(&scene, camera, 128, 72, &graph, &sources, &controls)
                .unwrap();
            assert_eq!(resized.rgba.len(), 128 * 72 * 4);
            assert_eq!(
                renderer.slots[0]
                    .resources
                    .as_ref()
                    .unwrap()
                    .staging
                    .as_ref()
                    .unwrap()
                    .size,
                128 * 72 * 4
            );
        }
    }
    #[test]
    fn metadata_validator_reuses_declared_contracts_without_any_shader_root() {
        let absent = std::env::temp_dir().join("module-engine-no-root-for-render-controls");
        let (graph, _) = custom_graph();
        let validator = RenderGraphValidator::new(&graph).unwrap();
        for _ in 0..120 {
            validator.validate(&RenderOverrides::default()).unwrap();
            validate_overrides_at(&graph, &RenderOverrides::default(), &absent).unwrap();
        }
        let mut controls = RenderOverrides::default();
        controls.enabled.insert("missing".into(), false);
        assert!(
            validator
                .validate(&controls)
                .unwrap_err()
                .to_string()
                .contains("Unknown render pass")
        );
    }
    #[test]
    fn prepared_plan_reuse_skips_layout_rebuilds_but_keeps_real_edits_and_explicit_freshness() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let mut renderer = VulkanRenderer::new().unwrap();
        let mut graph = default_graph(RenderPath::Forward);
        let mut sources = BTreeMap::new();
        renderer.prepare(&graph, &sources).unwrap();
        let initial = renderer.performance();
        for index in 0..120 {
            graph.nodes[0].position[0] = index as f32;
            graph.nodes[0].label = format!("Layout {index}");
            renderer.prepared_inputs.as_mut().unwrap().2 = Instant::now();
            renderer.prepare_for_render(&graph, &sources).unwrap();
        }
        let stable = renderer.performance();
        assert_eq!(stable.plan_builds, initial.plan_builds);
        assert_eq!(stable.plan_cache_hits - initial.plan_cache_hits, 120);
        assert_eq!(stable.pipeline_creations, initial.pipeline_creations);
        graph
            .nodes
            .iter_mut()
            .find(|node| node.id == "tonemap")
            .unwrap()
            .params["exposure"] = serde_json::json!(1.25);
        renderer.prepare_for_render(&graph, &sources).unwrap();
        assert_eq!(renderer.performance().plan_builds, stable.plan_builds + 1);
        sources.insert("new-inline-source".into(), "float gain = 1.;".into());
        renderer.prepare_for_render(&graph, &sources).unwrap();
        assert_eq!(renderer.performance().plan_builds, stable.plan_builds + 2);
        renderer.prepared_inputs.as_mut().unwrap().2 = Instant::now() - INCLUDE_REFRESH_INTERVAL;
        renderer.prepare_for_render(&graph, &sources).unwrap();
        assert_eq!(renderer.performance().plan_builds, stable.plan_builds + 3);
        renderer.prepare(&graph, &sources).unwrap();
        assert_eq!(renderer.performance().plan_builds, stable.plan_builds + 4);
    }
    fn custom_graph() -> (Graph, BTreeMap<String, String>) {
        let mut graph = default_graph(RenderPath::Forward);
        let node = graph.nodes.iter_mut().find(|n| n.id == "ao").unwrap();
        node.module = "gpu.custom".into();
        node.params = serde_json::json!({"source_id":"custom","manifest":custom_compute_manifest(),"gain":0.5});
        graph
            .edges
            .retain(|e| e.to_node != "ao" || e.to_port == "color");
        (
            graph,
            BTreeMap::from([("custom".into(), demo_custom_compute_source())]),
        )
    }
    #[test]
    fn metadata_rejects_cycles_semantics_formats_and_missing_resources() {
        let mut graph = default_graph(RenderPath::Forward);
        graph.edges.retain(|e| e.to_node != "tonemap");
        graph
            .edges
            .push(make_edge("aa", "color", "tonemap", "color"));
        assert!(
            build_plan(&graph, &BTreeMap::new())
                .unwrap_err()
                .to_string()
                .contains("cycle")
        );
        let (mut graph, sources) = custom_graph();
        graph
            .nodes
            .iter_mut()
            .find(|n| n.id == "ao")
            .unwrap()
            .params["manifest"]["outputs"][0]["semantic"] = serde_json::json!("ldr_linear");
        assert!(
            build_plan(&graph, &sources)
                .unwrap_err()
                .to_string()
                .contains("expects")
        );
        let mut overrides = RenderOverrides::default();
        overrides.parameters.insert(
            "unknown".into(),
            BTreeMap::from([("exposure".into(), serde_json::json!(1.))]),
        );
        assert!(validate_overrides(&default_graph(RenderPath::Forward), &overrides).is_err());
        overrides = RenderOverrides::default();
        overrides.enabled.insert("tonemap".into(), false);
        assert!(
            validate_overrides(&default_graph(RenderPath::Forward), &overrides)
                .unwrap_err()
                .to_string()
                .contains("type-preserving")
        );
        overrides = RenderOverrides::default();
        overrides
            .resource_commands
            .push(RenderResourceCommand::CopyResource {
                from: "geometry.normals".into(),
                to: "ao.color".into(),
            });
        assert!(
            validate_overrides(&default_graph(RenderPath::Forward), &overrides)
                .unwrap_err()
                .to_string()
                .contains("semantic")
        );
    }
    #[test]
    fn compiled_hlsl_must_match_manifest_bindings_workgroup_and_storage_format() {
        crate::requires_capabilities!(Dxc);
        let (graph, sources) = custom_graph();
        let plan = build_plan(&graph, &sources).unwrap();
        let pass = plan.passes.iter().find(|p| p.id == "ao").unwrap();
        for (source, expected) in [
            (
                pass.source
                    .replace("numthreads(8,8,1)", "numthreads(4,4,1)"),
                "workgroup",
            ),
            (
                pass.source.replace("binding(1,0)", "binding(9,0)"),
                "outside",
            ),
            (
                pass.source
                    .replace("image_format(\"rgba32f\")", "image_format(\"rgba8\")"),
                "format decoration",
            ),
        ] {
            let shader = compile_hlsl(&source, "main", "cs_6_0").unwrap();
            assert!(
                verify_shader(pass, &shader.words, false)
                    .unwrap_err()
                    .to_string()
                    .contains(expected)
            );
        }
    }
    #[test]
    fn actual_gpu_bypass_and_copy_agree_and_invalid_candidate_retains_prepared_graph() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let mut renderer = VulkanRenderer::new().expect("GPU regression requires Vulkan1.3 driver");
        let scene = ObjectsWorld::demo().snapshot();
        let graph = default_graph(RenderPath::Forward);
        let sources = BTreeMap::new();
        let base = renderer
            .render(
                &scene,
                Camera::default(),
                160,
                120,
                &graph,
                &sources,
                &RenderOverrides::default(),
            )
            .unwrap();
        assert!(base.stats.shaded_pixels > 500);
        assert!(
            base.timings.is_none(),
            "ordinary frames do not collect timestamps"
        );
        renderer.set_frame_profiling(true);
        let mut bypass = RenderOverrides::default();
        bypass.enabled.insert("ao".into(), false);
        let no_ao = renderer
            .render(
                &scene,
                Camera::default(),
                160,
                120,
                &graph,
                &sources,
                &bypass,
            )
            .unwrap();
        let timings = no_ao.timings.as_ref().expect("profiled frame");
        assert!(timings.cpu_record_ms.unwrap() > 0.);
        assert!(timings.cpu_submit_ms.unwrap() > 0.);
        match timings.gpu.as_ref().expect("explicit GPU timing state") {
            module_engine_core::GpuTimings::Available { graph_ms, passes } => {
                assert!(graph_ms.is_finite() && *graph_ms > 0.);
                assert_eq!(
                    passes
                        .iter()
                        .map(|pass| pass.id.as_str())
                        .collect::<Vec<_>>(),
                    renderer
                        .prepared
                        .as_ref()
                        .unwrap()
                        .passes
                        .iter()
                        .map(|pass| pass.id.as_str())
                        .collect::<Vec<_>>()
                );
                assert!(
                    passes
                        .iter()
                        .all(|pass| pass.ms.is_finite() && pass.ms >= 0.)
                );
                assert!(passes.iter().any(|pass| pass.id == "ao" && pass.bypassed));
            }
            module_engine_core::GpuTimings::Unavailable { reason } => {
                assert_eq!(renderer.timestamp_valid_bits, 0, "{reason}");
            }
        }
        assert_ne!(
            base.rgba, no_ao.rgba,
            "SSAO executes on GPU and must affect actual pixels"
        );
        let mut copying = RenderOverrides::default();
        copying
            .resource_commands
            .push(RenderResourceCommand::CopyResource {
                from: "geometry.color".into(),
                to: "ao.color".into(),
            });
        let copied = renderer
            .render(
                &scene,
                Camera::default(),
                160,
                120,
                &graph,
                &sources,
                &copying,
            )
            .unwrap();
        assert_eq!(
            copied.rgba, no_ao.rgba,
            "vkCmdCopyImage must match explicitly bypassing AO"
        );
        if let Some(module_engine_core::GpuTimings::Available { passes, .. }) = copied
            .timings
            .as_ref()
            .and_then(|timings| timings.gpu.as_ref())
        {
            assert!(passes.iter().any(|pass| pass.id == "ao" && !pass.bypassed));
        }
        let (custom_graph, mut custom_sources) = custom_graph();
        renderer.prepare(&custom_graph, &custom_sources).unwrap();
        let previous = renderer.prepared.clone().unwrap();
        custom_sources.insert("custom".into(), "not HLSL".into());
        assert!(renderer.prepare(&custom_graph, &custom_sources).is_err());
        assert_eq!(renderer.prepared.as_ref().unwrap().output, previous.output);
        assert_eq!(
            renderer
                .prepared
                .as_ref()
                .unwrap()
                .passes
                .last()
                .unwrap()
                .id,
            previous.passes.last().unwrap().id
        );
    }
}

#[cfg(test)]
mod fullscreen_tests {
    use super::*;
    use crate::{
        RenderPath, custom_compute_manifest, custom_fullscreen_manifest, default_graph,
        demo_custom_compute_source, demo_custom_fullscreen_source,
    };
    use module_engine_core::{ObjectsWorld, WorldProvider};
    #[test]
    fn independent_fullscreen_raster_and_compute_execute_equivalent_shader_math() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let mut renderer = VulkanRenderer::new().unwrap();
        let scene = ObjectsWorld::demo().snapshot();
        let mut graph = default_graph(RenderPath::Forward);
        let node = graph.nodes.iter_mut().find(|n| n.id == "ao").unwrap();
        node.module = "gpu.custom".into();
        node.params = serde_json::json!({"source_id":"custom","manifest":custom_compute_manifest(),"gain":0.4});
        graph
            .edges
            .retain(|e| e.to_node != "ao" || e.to_port == "color");
        let mut sources = BTreeMap::from([("custom".into(), demo_custom_compute_source())]);
        let compute = renderer
            .render(
                &scene,
                Camera::default(),
                128,
                96,
                &graph,
                &sources,
                &RenderOverrides::default(),
            )
            .unwrap();
        graph
            .nodes
            .iter_mut()
            .find(|n| n.id == "ao")
            .unwrap()
            .params["manifest"] = serde_json::to_value(custom_fullscreen_manifest()).unwrap();
        sources.insert("custom".into(), demo_custom_fullscreen_source());
        let raster = renderer
            .render(
                &scene,
                Camera::default(),
                128,
                96,
                &graph,
                &sources,
                &RenderOverrides::default(),
            )
            .unwrap();
        assert_eq!(compute.rgba, raster.rgba);
    }
}
#[cfg(test)]
mod conversion_tests {
    use super::*;
    use crate::{RenderPath, builtin_hlsl_source, builtin_manifest, default_graph};
    use module_engine_core::{ObjectsWorld, WorldProvider};
    #[test]
    fn builtins_including_fxaa_and_color_encode_become_editable_custom_without_pixel_changes() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let mut renderer = VulkanRenderer::new().unwrap();
        let mut scene = ObjectsWorld::demo().snapshot();
        let item = &mut scene.items[0];
        item.mesh.texcoords = vec![[0.38, 0.63]; item.mesh.vertices.len()];
        item.material.base_color_texture = Some(Arc::new(module_engine_core::TextureImage {
            width: 3,
            height: 2,
            rgba8: vec![
                232, 163, 38, 255, 16, 18, 193, 255, 49, 224, 58, 255, 186, 24, 139, 255, 22, 175,
                98, 255, 243, 201, 143, 255,
            ],
        }));
        for path in [RenderPath::Forward, RenderPath::Deferred] {
            let mut graph = default_graph(path);
            let mut sources = BTreeMap::new();
            let before = renderer
                .render(
                    &scene,
                    Camera::default(),
                    128,
                    96,
                    &graph,
                    &sources,
                    &RenderOverrides::default(),
                )
                .unwrap();
            for node in graph
                .nodes
                .iter_mut()
                .filter(|n| n.module.starts_with("gpu."))
            {
                let mut manifest = builtin_manifest(&node.module).unwrap();
                if manifest.stage == "raster" {
                    manifest.vertex_entry = Some("mainVS".into());
                }
                let key = format!("editable_{}", node.id);
                sources.insert(
                    key.clone(),
                    builtin_hlsl_source(&node.module).unwrap().into(),
                );
                node.module = "gpu.custom".into();
                node.params["source_id"] = serde_json::json!(key);
                node.params["manifest"] = serde_json::to_value(manifest).unwrap();
            }
            let after = renderer
                .render(
                    &scene,
                    Camera::default(),
                    128,
                    96,
                    &graph,
                    &sources,
                    &RenderOverrides::default(),
                )
                .unwrap();
            assert_eq!(
                before.rgba, after.rgba,
                "Converting shaders to editable HLSL must preserve the complete pass graph behavior"
            );
        }
    }
}

fn pass_enabled(pass: &Pass, overrides: &RenderOverrides) -> bool {
    overrides
        .enabled
        .get(&pass.id)
        .copied()
        .or_else(|| pass.params.get("enabled").and_then(|v| v.as_bool()))
        .unwrap_or(true)
}
#[cfg(test)]
mod authored_configuration_tests {
    use super::*;
    use crate::{RenderPath, custom_compute_manifest, default_graph};
    use module_engine_core::{ObjectsWorld, WorldProvider};
    #[test]
    fn authored_config_typos_are_rejected_by_metadata_validation() {
        let mut graph = default_graph(RenderPath::Forward);
        graph
            .nodes
            .iter_mut()
            .find(|n| n.id == "tonemap")
            .unwrap()
            .params["exposue"] = serde_json::json!(2.0);
        assert!(
            validate_overrides(&graph, &RenderOverrides::default())
                .unwrap_err()
                .to_string()
                .contains("unknown parameter exposue")
        );
        let mut value = serde_json::to_value(custom_compute_manifest()).unwrap();
        value["entri"] = serde_json::json!("main");
        assert!(
            serde_json::from_value::<crate::PassManifest>(value)
                .unwrap_err()
                .to_string()
                .contains("unknown field")
        );
        let mut value = serde_json::to_value(custom_compute_manifest()).unwrap();
        value["inputs"][0]["samplez"] = serde_json::json!(4);
        assert!(
            serde_json::from_value::<crate::PassManifest>(value)
                .unwrap_err()
                .to_string()
                .contains("unknown field")
        );
        graph = default_graph(RenderPath::Forward);
        graph
            .nodes
            .iter_mut()
            .find(|n| n.id == "tonemap")
            .unwrap()
            .params["enabled"] = serde_json::json!(false);
        assert!(
            validate_overrides(&graph, &RenderOverrides::default())
                .unwrap_err()
                .to_string()
                .contains("cannot be disabled")
        );
    }

    #[test]
    fn authored_disable_has_explicit_precedence_in_rendered_frames() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let mut graph = default_graph(RenderPath::Forward);
        graph
            .nodes
            .iter_mut()
            .find(|n| n.id == "ao")
            .unwrap()
            .params["enabled"] = serde_json::json!(false);
        validate_overrides(&graph, &RenderOverrides::default()).unwrap();
        let mut renderer = VulkanRenderer::new().unwrap();
        let scene = ObjectsWorld::demo().snapshot();
        let sources = BTreeMap::new();
        let authored = renderer
            .render(
                &scene,
                Camera::default(),
                128,
                96,
                &graph,
                &sources,
                &RenderOverrides::default(),
            )
            .unwrap();
        let mut explicit = RenderOverrides::default();
        explicit.enabled.insert("ao".into(), false);
        let baseline_graph = default_graph(RenderPath::Forward);
        let controlled = renderer
            .render(
                &scene,
                Camera::default(),
                128,
                96,
                &baseline_graph,
                &sources,
                &explicit,
            )
            .unwrap();
        assert_eq!(
            authored.rgba, controlled.rgba,
            "Authored enabled:false must execute the same native bypass as a portable command"
        );
        explicit.enabled.insert("ao".into(), true);
        let restored = renderer
            .render(
                &scene,
                Camera::default(),
                128,
                96,
                &graph,
                &sources,
                &explicit,
            )
            .unwrap();
        let baseline = renderer
            .render(
                &scene,
                Camera::default(),
                128,
                96,
                &baseline_graph,
                &sources,
                &RenderOverrides::default(),
            )
            .unwrap();
        assert_eq!(
            restored.rgba, baseline.rgba,
            "Explicit command override takes precedence over authored disabled state"
        );
        assert_ne!(baseline.rgba, authored.rgba);
    }
}

#[cfg(test)]
mod include_regression {
    use super::*;
    use crate::{
        RenderPath, compile_hlsl, custom_compute_manifest, default_graph,
        demo_custom_compute_source,
    };
    use module_engine_core::{ObjectsWorld, WorldProvider};
    #[test]
    fn actual_nested_include_mutation_recompiles_unchanged_inline_shader_and_gpu_pipeline() {
        crate::requires_capabilities!(Vulkan, Dxc);
        let mut renderer =
            VulkanRenderer::new().expect("Real Vulkan adapter required for GPU regression");
        let name = format!("include-regression-{}", std::process::id());
        let workspace = crate::compiler::resource_root();
        let directory = workspace.join(".work").join(&name);
        std::fs::create_dir_all(directory.join("child")).unwrap();
        std::fs::write(
            directory.join("main.hlsli"),
            "#ifndef TEST_MAIN\n#define TEST_MAIN\n#include \"child/gain.hlsli\"\n#endif\n",
        )
        .unwrap();
        let child = directory.join("child/gain.hlsli");
        let child_source = "#ifndef TEST_GAIN\n#define TEST_GAIN\n#include \"../main.hlsli\"\nfloat header_gain() {return 0.2;}\n#endif\n";
        std::fs::write(&child, child_source).unwrap();
        let source = format!(
            "#include \".work/{name}/main.hlsli\"\n{}",
            demo_custom_compute_source()
                .replace("c.rgb*values[0].x", "c.rgb*values[0].x*header_gain()")
        );
        let mut graph = default_graph(RenderPath::Forward);
        let node = graph.nodes.iter_mut().find(|n| n.id == "ao").unwrap();
        node.module = "gpu.custom".into();
        node.params = serde_json::json!({"source_id":"nested","manifest":custom_compute_manifest(),"gain":1.0});
        graph
            .edges
            .retain(|e| e.to_node != "ao" || e.to_port == "color");
        let sources = BTreeMap::from([("nested".into(), source.clone())]);
        let scene = ObjectsWorld::demo().snapshot();
        let first_spirv = compile_hlsl(&source, "main", "cs_6_0").unwrap().words;
        let first = renderer
            .render(
                &scene,
                Camera::default(),
                160,
                120,
                &graph,
                &sources,
                &RenderOverrides::default(),
            )
            .unwrap();
        let pipeline_count = renderer.pipelines.len();
        let key = pipeline_key(
            renderer
                .prepared
                .as_ref()
                .unwrap()
                .passes
                .iter()
                .find(|p| p.id == "ao")
                .unwrap(),
        );
        renderer.prepare(&graph, &sources).unwrap();
        assert_eq!(
            renderer.pipelines.len(),
            pipeline_count,
            "Unchanged dependencies must reuse GPU pipelines"
        );
        std::fs::write(&child, child_source.replace("return 0.2", "return 0.8")).unwrap();
        let second_spirv = compile_hlsl(&source, "main", "cs_6_0").unwrap().words;
        assert_ne!(
            first_spirv, second_spirv,
            "DXC must consume changed nested include with identical inline text"
        );
        renderer.prepare(&graph, &sources).unwrap();
        let changed_key = pipeline_key(
            renderer
                .prepared
                .as_ref()
                .unwrap()
                .passes
                .iter()
                .find(|p| p.id == "ao")
                .unwrap(),
        );
        assert_ne!(key, changed_key);
        assert_eq!(
            renderer.pipelines.len(),
            pipeline_count + 1,
            "Changed include needs a new compiled pipeline"
        );
        let second = renderer
            .render(
                &scene,
                Camera::default(),
                160,
                120,
                &graph,
                &sources,
                &RenderOverrides::default(),
            )
            .unwrap();
        let pixels = first
            .rgba
            .as_chunks::<4>()
            .0
            .iter()
            .zip(second.rgba.as_chunks::<4>().0.iter())
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            pixels > 100,
            "Changed nested include must change actual GPU pixels"
        );
        assert_eq!(
            sources["nested"], source,
            "Inline authored source remains byte-identical"
        );
        std::fs::write(&child, "this is invalid included HLSL\n").unwrap();
        let error = renderer.prepare(&graph, &sources).unwrap_err();
        let VulkanError::Pass { pass, source } = &error else {
            panic!("expected pass preparation context: {error}");
        };
        assert_eq!(pass, "ao");
        assert!(matches!(
            source.as_ref(),
            VulkanError::Shader(crate::ShaderError::Compile { status, .. }) if !status.success()
        ));
        assert!(error.to_string().contains("HLSL compilation failed"));
        let rejected = renderer.performance();
        for _ in 0..120 {
            renderer.rejected_inputs.as_mut().unwrap().checked = Instant::now();
            assert!(
                renderer
                    .render(
                        &scene,
                        Camera::default(),
                        160,
                        120,
                        &graph,
                        &sources,
                        &RenderOverrides::default()
                    )
                    .unwrap_err()
                    .to_string()
                    .contains("HLSL compilation failed")
            );
        }
        assert_eq!(
            renderer.performance().prepare_calls,
            rejected.prepare_calls,
            "failing hot reload must not compile on every UI frame"
        );
        assert_eq!(
            renderer.performance().pipeline_build_attempts,
            rejected.pipeline_build_attempts
        );
        renderer.rejected_inputs.as_mut().unwrap().checked =
            Instant::now() - INCLUDE_REFRESH_INTERVAL;
        assert!(
            renderer
                .render(
                    &scene,
                    Camera::default(),
                    160,
                    120,
                    &graph,
                    &sources,
                    &RenderOverrides::default()
                )
                .is_err()
        );
        assert_eq!(
            renderer.performance().pipeline_build_attempts,
            rejected.pipeline_build_attempts,
            "unchanged failed include bytes must not invoke DXC again after polling"
        );
        assert_eq!(
            pipeline_key(
                renderer
                    .prepared
                    .as_ref()
                    .unwrap()
                    .passes
                    .iter()
                    .find(|pass| pass.id == "ao")
                    .unwrap()
            ),
            changed_key,
            "last good GPU plan survives every rejected reload"
        );
        std::fs::write(&child, child_source.replace("return 0.2", "return 0.8")).unwrap();
        renderer.rejected_inputs.as_mut().unwrap().checked =
            Instant::now() - INCLUDE_REFRESH_INTERVAL;
        assert_eq!(
            renderer
                .render(
                    &scene,
                    Camera::default(),
                    160,
                    120,
                    &graph,
                    &sources,
                    &RenderOverrides::default()
                )
                .unwrap()
                .rgba,
            second.rgba,
            "Failed include candidate must retain valid pipeline behavior"
        );
        assert!(renderer.rejected_inputs.is_none());
        assert!(renderer.diagnostics().is_empty());
        std::fs::write(&child, child_source.replace("return 0.2", "return 0.6")).unwrap();
        renderer.prepared_inputs.as_mut().unwrap().2 = Instant::now() - INCLUDE_REFRESH_INTERVAL;
        let third = renderer
            .render(
                &scene,
                Camera::default(),
                160,
                120,
                &graph,
                &sources,
                &RenderOverrides::default(),
            )
            .unwrap();
        assert_ne!(
            third.rgba, second.rgba,
            "disk-only repair/change must reach real pixels without an inline edit"
        );
        assert!(renderer.performance().pipeline_build_attempts > rejected.pipeline_build_attempts);
    }
}
