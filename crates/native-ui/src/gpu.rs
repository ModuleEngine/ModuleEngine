use super::{PainterKind, VkResultExt, painter::Painter};
use ash::vk;
use egui::{TexturesDelta, epaint::ClippedPrimitive};
use module_engine_vulkan::{DeviceCapabilities, EditorDevice, EditorDeviceHandles};
use serde::Serialize;
use std::{ffi::CStr, path::Path, sync::Arc, time::Instant};
use winit::{
    raw_window_handle::{HasDisplayHandle, HasWindowHandle},
    window::Window,
};

pub type PresentedFrame = (Instant, Option<Vec<u8>>);

/// Successful submission to the window, with CPU time through presentation.
#[derive(Clone, Copy, Debug)]
pub struct WindowPresentation {
    pub time: Instant,
    pub pixels: [u32; 2],
    pub present_mode: &'static str,
    pub paint_elapsed: std::time::Duration,
    /// A software presentation worker accepts the request asynchronously.
    pub present_elapsed: Option<std::time::Duration>,
}

pub(crate) struct WindowPaint {
    pub presentation: WindowPresentation,
    pub pixels: Option<Vec<u8>>,
}

fn present_mode_name(mode: vk::PresentModeKHR) -> &'static str {
    match mode {
        vk::PresentModeKHR::IMMEDIATE => "Immediate",
        vk::PresentModeKHR::MAILBOX => "Mailbox",
        _ => "Fifo",
    }
}

pub struct Gpu {
    // Drop swapchains first, then the device-owned root surface and its native window.
    painter: Arc<std::sync::Mutex<Painter>>,
    root: WindowFrames,
    children: std::collections::HashMap<winit::window::WindowId, WindowFrames>,
    owner: EditorDevice,
    _root_window: Arc<Window>,

    pub info: DeviceInfo,
}

struct WindowFrames {
    frames: Frames,
    readback: Option<HostBuffer>,
    surface: Option<module_engine_vulkan::WindowSurface>,
    window: Arc<Window>,
    benchmark: bool,
    recreate: bool,
    software_present: Option<SoftwarePresent>,
}

#[derive(Clone, Serialize)]
pub struct DeviceInfo {
    pub adapter: String,
    vendor_id: u32,
    device_id: u32,
    device_uuid: String,
    driver: String,
    driver_version: u32,
    api_version: u32,
    capabilities: DeviceCapabilities,
    producer_slots: usize,
    swapchain_format: String,
    swapchain_images: usize,
    pub present_mode: &'static str,
    ui_frames_in_flight: usize,
}

impl Gpu {
    pub fn new(
        window: Arc<Window>,
        root: &Path,
        kind: PainterKind,
        benchmark: bool,
        meshlets: bool,
    ) -> Result<Self, String> {
        let handle = window.window_handle().map_err(|e| e.to_string())?;
        let display = window.display_handle().map_err(|e| e.to_string())?;
        // SAFETY: The application drops this GPU owner before its window, even on failure.
        let owner = unsafe {
            EditorDevice::for_window(root.into(), display.as_raw(), handle.as_raw(), meshlets)
        }
        .map_err(|e| e.to_string())?;
        let handles = owner.handles();
        if benchmark && (!handles.capabilities.tier_a || handles.producer_slots != 2) {
            return Err("R3.1 requires the tier A R2 device with two producer slots".into());
        }
        let size = window.inner_size();
        let frames = Frames::new(
            &handles,
            vk::Extent2D {
                width: size.width,
                height: size.height,
            },
            benchmark,
            vk::SwapchainKHR::null(),
        )?;
        let painter = Painter::new(&handles, &frames, root, kind)?;
        let mut ids = vk::PhysicalDeviceIDProperties::default();
        let mut driver = vk::PhysicalDeviceDriverProperties::default();
        let mut properties = vk::PhysicalDeviceProperties2::default()
            .push_next(&mut ids)
            .push_next(&mut driver);
        // The physical device and linked query structures are valid for the query.
        unsafe {
            handles
                .instance
                .get_physical_device_properties2(handles.physical, &mut properties)
        };
        let properties = properties.properties;
        // These UTF-8-convertible driver strings are NUL-terminated by Vulkan.
        let (adapter, driver_name) = unsafe {
            (
                CStr::from_ptr(properties.device_name.as_ptr()).to_string_lossy(),
                CStr::from_ptr(driver.driver_info.as_ptr()).to_string_lossy(),
            )
        };
        let info = DeviceInfo {
            adapter: adapter.into_owned(),
            vendor_id: properties.vendor_id,
            device_id: properties.device_id,
            device_uuid: ids
                .device_uuid
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            driver: driver_name.into_owned(),
            driver_version: properties.driver_version,
            api_version: properties.api_version,
            capabilities: *handles.capabilities,
            producer_slots: handles.producer_slots,
            swapchain_format: format!("{:?}", frames.format),
            swapchain_images: frames.images.len(),
            present_mode: if benchmark {
                "IMMEDIATE"
            } else {
                present_mode_name(frames.mode)
            },
            ui_frames_in_flight: 1,
        };
        let software_present =
            if !benchmark && properties.device_type == vk::PhysicalDeviceType::CPU {
                Some(SoftwarePresent::new(
                    &frames,
                    handles.queue_access.clone(),
                    window.clone(),
                )?)
            } else {
                None
            };
        Ok(Self {
            painter: Arc::new(std::sync::Mutex::new(painter)),
            root: WindowFrames {
                frames,
                readback: None,
                surface: None,
                window: window.clone(),
                benchmark,
                recreate: false,
                software_present,
            },
            children: Default::default(),
            owner,
            _root_window: window,
            info,
        })
    }

