//! Native Vulkan renderer. All shading, postprocessing, and Custom passes execute
//! on the GPU. Scene vertex preparation and final frame readback are CPU work.
mod adapter;
mod capabilities;
mod compiler;
mod frames;
mod geometry;
mod graph;
mod memory;
pub mod presentation;
mod profiling;
mod retirement;
mod staging;
mod transfer;
mod vulkan;
pub use adapter::{VulkanAdapter, install};
pub use capabilities::DeviceCapabilities;
pub use compiler::shader_dependency_revision_at;
pub use compiler::{
    ShaderArtifact, ShaderPerformance, compile_hlsl, compile_hlsl_at, dxc_path,
    precompile_graph_at, resource_root, shader_performance,
};
pub use error::{ShaderError, VulkanError};
pub use graph::builtin_hlsl_source;
pub use graph::{
    PassManifest, RenderPath, ResourceSpec, builtin_manifest, custom_compute_manifest,
    custom_raster_manifest, default_graph, demo_custom_compute_source, demo_custom_raster_source,
    module_descriptors, port_type, remove_pass_with_bypass, render_graph_targets,
};
pub use graph::{custom_fullscreen_manifest, demo_custom_fullscreen_source};
pub use instance::validation_error_count;
pub use module_engine_core::RendererPerformance;
use presentation::instance;
pub use retirement::{ProducerRetirementReport, finish_producer_retirements};
pub use vulkan::{
    CullingStats, FrameWork, RenderGraphValidator, ResidentScene, SceneCulling, SceneInput,
    VertexAllocation, VulkanRenderer, validate_overrides, validate_overrides_at,
};

mod error {
    use ash::vk;
    use std::{
        io,
        path::{Path, PathBuf},
        sync::Arc,
    };

    #[derive(Clone, Debug, thiserror::Error)]
    pub enum ShaderError {
        #[error(
            "No native DXC compiler for {platform}. Restore {bundled} with its native shared libraries, install DXC on PATH, or set MODULE_ENGINE_DXC. Precompiled Player shaders do not require DXC."
        )]
        MissingDxc {
            platform: &'static str,
            bundled: PathBuf,
        },
        #[error(
            "MODULE_ENGINE_DXC {path}: {source}; set it to a native executable, not Windows dxc.exe on Linux"
        )]
        DxcOverride {
            path: PathBuf,
            #[source]
            source: Arc<ShaderError>,
        },
        #[error("DXC executable {path}: {reason}")]
        InvalidDxc { path: PathBuf, reason: &'static str },
        #[cfg(target_os = "linux")]
        #[error("DXC library path: {0}")]
        DxcLibraryPath(#[source] Arc<std::env::JoinPathsError>),
        #[error(
            "Native DXC {path}: {source}; restore this platform's tools/dxc and shared libraries or set MODULE_ENGINE_DXC"
        )]
        DxcLaunch {
            path: PathBuf,
            #[source]
            source: Arc<io::Error>,
        },
        #[error("Native DXC {path} rejected --version ({status}): {diagnostics}")]
        DxcVersion {
            path: PathBuf,
            status: std::process::ExitStatus,
            diagnostics: String,
        },
        #[error("HLSL compilation failed ({profile}, entry {entry}):\n{diagnostics}")]
        Compile {
            profile: String,
            entry: String,
            status: std::process::ExitStatus,
            diagnostics: String,
        },
        #[error("{operation} {path}: {source}")]
        Io {
            operation: &'static str,
            path: PathBuf,
            #[source]
            source: Arc<io::Error>,
        },
        #[error("Precompiled Player shader {path}: {source}; re-export this graph")]
        SealedArtifact {
            path: PathBuf,
            #[source]
            source: Arc<io::Error>,
        },
        #[error("HLSL include {path} must be UTF-8: {source}")]
        IncludeEncoding {
            path: PathBuf,
            #[source]
            source: Arc<std::string::FromUtf8Error>,
        },
        #[error("{0}")]
        Include(String),
        #[error("{0}")]
        InvalidSpirv(&'static str),
        #[error("{0}")]
        ShaderInterface(String),
        #[error("HLSL dependencies changed during compilation; retry the current files")]
        DependenciesChanged,
    }

    impl ShaderError {
        pub(crate) fn io(operation: &'static str, path: &Path, source: io::Error) -> Self {
            Self::Io {
                operation,
                path: path.into(),
                source: Arc::new(source),
            }
        }

        pub(crate) fn dxc_unavailable(&self) -> bool {
            matches!(self, Self::MissingDxc { .. })
        }
    }

    #[derive(Clone, Debug, thiserror::Error)]
    pub enum VulkanError {
        #[error("Vulkan loader: {0}")]
        Loader(#[source] Arc<ash::LoadingError>),
        #[error(
            "No Vulkan 1.3 graphics+compute adapter with dynamic rendering; a compatible GPU driver is required"
        )]
        NoAdapter,
        #[error(
            "No matching GUI adapter with Vulkan 1.3 dynamic rendering and external-memory import"
        )]
        NoPresentationAdapter,
        #[error("Vulkan {operation}: {source:?}")]
        Api {
            operation: &'static str,
            #[source]
            source: vk::Result,
        },
        #[error("Vulkan device {name}: {source:?}")]
        CreateDevice {
            name: String,
            #[source]
            source: vk::Result,
        },
        #[error("Vulkan memory type {required:?} unavailable (compatible bits {bits:#x})")]
        MemoryType {
            bits: u32,
            required: vk::MemoryPropertyFlags,
        },
        #[error("Register Vulkan modules: {0}")]
        Registration(String),
        #[error("Adapter {adapter} lacks required format capabilities {format:?}: {required:?}")]
        FormatSupport {
            adapter: String,
            format: vk::Format,
            required: vk::FormatFeatureFlags,
        },
        #[error("GPU graph exceeds 64 passes")]
        PassLimit,
        #[error("{0}")]
        Graph(String),
        #[error("Custom {pass} manifest: {source}")]
        Manifest {
            pass: String,
            #[source]
            source: Arc<serde_json::Error>,
        },
        #[error("{0}")]
        Overrides(String),
        #[error("{0}")]
        Resource(String),
        #[error("GPU scene mesh pool exceeds the device storage buffer range")]
        SceneBufferLimit,
        #[error("{0}")]
        Presentation(String),
        #[error("GPU viewport must be nonzero, <=8192 per axis and <=16M pixels")]
        Viewport,
        #[error("Unknown GPU producer presentation ticket {0}")]
        UnknownTicket(u64),
        #[error("GPU presentation ticket exhausted")]
        TicketExhausted,
        #[error(
            "GPU producer submission is still in flight; poll or discard its ticket before CPU readback"
        )]
        ProducerBusy,
        #[error("CPU render did not complete")]
        NoReadback,
        #[error("GPU {operation}: {source:?}; resources remain pinned")]
        ProducerUnconfirmed {
            operation: &'static str,
            #[source]
            source: vk::Result,
        },
        #[error("Pass {pass}: {source}")]
        Pass {
            pass: String,
            #[source]
            source: Arc<VulkanError>,
        },
        #[error("Pass {0}: HLSL dependencies changed during pipeline preparation")]
        PassDependenciesChanged(String),
        #[error("{stage} entry contains NUL: {source}")]
        ShaderEntry {
            stage: &'static str,
            #[source]
            source: std::ffi::NulError,
        },
        #[error("{0}")]
        Profiling(&'static str),
        #[error(transparent)]
        Shader(#[from] ShaderError),
    }

    impl VulkanError {
        pub(crate) fn api(operation: &'static str) -> impl FnOnce(vk::Result) -> Self {
            move |source| Self::Api { operation, source }
        }

        pub(crate) fn gpu_unavailable(&self) -> bool {
            match self {
                Self::Loader(source) => {
                    matches!(source.as_ref(), ash::LoadingError::LibraryLoadFailure(_))
                }
                Self::NoAdapter => true,
                Self::Api {
                    operation: "1.3 instance",
                    source: vk::Result::ERROR_INCOMPATIBLE_DRIVER,
                } => true,
                _ => false,
            }
        }
    }
}

