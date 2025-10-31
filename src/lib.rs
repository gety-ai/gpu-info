#[cfg(not(target_os = "macos"))]
mod vulkan;

#[cfg(target_os = "macos")]
mod metal;

#[cfg(target_os = "macos")]
pub use metal::*;
#[cfg(not(target_os = "macos"))]
pub use vulkan::*;

// OpenGL related
// use glutin::{
//     context::{NotCurrentContext, PossiblyCurrentContext},
//     display::GetGlDisplay,
//     prelude::*,
//     surface::{Surface, WindowSurface},
// };
// use raw_window_handle::HasRawWindowHandle;
// use std::num::ParseIntError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to perform Vulkan operation: {0}")]
    VulkanOperationFailed(String),
    #[error("Vulkan is not supported on this platform")]
    VulkanNotSupported,
    #[error("Failed to create OpenGL context")]
    OpenGLContextCreationFailed,
    #[error("Failed to query GPU info")]
    OpenGLQueryFailed,

    #[cfg(target_os = "macos")]
    #[error("failed to query metal api: {0}")]
    Metal(#[from] metal::MetalError),
}

impl Error {
    pub fn is_vulkan_not_supported(&self) -> bool {
        matches!(self, Error::VulkanNotSupported)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum GPUKind {
    Integrated,
    Discrete,
    Virtual,
    CPU,
    Unknown,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum GPULocation {
    BuiltIn,
    Slot,
    External,
    #[default]
    Unspecified,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct GPU {
    pub kind: GPUKind,
    pub name: String,
    pub vendor: String,
    pub driver_version: String,
    /// 0 is means unknown or not available
    pub vram: u64,
    // pub max_resolution: Resolution,
    // pub current_resolution: Resolution,
    pub clock_speed: Option<u32>,
    pub temperature: Option<u32>,
}

pub fn retrieve_gpu_info() -> Result<Vec<GPU>, Error> {
    #[cfg(target_os = "macos")]
    let gpus = retrieve_gpu_info_via_metal()?
        .into_iter()
        .map(|g| g.into())
        .collect::<Vec<GPU>>();

    #[cfg(not(target_os = "macos"))]
    let gpus = Vec::new();

    Ok(gpus)
}

// pub fn retrieve_gpu_info_via_gl() -> Result<Vec<GPU>, Error> {
//     // Create a headless context
//     let event_loop = winit::event_loop::EventLoop::new();
//     let window_builder = winit::window::WindowAttributes::default()
//         .with_visible(false)
//         .with_inner_size(winit::dpi::LogicalSize::new(1, 1));

//     let template = glutin::config::ConfigTemplateBuilder::new()
//         .with_transparency(true)
//         .with_float_pixels(true)
//         .build();
//     let config = glutin::display::

//     let context = unsafe {
//         gl_config
//             .display()
//             .create_context(&gl_config, &window.raw_window_handle())
//             .map_err(|_| Error::OpenGLContextCreationFailed)?
//     };

//     let surface = Surface::new(&gl_config.display(), window.inner_size().into(), &window);

//     let context = context
//         .make_current(&surface)
//         .map_err(|_| Error::OpenGLContextCreationFailed)?;

//     // Load OpenGL functions
//     gl::load_with(|s| context.get_proc_address(s));

//     // Query GPU information
//     let vendor = unsafe {
//         let data = gl::GetString(gl::VENDOR);
//         std::ffi::CStr::from_ptr(data as *const i8)
//             .to_string_lossy()
//             .into_owned()
//     };

//     let renderer = unsafe {
//         let data = gl::GetString(gl::RENDERER);
//         std::ffi::CStr::from_ptr(data as *const i8)
//             .to_string_lossy()
//             .into_owned()
//     };

//     let version = unsafe {
//         let data = gl::GetString(gl::VERSION);
//         std::ffi::CStr::from_ptr(data as *const i8)
//             .to_string_lossy()
//             .into_owned()
//     };

//     // Try to determine GPU kind (this is a rough estimate)
//     let kind = if renderer.to_lowercase().contains("intel") {
//         GPUKind::Integrated
//     } else if renderer.to_lowercase().contains("nvidia") || renderer.to_lowercase().contains("amd")
//     {
//         GPUKind::Discrete
//     } else {
//         GPUKind::Unknown
//     };

//     // Try to get VRAM info (this is not standardized and may not work on all GPUs)
//     let vram = unsafe {
//         let mut vram_size = 0;
//         gl::GetIntegerv(
//             gl::GPU_MEMORY_INFO_TOTAL_AVAILABLE_MEMORY_NVX,
//             &mut vram_size,
//         );
//         if vram_size > 0 {
//             (vram_size as u64) * 1024 // Convert KB to bytes
//         } else {
//             0
//         }
//     };

//     let gpu = GPU {
//         kind,
//         name: renderer,
//         vendor,
//         driver_version: version,
//         vram,
//         clock_speed: None,
//         temperature: None,
//     };

//     Ok(vec![gpu])
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrieve_gpu_info() {
        let gpus = retrieve_gpu_info().unwrap();
        eprintln!("GPUs: {gpus:#?}");
        assert!(!gpus.is_empty());
    }
}