    pub fn bridge(&self) -> crate::FrameBridge {
        crate::FrameBridge {
            painter: Arc::downgrade(&self.painter),
            target: Arc::new(self.owner.target()),
            adapter: self.info.adapter.clone(),
            present_mode: self.info.present_mode,
        }
    }

    pub fn paint(
        &mut self,
        primitives: &[ClippedPrimitive],
        delta: &mut TexturesDelta,
        scale: f32,
        capture: bool,
    ) -> Result<Option<PresentedFrame>, String> {
        self.paint_observed(primitives, delta, scale, capture)
            .map(|draw| draw.map(|draw| (draw.presentation.time, draw.pixels)))
    }

    pub(crate) fn paint_observed(
        &mut self,
        primitives: &[ClippedPrimitive],
        delta: &mut TexturesDelta,
        scale: f32,
        capture: bool,
    ) -> Result<Option<WindowPaint>, String> {
        self.root.paint(
            &self.owner.handles(),
            &mut *self
                .painter
                .lock()
                .map_err(|_| "Native painter lock poisoned")?,
            primitives,
            delta,
            scale,
            capture,
        )
    }

    pub(crate) fn attach(&mut self, window: Arc<Window>) -> Result<(), String> {
        let display = window.display_handle().map_err(|e| e.to_string())?;
        let handle = window.window_handle().map_err(|e| e.to_string())?;
        // SAFETY: All winit windows use the same event loop/display. WindowFrames
        // owns the window and drops the swapchain before the surface and window.
        let surface = unsafe { self.owner.attach_window(display.as_raw(), handle.as_raw()) }
            .map_err(|e| e.to_string())?;
        let mut handles = self.owner.handles();
        handles.surface = surface.handle();
        let size = window.inner_size();
        let frames = Frames::new(
            &handles,
            vk::Extent2D {
                width: size.width,
                height: size.height,
            },
            false,
            vk::SwapchainKHR::null(),
        )?;
        if frames.format != self.root.frames.format {
            return Err("Detached window requires a different painter attachment format".into());
        }
        self.children.insert(
            window.id(),
            WindowFrames {
                frames,
                readback: None,
                surface: Some(surface),
                window,
                benchmark: false,
                recreate: false,
                software_present: None,
            },
        );
        Ok(())
    }

    pub(crate) fn detach(&mut self, id: winit::window::WindowId) -> Result<(), String> {
        self.finish()?;
        self.children.remove(&id);
        Ok(())
    }

    pub(crate) fn paint_child(
        &mut self,
        id: winit::window::WindowId,
        primitives: &[ClippedPrimitive],
        delta: &mut TexturesDelta,
        scale: f32,
        capture: bool,
    ) -> Result<Option<Vec<u8>>, String> {
        // Root and child share the painter's vertex buffers, descriptors and atlas.
        self.root.frames.wait()?;
        let child = self
            .children
            .get_mut(&id)
            .ok_or("Missing native viewport")?;
        let mut handles = self.owner.handles();
        handles.surface = child
            .surface
            .as_ref()
            .ok_or("Missing native surface")?
            .handle();
        let draw = child.paint(
            &handles,
            &mut *self
                .painter
                .lock()
                .map_err(|_| "Native painter lock poisoned")?,
            primitives,
            delta,
            scale,
            capture,
        )?;
        child.frames.wait()?;
        Ok(draw.and_then(|draw| draw.pixels))
    }

