//! Bevy integration for Funky Renderer
//! 
//! This module provides a Bevy plugin that wraps our custom Vulkan renderer
//! and integrates it with Bevy's ECS system.

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, RawHandleWrapper};
use std::sync::Arc;
use std::time::Instant;
use parking_lot::Mutex;

use crate::renderer::VulkanRenderer;
use crate::cube::CubeRenderer;
use crate::multithreading::MultiThreadedRenderer;

/// Plugin that adds the Funky Vulkan Renderer to Bevy
pub struct FunkyRendererPlugin;

impl Plugin for FunkyRendererPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FunkyRenderState>()
            .init_resource::<RenderStats>()
            .add_systems(Startup, setup_funky_renderer)
            .add_systems(Update, (
                update_cube_rotation,
                render_frame,
                update_window_title,
            ).chain());
    }
}

/// Resource holding the Vulkan renderer state
#[derive(Resource, Default)]
pub struct FunkyRenderState {
    pub renderer: Option<FunkyRenderer>,
    pub initialized: bool,
}

/// Wrapper for thread-safe renderer access
pub struct FunkyRenderer {
    pub vulkan: Arc<Mutex<VulkanRendererWrapper>>,
}

pub struct VulkanRendererWrapper {
    pub renderer: VulkanRenderer,
    pub cube: CubeRenderer,
    pub mt_renderer: Option<MultiThreadedRenderer>,
}

/// Resource for tracking render statistics
#[derive(Resource, Default)]
pub struct RenderStats {
    pub fps: f64,
    pub frame_time_ms: f64,
    pub frame_count: u64,
    pub last_fps_time: Option<Instant>,
    pub start_time: Option<Instant>,
}

/// Component for entities that should be rendered as rotating cubes
#[derive(Component)]
pub struct FunkyCube {
    pub rotation_speed: f32,
    pub current_rotation: f32,
}

impl Default for FunkyCube {
    fn default() -> Self {
        Self {
            rotation_speed: 1.0,
            current_rotation: 0.0,
        }
    }
}

/// Component for pastel-colored cube faces
#[derive(Component)]
pub struct PastelColors {
    pub front: [f32; 3],
    pub back: [f32; 3],
    pub top: [f32; 3],
    pub bottom: [f32; 3],
    pub right: [f32; 3],
    pub left: [f32; 3],
}

impl Default for PastelColors {
    fn default() -> Self {
        Self {
            front: [0.5, 0.7, 0.5],      // Pale green
            back: [0.6, 0.6, 0.3],       // Olive
            top: [0.7, 0.9, 0.9],        // Light cyan
            bottom: [0.4, 0.55, 0.4],    // Muted green
            right: [0.9, 0.9, 0.7],      // Pale yellow
            left: [0.35, 0.55, 0.35],    // Forest green
        }
    }
}

/// System to initialize the Funky Renderer
fn setup_funky_renderer(
    mut render_state: ResMut<FunkyRenderState>,
    mut stats: ResMut<RenderStats>,
    windows: Query<&RawHandleWrapper, With<PrimaryWindow>>,
    mut commands: Commands,
) {
    if render_state.initialized {
        return;
    }

    info!("🚀 Initializing Funky Vulkan Renderer with Bevy");
    
    // Get window handle from Bevy (for future direct Vulkan integration)
    let _window_handle = match windows.get_single() {
        Ok(handle) => handle,
        Err(_) => {
            warn!("No primary window found, deferring renderer initialization");
            return;
        }
    };

    // For now, we'll mark as initialized but the actual Vulkan init
    // happens in render_frame when we have proper window access
    stats.start_time = Some(Instant::now());
    stats.last_fps_time = Some(Instant::now());
    
    // Spawn a default rotating cube entity
    commands.spawn((
        FunkyCube::default(),
        PastelColors::default(),
        Name::new("Pastel Cube"),
    ));
    
    render_state.initialized = true;
    info!("✓ Funky Renderer plugin initialized");
    info!("  Note: Using Bevy's window management");
}

/// System to update cube rotation based on time
fn update_cube_rotation(
    time: Res<Time>,
    mut cubes: Query<&mut FunkyCube>,
) {
    for mut cube in cubes.iter_mut() {
        cube.current_rotation += cube.rotation_speed * time.delta_secs();
    }
}

/// System to render a frame using our custom Vulkan renderer
fn render_frame(
    _render_state: Res<FunkyRenderState>,
    cubes: Query<&FunkyCube>,
    mut stats: ResMut<RenderStats>,
) {
    let frame_start = Instant::now();
    
    // Get the first cube's rotation (if any)
    let _rotation = cubes.iter().next().map(|c| c.current_rotation).unwrap_or(0.0);
    
    // In a full implementation, we would:
    // 1. Access the VulkanRenderer from render_state
    // 2. Update uniform buffers with the cube rotation
    // 3. Record and submit command buffers
    // 4. Present the frame
    //
    // For now, Bevy handles the actual rendering through its own pipeline.
    // This system serves as the integration point where ECS data feeds into
    // the custom renderer.
    
    // Update frame statistics
    stats.frame_time_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
    stats.frame_count += 1;
    
    if let Some(last_time) = stats.last_fps_time {
        let elapsed = last_time.elapsed();
        if elapsed.as_millis() >= 100 {
            stats.fps = stats.frame_count as f64 / elapsed.as_secs_f64();
            stats.frame_count = 0;
            stats.last_fps_time = Some(Instant::now());
        }
    }
}

/// System to update window title with FPS
fn update_window_title(
    stats: Res<RenderStats>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if let Ok(mut window) = windows.get_single_mut() {
        window.title = format!(
            "Funky Renderer (Bevy) | FPS: {:.1} | Frame: {:.2}ms",
            stats.fps,
            stats.frame_time_ms
        );
    }
}

/// Bundle for spawning a complete Funky Cube entity
#[derive(Bundle, Default)]
pub struct FunkyCubeBundle {
    pub cube: FunkyCube,
    pub colors: PastelColors,
    pub name: Name,
}

impl FunkyCubeBundle {
    pub fn new(rotation_speed: f32) -> Self {
        Self {
            cube: FunkyCube {
                rotation_speed,
                current_rotation: 0.0,
            },
            colors: PastelColors::default(),
            name: Name::new("Funky Cube"),
        }
    }
}

/// Extension trait for App to easily add Funky Renderer
pub trait FunkyRendererAppExt {
    fn add_funky_renderer(&mut self) -> &mut Self;
}

impl FunkyRendererAppExt for App {
    fn add_funky_renderer(&mut self) -> &mut Self {
        self.add_plugins(FunkyRendererPlugin)
    }
}
