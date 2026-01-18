//! Funky Renderer - A custom Vulkan renderer with optional Bevy integration
//! 
//! ## Features
//! 
//! - `standalone` (default): Run as a standalone Vulkan application
//! - `bevy_plugin`: Enable Bevy ECS integration
//! 
//! ## Running
//! 
//! Standalone mode (default):
//! ```bash
//! cargo run --release
//! ```
//! 
//! Bevy integration:
//! ```bash
//! cargo run --bin funkyrenderer_bevy --features bevy_plugin --no-default-features --release
//! ```

pub mod renderer;
pub mod cube;
pub mod multithreading;

#[cfg(feature = "bevy_plugin")]
pub mod bevy_plugin;

// Re-exports for library usage
pub use renderer::VulkanRenderer;
pub use cube::CubeRenderer;
pub use multithreading::MultiThreadedRenderer;

#[cfg(feature = "bevy_plugin")]
pub use bevy_plugin::{FunkyRendererPlugin, FunkyCube, PastelColors, FunkyCubeBundle};