    pub(crate) fn software_present_busy(&self) -> bool {
        self.children.is_empty()
            && self
                .root
                .software_present
                .as_ref()
                .is_some_and(SoftwarePresent::busy)
    }

    pub fn extent(&self) -> [u32; 2] {
        [
            self.root.frames.extent.width,
            self.root.frames.extent.height,
        ]
    }

    pub fn finish(&mut self) -> Result<(), String> {
        let presentation = self.root.complete_present();
        let handles = self.owner.handles();
        let _queue = handles
            .queue_access
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        // SAFETY: All submissions synchronize through the same device lock.
        unsafe { self.owner.handles().device.device_wait_idle() }.at("retire native UI device")?;
        presentation
    }
}

impl WindowFrames {
    fn complete_present(&mut self) -> Result<(), String> {
        if let Some(worker) = self.software_present.as_mut() {
            let suboptimal = worker.complete().inspect_err(|_| self.recreate = true)?;
            self.recreate |= suboptimal;
        }
        Ok(())
    }

    fn paint(
        &mut self,
        handles: &EditorDeviceHandles<'_>,
        painter: &mut Painter,
        primitives: &[ClippedPrimitive],
        delta: &mut TexturesDelta,
        pixels_per_point: f32,
        capture: bool,
    ) -> Result<Option<WindowPaint>, String> {
        let started = Instant::now();
        if !capture
            && delta.is_empty()
            && self
                .software_present
                .as_ref()
                .is_some_and(SoftwarePresent::busy)
        {
            return Ok(None);
        }
        self.complete_present()?;
        self.frames.wait()?;
        let _queue = handles
            .queue_access
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        painter.retire(primitives)?;
        painter.update_textures(&self.frames, delta)?;
        drop(_queue);
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(None);
        }
        let extent = vk::Extent2D {
            width: size.width,
            height: size.height,
        };
        if self.recreate
            || self.frames.state == FrameState::Acquired
            || self.frames.extent != extent
        {
            if self.benchmark {
                return Err("R3.1 presentation extent or surface changed".into());
            }
            let _queue = handles
                .queue_access
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            // SAFETY: No queue work may reference the swapchain during recreation.
            unsafe { handles.device.device_wait_idle() }.at("retire resized surface")?;
            // An abandoned acquisition owns an image and an unconsumed binary
            // semaphore. Retire that entire slot instead of acquiring with it again.
            self.readback = None;
            self.frames = Frames::new(handles, extent, false, self.frames.swapchain)?;
            self.recreate = false;
        }
        if capture {
            if !self.frames.can_capture {
                return Err("Window surface does not support transfer-source capture".into());
            }
            self.readback = Some(self.frames.host_buffer(
                pixel_bytes(self.frames.extent)?,
                vk::BufferUsageFlags::TRANSFER_DST,
            )?);
        }
        let presented = self.frames.draw(
            handles.queue_access,
            painter,
            primitives,
            pixels_per_point,
            self.readback.as_ref(),
        )?;
        let Some((index, suboptimal)) = presented else {
            self.recreate = true;
            self.frames.wait()?;
            self.readback = None;
            return Ok(None);
        };
        // Submission has signalled (or will signal) the per-image semaphore.
        // If presentation fails, retire the slot before attempting another draw.
        self.recreate = true;
        let presenting = Instant::now();
        let synchronous = if !capture && let Some(worker) = self.software_present.as_mut() {
            self.window.pre_present_notify();
            worker.submit(PresentRequest {
                swapchain: self.frames.swapchain,
                rendered: self.frames.images[index as usize].rendered,
                index,
            })?;
            self.recreate = suboptimal;
            false
        } else {
            match self
                .frames
                .present(handles.queue_access, &self.window, index)?
            {
                Some(present_suboptimal) => self.recreate = suboptimal || present_suboptimal,
                None => {
                    self.recreate = true;
                    self.frames.wait()?;
                    self.readback = None;
                    return Ok(None);
                }
            }
            true
        };
        let time = Instant::now();
        if self.recreate && self.benchmark {
            return Err("R3.1 presentation became suboptimal".into());
        }
        let pixels = if self.readback.is_some() {
            self.frames.wait()?;
            let buffer = self.readback.take().expect("capture buffer");
            let mut pixels = vec![0; pixel_bytes(self.frames.extent)?];
            buffer.read(&mut pixels)?;
            if self.frames.format == vk::Format::B8G8R8A8_UNORM {
                for pixel in pixels.as_chunks_mut::<4>().0 {
                    pixel.swap(0, 2);
                }
            }
            Some(pixels)
        } else {
            None
        };
        Ok(Some(WindowPaint {
            presentation: WindowPresentation {
                time,
                pixels: [self.frames.extent.width, self.frames.extent.height],
                present_mode: present_mode_name(self.frames.mode),
                paint_elapsed: time.duration_since(started),
                present_elapsed: synchronous.then(|| time.duration_since(presenting)),
            },
            pixels,
        }))
    }
}

