//! Funky Renderer - Bevy ECS + Custom Vulkan Renderer
//! 
//! High-performance Vulkan rendering with Bevy ECS.
//! 
//! ## Running
//! 
//! ```bash
//! cargo run --release
//! ```

pub mod renderer;
pub mod cube;
pub mod multithreading;

// Re-exports for library usage
pub use renderer::VulkanRenderer;
pub use cube::CubeRenderer;
pub use multithreading::MultiThreadedRenderer;