#[doc(hidden)]
pub mod verification {
    //! Capability preflight for source-checkout verification routes.
    use crate::{VulkanRenderer, compiler, validation_error_count};
    use std::sync::OnceLock;

    #[derive(Clone, Copy)]
    pub enum Capability {
        Vulkan,
        Dxc,
    }

    // Cache probes within the test executable. A prepared graph or a cache hit is
    // never used as evidence that a driver/compiler is available.
    pub fn unavailable(requirements: &[Capability]) -> Result<Vec<String>, String> {
        static GPU: OnceLock<Result<Option<String>, String>> = OnceLock::new();
        static DXC: OnceLock<Result<Option<String>, String>> = OnceLock::new();

        let mut missing = Vec::new();
        for requirement in requirements {
            let result = match requirement {
                Capability::Vulkan => GPU.get_or_init(|| match VulkanRenderer::new() {
                    Ok(_) => Ok(None),
                    Err(error) if error.gpu_unavailable() => Ok(Some(format!("Vulkan: {error}"))),
                    Err(error) => Err(format!("Vulkan capability probe failed: {error}")),
                }),
                Capability::Dxc => {
                    DXC.get_or_init(|| match compiler::probe_dxc(&compiler::resource_root()) {
                        Ok(()) => Ok(None),
                        Err(error) if error.dxc_unavailable() => Ok(Some(format!("DXC: {error}"))),
                        Err(error) => Err(format!("DXC capability probe failed: {error}")),
                    })
                }
            };
            if let Some(reason) = result.as_ref().map_err(Clone::clone)? {
                missing.push(reason.clone());
            }
        }
        Ok(missing)
    }

    /// Test guard runs before fixtures and checks validation messages through their teardown.
    pub struct ValidationGuard(u64);

    impl Drop for ValidationGuard {
        fn drop(&mut self) {
            if !std::thread::panicking() {
                assert_eq!(
                    validation_error_count(),
                    self.0,
                    "Vulkan validation reported errors"
                );
            }
        }
    }

    pub fn requires(requirements: &[Capability]) -> Option<ValidationGuard> {
        let guard = ValidationGuard(validation_error_count());
        let missing = unavailable(requirements).unwrap_or_else(|error| panic!("{error}"));
        if missing.is_empty() {
            Some(guard)
        } else {
            let current = std::thread::current();
            let name = current
                .name()
                .expect("capability guard must run on a named libtest thread");
            let reason = missing.join(" | ").replace(['\r', '\n', '\t'], " ");
            println!("\nMODULEENGINE_TEST_SKIP\t{name}\t{reason}");
            None
        }
    }

    /// Declare the real native prerequisites at the entry of a libtest case.
    #[macro_export]
    macro_rules! requires_capabilities {
        ($($requirement:ident),+ $(,)?) => {
            let Some(_vulkan_validation) = $crate::verification::requires(&[
                $($crate::verification::Capability::$requirement),+
            ]) else { return };
        };
    }
}