struct PresentRequest {
    swapchain: vk::SwapchainKHR,
    rendered: vk::Semaphore,
    index: u32,
}

/// CPU Vulkan drivers can block the caller of queue_present for an entire graph
/// frame. One worker presents the root surface while winit continues to deliver input.
struct SoftwarePresent {
    sender: Option<std::sync::mpsc::SyncSender<PresentRequest>>,
    results: std::sync::mpsc::Receiver<Result<bool, String>>,
    thread: Option<std::thread::JoinHandle<()>>,
    busy: Arc<std::sync::atomic::AtomicBool>,
    outstanding: bool,
}

impl SoftwarePresent {
    fn new(
        frames: &Frames,
        lock: Arc<std::sync::Mutex<()>>,
        window: Arc<Window>,
    ) -> Result<Self, String> {
        let (sender, requests) = std::sync::mpsc::sync_channel::<PresentRequest>(1);
        let (results_tx, results) = std::sync::mpsc::channel();
        let busy = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let running = busy.clone();
        let loader = frames.loader.clone();
        let queue = frames.queue;
        let thread = std::thread::Builder::new()
            .name("software-vulkan-present".into())
            .spawn(move || {
                while let Ok(req) = requests.recv() {
                    let result = present_image(
                        &loader,
                        &lock,
                        queue,
                        req.swapchain,
                        req.rendered,
                        req.index,
                    )
                    .map(|status| status.unwrap_or(true));
                    running.store(false, std::sync::atomic::Ordering::Release);
                    if results_tx.send(result).is_err() {
                        break;
                    }
                    window.request_redraw();
                }
            })
            .map_err(|e| format!("Start software presentation worker: {e}"))?;
        Ok(Self {
            sender: Some(sender),
            results,
            thread: Some(thread),
            busy,
            outstanding: false,
        })
    }

    fn busy(&self) -> bool {
        self.busy.load(std::sync::atomic::Ordering::Acquire)
    }

    fn submit(&mut self, request: PresentRequest) -> Result<(), String> {
        self.busy.store(true, std::sync::atomic::Ordering::Release);
        self.sender
            .as_ref()
            .expect("live presentation worker")
            .send(request)
            .map_err(|_| {
                self.busy.store(false, std::sync::atomic::Ordering::Release);
                "Software presentation worker stopped"
            })?;
        self.outstanding = true;
        Ok(())
    }

    fn complete(&mut self) -> Result<bool, String> {
        if !self.outstanding {
            return Ok(false);
        }
        self.outstanding = false;
        self.results
            .recv()
            .map_err(|_| "Software presentation worker disconnected")?
    }
}

impl Drop for SoftwarePresent {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn present_image(
    loader: &ash::khr::swapchain::Device,
    queue_access: &std::sync::Mutex<()>,
    queue: vk::Queue,
    swapchain: vk::SwapchainKHR,
    rendered: vk::Semaphore,
    index: u32,
) -> Result<Option<bool>, String> {
    let _queue = queue_access.lock().unwrap_or_else(|p| p.into_inner());
    // SAFETY: The frame owner retains the acquired image and semaphore until the call returns.
    let result = unsafe {
        loader.queue_present(
            queue,
            &vk::PresentInfoKHR::default()
                .wait_semaphores(&[rendered])
                .swapchains(&[swapchain])
                .image_indices(&[index]),
        )
    };
    match result {
        Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => Ok(None),
        other => other.map(Some).at("present painter frame"),
    }
}

impl Drop for Gpu {
    fn drop(&mut self) {
        // Also retire any submitted frame after an early return or failed present.
        if let Err(error) = self.finish() {
            eprintln!("{error}");
        }
    }
}

const TIMEOUT_NS: u64 = 3_000_000_000;
const COLOR: vk::ImageSubresourceRange = vk::ImageSubresourceRange {
    aspect_mask: vk::ImageAspectFlags::COLOR,
    base_mip_level: 0,
    level_count: 1,
    base_array_layer: 0,
    layer_count: 1,
};

fn pixel_bytes(extent: vk::Extent2D) -> Result<usize, String> {
    (extent.width as usize)
        .checked_mul(extent.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "Surface capture is too large".into())
}

struct PresentImage {
    raw: vk::Image,
    view: vk::ImageView,
    // The acquire dependency of this image also retires its last present wait.
    rendered: vk::Semaphore,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FrameState {
    Idle,
    Acquired,
    Submitted,
}

pub(super) struct Frames {
    pub device: ash::Device,
    pub queue: vk::Queue,
    pub pool: vk::CommandPool,
    pub format: vk::Format,
    extent: vk::Extent2D,
    mode: vk::PresentModeKHR,
    can_capture: bool,
    memory: vk::PhysicalDeviceMemoryProperties,
    loader: ash::khr::swapchain::Device,
    swapchain: vk::SwapchainKHR,
    images: Vec<PresentImage>,
    command: vk::CommandBuffer,
    acquired: vk::Semaphore,
    acquire_fence: vk::Fence,
    state: FrameState,
    #[cfg(test)]
    fail_before_submit: bool,
    fence: vk::Fence,
}

impl Frames {
    fn new(
        handles: &EditorDeviceHandles<'_>,
        extent: vk::Extent2D,
        benchmark: bool,
        old: vk::SwapchainKHR,
    ) -> Result<Self, String> {
        let device = handles.device;
        let surface = handles.surface_loader;
        let physical = handles.physical;
        let raw = handles.surface;

        // The surface and physical device were selected from the same live instance.
        unsafe {
            let caps = surface
                .get_physical_device_surface_capabilities(physical, raw)
                .at("surface capabilities")?;
            let extent = if caps.current_extent.width != u32::MAX && !benchmark {
                caps.current_extent
            } else {
                extent
            };
            let can_capture = caps
                .supported_usage_flags
                .contains(vk::ImageUsageFlags::TRANSFER_SRC);
            let mut usage = vk::ImageUsageFlags::COLOR_ATTACHMENT;
            if can_capture {
                usage |= vk::ImageUsageFlags::TRANSFER_SRC;
            }
            let alpha = [
                vk::CompositeAlphaFlagsKHR::OPAQUE,
                vk::CompositeAlphaFlagsKHR::INHERIT,
                vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED,
                vk::CompositeAlphaFlagsKHR::POST_MULTIPLIED,
            ]
            .into_iter()
            .find(|alpha| caps.supported_composite_alpha.contains(*alpha))
            .ok_or("Surface lacks a supported composite alpha mode")?;
            if !caps.supported_usage_flags.contains(usage)
                || (benchmark && !can_capture)
                || (caps.current_extent.width != u32::MAX && caps.current_extent != extent)
                || extent.width < caps.min_image_extent.width
                || extent.width > caps.max_image_extent.width
                || extent.height < caps.min_image_extent.height
                || extent.height > caps.max_image_extent.height
            {
                return Err("Unsupported Vulkan surface usage or client extent".into());
            }
            let supported = surface
                .get_physical_device_surface_formats(physical, raw)
                .at("surface formats")?;
            let format = [vk::Format::B8G8R8A8_UNORM, vk::Format::R8G8B8A8_UNORM]
                .into_iter()
                .find(|&format| {
                    supported.iter().any(|candidate| {
                        candidate.format == format
                            && candidate.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
                    })
                })
                .ok_or(
                    "Surface lacks an RGBA8/BGRA8 UNORM format with sRGB nonlinear presentation",
                )?;
            let modes = surface
                .get_physical_device_surface_present_modes(physical, raw)
                .at("surface modes")?;
            if benchmark && !modes.contains(&vk::PresentModeKHR::IMMEDIATE) {
                return Err(
                    "Surface lacks IMMEDIATE present mode required by the matched series".into(),
                );
            }
            let mode = if benchmark {
                vk::PresentModeKHR::IMMEDIATE
            } else {
                match std::env::var("MODULE_ENGINE_GUI_VSYNC").as_deref() {
                    Ok("off") => [
                        vk::PresentModeKHR::IMMEDIATE,
                        vk::PresentModeKHR::MAILBOX,
                        vk::PresentModeKHR::FIFO,
                    ]
                    .into_iter()
                    .find(|m| modes.contains(m))
                    .ok_or("No supported Vulkan presentation mode")?,
                    Ok("on") | Err(std::env::VarError::NotPresent) => {
                        if modes.contains(&vk::PresentModeKHR::MAILBOX) {
                            vk::PresentModeKHR::MAILBOX
                        } else {
                            vk::PresentModeKHR::FIFO
                        }
                    }
                    _ => return Err("MODULE_ENGINE_GUI_VSYNC must be on or off".into()),
                }
            };
            // Keep one frame slot and the minimum FIFO queue depth for interactive
            // windows. The R3.1 comparison retains its original surface configuration.
            let mut image_count = if benchmark || mode != vk::PresentModeKHR::FIFO {
                caps.min_image_count.saturating_add(1)
            } else {
                caps.min_image_count
            };
            if caps.max_image_count != 0 {
                image_count = image_count.min(caps.max_image_count);
            }
            let loader = ash::khr::swapchain::Device::new(handles.instance, device);
            let swapchain = loader
                .create_swapchain(
                    &vk::SwapchainCreateInfoKHR::default()
                        .surface(raw)
                        .min_image_count(image_count)
                        .image_format(format)
                        .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
                        .image_extent(extent)
                        .image_array_layers(1)
                        .image_usage(usage)
                        .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                        .pre_transform(caps.current_transform)
                        .composite_alpha(alpha)
                        .old_swapchain(old)
                        .present_mode(mode)
                        .clipped(true),
                    None,
                )
                .at("create swapchain")?;
            let mut frames = Self {
                device: device.clone(),
                queue: handles.queue,
                pool: vk::CommandPool::null(),
                format,
                extent,
                mode,
                can_capture,
                memory: handles
                    .instance
                    .get_physical_device_memory_properties(physical),
                loader,
                swapchain,
                images: Vec::new(),
                command: vk::CommandBuffer::null(),
                acquired: vk::Semaphore::null(),
                acquire_fence: vk::Fence::null(),
                state: FrameState::Idle,
                #[cfg(test)]
                fail_before_submit: false,
                fence: vk::Fence::null(),
            };
            for raw in frames
                .loader
                .get_swapchain_images(swapchain)
                .at("swapchain images")?
            {
                frames.images.push(PresentImage {
                    raw,
                    view: device
                        .create_image_view(
                            &vk::ImageViewCreateInfo::default()
                                .image(raw)
                                .view_type(vk::ImageViewType::TYPE_2D)
                                .format(format)
                                .subresource_range(COLOR),
                            None,
                        )
                        .at("create swapchain image view")?,
                    rendered: vk::Semaphore::null(),
                });
                frames
                    .images
                    .last_mut()
                    .expect("just pushed image")
                    .rendered = device
                    .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                    .at("create present semaphore")?;
            }
            frames.pool = device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(handles.family)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .at("create painter command pool")?;
            frames.command = device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(frames.pool)
                        .command_buffer_count(1),
                )
                .at("allocate frame command buffer")?[0];
            frames.acquired = device
                .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                .at("create acquire semaphore")?;
            frames.acquire_fence = device
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .at("create acquire fence")?;
            frames.fence = device
                .create_fence(
                    &vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED),
                    None,
                )
                .at("create frame fence")?;
            Ok(frames)
        }
    }

    fn wait(&self) -> Result<(), String> {
        let fences = [self.acquire_fence, self.fence];
        let active = match self.state {
            FrameState::Idle => return Ok(()),
            FrameState::Acquired => &fences[..1],
            FrameState::Submitted => &fences[..],
        };
        // SAFETY: Only wait for operations that were successfully issued.
        unsafe { self.device.wait_for_fences(active, true, TIMEOUT_NS) }
            .at("wait for painter frame")
    }

    fn draw(
        &mut self,
        queue_access: &std::sync::Mutex<()>,
        painter: &mut Painter,
        primitives: &[ClippedPrimitive],
        pixels_per_point: f32,
        readback: Option<&HostBuffer>,
    ) -> Result<Option<(u32, bool)>, String> {
        let device = &self.device;
        let command = self.command;
        // The previous frame's fence is signaled. This is the only submitter,
        // all resources are live, and this submission waits for image acquisition
        // before the first color attachment operation.
        unsafe {
            self.state = FrameState::Idle;
            device
                .reset_fences(&[self.acquire_fence])
                .at("reset acquire fence")?;
            let acquire = self.loader.acquire_next_image(
                self.swapchain,
                TIMEOUT_NS,
                self.acquired,
                self.acquire_fence,
            );
            if acquire == Err(vk::Result::ERROR_OUT_OF_DATE_KHR) {
                return Ok(None);
            }
            let (index, acquire_suboptimal) = acquire.at("acquire swapchain image")?;
            self.state = FrameState::Acquired;
            let image = &self.images[index as usize];
            device
                .reset_command_buffer(command, vk::CommandBufferResetFlags::empty())
                .at("reset painter command buffer")?;
            device
                .begin_command_buffer(
                    command,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .at("begin painter command buffer")?;
            self.transition(
                command,
                image.raw,
                (
                    vk::ImageLayout::UNDEFINED,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::AccessFlags::empty(),
                ),
                (
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                    vk::AccessFlags::COLOR_ATTACHMENT_READ
                        | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                ),
            );
            let rect = vk::Rect2D::default().extent(self.extent);
            let color = [vk::RenderingAttachmentInfo::default()
                .image_view(image.view)
                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .clear_value(vk::ClearValue {
                    color: vk::ClearColorValue {
                        float32: [17. / 255., 24. / 255., 35. / 255., 1.],
                    },
                })];
            device.cmd_begin_rendering(
                command,
                &vk::RenderingInfo::default()
                    .render_area(rect)
                    .layer_count(1)
                    .color_attachments(&color),
            );
            let painted = painter.record(self, command, self.extent, pixels_per_point, primitives);
            device.cmd_end_rendering(command);
            painted?;
            let color_written = (
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            );
            let before_present = if let Some(buffer) = readback {
                let transfer_read = (
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::AccessFlags::TRANSFER_READ,
                );
                self.transition(command, image.raw, color_written, transfer_read);
                device.cmd_copy_image_to_buffer(
                    command,
                    image.raw,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    buffer.raw,
                    &[vk::BufferImageCopy::default()
                        .image_subresource(
                            vk::ImageSubresourceLayers::default()
                                .aspect_mask(vk::ImageAspectFlags::COLOR)
                                .layer_count(1),
                        )
                        .image_extent(
                            vk::Extent3D::default()
                                .width(self.extent.width)
                                .height(self.extent.height)
                                .depth(1),
                        )],
                );
                let host = [vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::HOST_READ)];
                device.cmd_pipeline_barrier(
                    command,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::HOST,
                    vk::DependencyFlags::empty(),
                    &host,
                    &[],
                    &[],
                );
                transfer_read
            } else {
                color_written
            };
            self.transition(
                command,
                image.raw,
                before_present,
                (
                    vk::ImageLayout::PRESENT_SRC_KHR,
                    vk::PipelineStageFlags::BOTTOM_OF_PIPE,
                    vk::AccessFlags::empty(),
                ),
            );
            device
                .end_command_buffer(command)
                .at("end painter command buffer")?;
            device
                .reset_fences(&[self.fence])
                .at("reset painter fence")?;
            #[cfg(test)]
            if std::mem::take(&mut self.fail_before_submit) {
                return Err("injected submit failure".into());
            }
            let _queue = queue_access.lock().unwrap_or_else(|p| p.into_inner());
            device
                .queue_submit(
                    self.queue,
                    &[vk::SubmitInfo::default()
                        .wait_semaphores(&[self.acquired])
                        .wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
                        .command_buffers(&[command])
                        .signal_semaphores(&[image.rendered])],
                    self.fence,
                )
                .at("submit painter frame")?;
            self.state = FrameState::Submitted;
            Ok(Some((index, acquire_suboptimal)))
        }
    }

    fn present(
        &self,
        queue_access: &std::sync::Mutex<()>,
        window: &Window,
        index: u32,
    ) -> Result<Option<bool>, String> {
        window.pre_present_notify();
        present_image(
            &self.loader,
            queue_access,
            self.queue,
            self.swapchain,
            self.images[index as usize].rendered,
            index,
        )
    }

    pub fn transition(
        &self,
        command: vk::CommandBuffer,
        image: vk::Image,
        source: (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags),
        destination: (vk::ImageLayout, vk::PipelineStageFlags, vk::AccessFlags),
    ) {
        let barriers = [vk::ImageMemoryBarrier::default()
            .old_layout(source.0)
            .src_access_mask(source.2)
            .new_layout(destination.0)
            .dst_access_mask(destination.2)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(COLOR)];
        // Callers record into an owned command buffer outside dynamic rendering.
        // The queue owns the acquired image or the painter's local texture.
        unsafe {
            self.device.cmd_pipeline_barrier(
                command,
                source.1,
                destination.1,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &barriers,
            )
        };
    }

    pub fn host_buffer(
        &self,
        bytes: usize,
        usage: vk::BufferUsageFlags,
    ) -> Result<HostBuffer, String> {
        let device = &self.device;
        // The buffer owns its allocation; mapped accesses are apart from GPU use.
        unsafe {
            let mut buffer = HostBuffer {
                device: device.clone(),
                raw: device
                    .create_buffer(
                        &vk::BufferCreateInfo::default()
                            .size(bytes as u64)
                            .usage(usage),
                        None,
                    )
                    .at("create mapped painter buffer")?,
                memory: vk::DeviceMemory::null(),
                capacity: bytes,
            };
            let requirements = device.get_buffer_memory_requirements(buffer.raw);
            buffer.memory = self.allocate(
                requirements,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            )?;
            device
                .bind_buffer_memory(buffer.raw, buffer.memory, 0)
                .at("bind painter buffer")?;
            Ok(buffer)
        }
    }

    pub fn allocate(
        &self,
        requirements: vk::MemoryRequirements,
        flags: vk::MemoryPropertyFlags,
    ) -> Result<vk::DeviceMemory, String> {
        let index = (0..self.memory.memory_type_count)
            .find(|&index| {
                requirements.memory_type_bits & (1 << index) != 0
                    && self.memory.memory_types[index as usize]
                        .property_flags
                        .contains(flags)
            })
            .ok_or_else(|| format!("No Vulkan memory type for {flags:?}"))?;
        // The queried requirements belong to the resource for this device.
        unsafe {
            self.device
                .allocate_memory(
                    &vk::MemoryAllocateInfo::default()
                        .allocation_size(requirements.size)
                        .memory_type_index(index),
                    None,
                )
                .at("allocate painter memory")
        }
    }
}

impl Drop for Frames {
    fn drop(&mut self) {
        // On successful construction Gpu retires the device before field cleanup.
        // Partial constructors have not submitted work. All partial handles are null.
        unsafe {
            if self.state != FrameState::Idle {
                // WSI may still be signalling the acquisition even when no draw was submitted.
                let _ = self
                    .device
                    .wait_for_fences(&[self.acquire_fence], true, u64::MAX);
            }
            self.device.destroy_fence(self.acquire_fence, None);
            self.device.destroy_fence(self.fence, None);
            self.device.destroy_semaphore(self.acquired, None);
            self.device.destroy_command_pool(self.pool, None);
            for image in &self.images {
                self.device.destroy_semaphore(image.rendered, None);
                self.device.destroy_image_view(image.view, None);
            }
            self.loader.destroy_swapchain(self.swapchain, None);
        }
    }
}

pub(super) struct HostBuffer {
    device: ash::Device,
    pub raw: vk::Buffer,
    memory: vk::DeviceMemory,
    pub capacity: usize,
}

impl HostBuffer {
    #[cfg(feature = "spike")]
    pub fn write(&self, bytes: &[u8]) -> Result<(), String> {
        self.mapped(bytes.len(), |mapped| {
            // The successful mapping covers the entire source slice and never aliases it.
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), mapped, bytes.len()) };
        })
    }

    fn read(&self, bytes: &mut [u8]) -> Result<(), String> {
        self.mapped(bytes.len(), |mapped| {
            // The capture is coherent and the frame fence has signaled before this read.
            unsafe { std::ptr::copy_nonoverlapping(mapped, bytes.as_mut_ptr(), bytes.len()) };
        })
    }

    fn mapped(&self, count: usize, copy: impl FnOnce(*mut u8)) -> Result<(), String> {
        if count == 0 || count > self.capacity {
            return Err("Native painter buffer range is outside its allocation".into());
        }
        // The buffer owns coherent memory which is unmapped between calls.
        unsafe {
            let mapped = self
                .device
                .map_memory(self.memory, 0, count as u64, vk::MemoryMapFlags::empty())
                .at("map painter buffer")?;
            copy(mapped.cast());
            self.device.unmap_memory(self.memory);
        }
        Ok(())
    }
}

impl Drop for HostBuffer {
    fn drop(&mut self) {
        // The owning frame or upload has completed, or this is a partial constructor.
        unsafe {
            self.device.destroy_buffer(self.raw, None);
            self.device.free_memory(self.memory, None);
        }
    }
}

#[cfg(all(test, any(target_os = "linux", windows)))]
#[path = "gpu_tests.rs"]
mod tests;
