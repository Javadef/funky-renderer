//! Funky Renderer - Bevy ECS + Custom Vulkan + egui
//! 
//! Uses Bevy's ECS for game logic, custom ash/Vulkan for rendering, egui for debug UI.

mod renderer;
mod cube;
mod multithreading;
mod egui_integration;
mod egui_vulkan;

use renderer::VulkanRenderer;
use cube::CubeRenderer;
use egui_integration::{EguiIntegration, UiData, ComponentCounts};
use egui_vulkan::EguiVulkanRenderer;
use ash::vk;
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

// Bevy ECS imports
use bevy_ecs::prelude::*;

// ============================================================================
// COMPONENTS
// ============================================================================

#[derive(Component, Default, Clone, Copy)]
pub struct Transform {
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
}

impl Transform {
    pub fn new() -> Self {
        Self {
            position: glam::Vec3::ZERO,
            rotation: glam::Quat::IDENTITY,
            scale: glam::Vec3::ONE,
        }
    }
}

#[derive(Component, Default, Clone, Copy)]
pub struct Velocity {
    pub linear: glam::Vec3,
    pub angular: glam::Vec3,
}

#[derive(Component)]
pub struct SpinningCube;

#[derive(Component)]
pub struct Renderable;

#[derive(Component)]
pub struct Camera {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { fov: 45.0_f32.to_radians(), near: 0.1, far: 100.0 }
    }
}

// ============================================================================
// RESOURCES
// ============================================================================

#[derive(Resource, Default)]
pub struct PerformanceStats {
    pub fps: f64,
    pub frame_time_ms: f64,
    pub frame_count: u64,
    pub last_fps_update: Option<Instant>,
}

#[derive(Resource)]
pub struct FrameTiming {
    pub start_time: Instant,
    pub delta_time: f32,
}

impl Default for FrameTiming {
    fn default() -> Self {
        Self { start_time: Instant::now(), delta_time: 0.016 }
    }
}

// ============================================================================
// SYSTEMS
// ============================================================================

fn setup_scene(mut commands: Commands) {
    println!("🎬 Setting up scene with Bevy ECS...");
    commands.spawn((Camera::default(), Transform::new()));
    commands.spawn((
        SpinningCube,
        Renderable,
        Transform::new(),
        Velocity { linear: glam::Vec3::ZERO, angular: glam::Vec3::new(0.0, 1.0, 0.0) },
    ));
    println!("✓ Scene setup complete - 1 camera, 1 spinning cube");
}

fn rotation_system(timing: Res<FrameTiming>, mut query: Query<(&mut Transform, &Velocity)>) {
    let dt = timing.delta_time;
    for (mut transform, velocity) in query.iter_mut() {
        if velocity.angular != glam::Vec3::ZERO {
            let rotation = glam::Quat::from_euler(
                glam::EulerRot::YXZ,
                velocity.angular.y * dt,
                velocity.angular.x * dt,
                velocity.angular.z * dt,
            );
            transform.rotation = rotation * transform.rotation;
        }
        transform.position += velocity.linear * dt;
    }
}

fn update_performance_stats(mut stats: ResMut<PerformanceStats>) {
    stats.frame_count += 1;
    let now = Instant::now();
    let last_update = stats.last_fps_update.get_or_insert(now);
    let elapsed = now.duration_since(*last_update);
    if elapsed.as_millis() >= 100 {
        stats.fps = stats.frame_count as f64 / elapsed.as_secs_f64();
        stats.frame_time_ms = 1000.0 / stats.fps;
        stats.frame_count = 0;
        stats.last_fps_update = Some(now);
    }
}

// ============================================================================
// APP
// ============================================================================

struct App {
    window: Option<Window>,
    renderer: Option<VulkanRenderer>,
    cube_renderer: Option<CubeRenderer>,
    
    // Bevy ECS
    world: World,
    schedule: Schedule,
    startup_schedule: Schedule,
    startup_done: bool,
    
    // egui
    egui_integration: Option<EguiIntegration>,
    egui_vulkan: Option<EguiVulkanRenderer>,
    
    last_frame_time: Instant,
    minimized: bool,
}

impl App {
    fn new() -> Self {
        let mut world = World::new();
        world.insert_resource(PerformanceStats::default());
        world.insert_resource(FrameTiming::default());
        
        let mut startup_schedule = Schedule::default();
        startup_schedule.add_systems(setup_scene);
        
        let mut schedule = Schedule::default();
        schedule.add_systems((rotation_system, update_performance_stats));
        
        Self {
            window: None,
            renderer: None,
            cube_renderer: None,
            world,
            schedule,
            startup_schedule,
            startup_done: false,
            egui_integration: None,
            egui_vulkan: None,
            last_frame_time: Instant::now(),
            minimized: false,
        }
    }
    
    fn get_cube_rotation(&mut self) -> f32 {
        let mut rotation_y = 0.0f32;
        let mut query = self.world.query::<(&SpinningCube, &Transform)>();
        for (_, transform) in query.iter(&self.world) {
            let (yaw, _, _) = transform.rotation.to_euler(glam::EulerRot::YXZ);
            rotation_y = yaw;
        }
        rotation_y
    }
    
    fn update_window_title(&self) {
        if let Some(window) = &self.window {
            let stats = self.world.resource::<PerformanceStats>();
            let title = format!(
                "Funky Renderer | FPS: {:.1} | Frame: {:.2}ms | Bevy ECS + Vulkan + egui",
                stats.fps, stats.frame_time_ms
            );
            window.set_title(&title);
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        
        println!("🚀 Funky Vulkan Renderer - Bevy ECS + egui Edition");
        println!("════════════════════════════════════════════");
        
        let window_attributes = Window::default_attributes()
            .with_title("Funky Renderer | Initializing...")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
            .with_resizable(true);
        
        let window = event_loop.create_window(window_attributes).unwrap();
        
        unsafe {
            match VulkanRenderer::new(&window) {
                Ok(renderer) => {
                    println!("✓ Vulkan renderer initialized");
                    println!("  Resolution: {}x{}", 
                        renderer.swapchain_extent.width, 
                        renderer.swapchain_extent.height);
                    
                    match CubeRenderer::new(&renderer) {
                        Ok(cube) => {
                            println!("✓ Cube geometry created");
                            self.cube_renderer = Some(cube);
                        }
                        Err(e) => {
                            eprintln!("✗ Failed to create cube: {}", e);
                        }
                    }
                    
                    // Initialize egui
                    let egui_integration = EguiIntegration::new(&window);
                    let egui_vulkan = EguiVulkanRenderer::new(
                        &renderer.device,
                        renderer.physical_device,
                        &renderer.instance,
                        renderer.render_pass,
                        &egui_integration.ctx,
                        renderer.graphics_queue,
                        renderer.graphics_queue_family_index,
                    );
                    self.egui_integration = Some(egui_integration);
                    self.egui_vulkan = Some(egui_vulkan);
                    println!("✓ egui debug UI initialized");
                    
                    self.renderer = Some(renderer);
                }
                Err(e) => {
                    eprintln!("✗ Failed to initialize Vulkan: {}", e);
                    event_loop.exit();
                    return;
                }
            }
        }
        
        if !self.startup_done {
            self.startup_schedule.run(&mut self.world);
            self.startup_done = true;
        }
        
        println!("\n🎮 Controls:");
        println!("   ESC - Exit");
        println!("   F3 - Toggle UI");
        println!("   F11 - Toggle Fullscreen\n");
        
        self.window = Some(window);
    }
    
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Let egui handle events first
        if let Some(egui) = &mut self.egui_integration {
            if let Some(window) = &self.window {
                if egui.handle_event(window, &event) {
                    return;
                }
            }
        }
        
        match event {
            WindowEvent::CloseRequested => {
                self.cleanup();
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state.is_pressed() {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => {
                            self.cleanup();
                            event_loop.exit();
                        }
                        PhysicalKey::Code(KeyCode::F3) => {
                            if let Some(egui) = &mut self.egui_integration {
                                egui.toggle_ui();
                            }
                        }
                        PhysicalKey::Code(KeyCode::F11) => {
                            if let Some(window) = &self.window {
                                let is_fullscreen = window.fullscreen().is_some();
                                if is_fullscreen {
                                    window.set_fullscreen(None);
                                } else {
                                    window.set_fullscreen(Some(
                                        winit::window::Fullscreen::Borderless(None)
                                    ));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                if new_size.width == 0 || new_size.height == 0 {
                    self.minimized = true;
                } else {
                    self.minimized = false;
                    if let Some(renderer) = &mut self.renderer {
                        renderer.framebuffer_resized = true;
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if !self.minimized {
                    self.render_frame();
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
    
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl App {
    fn render_frame(&mut self) {
        // Update delta time
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;
        
        {
            let mut timing = self.world.resource_mut::<FrameTiming>();
            timing.delta_time = delta;
        }
        
        // Run ECS systems
        self.schedule.run(&mut self.world);
        
        // Get rotation from ECS
        let rotation = self.get_cube_rotation();
        
        let renderer = match &mut self.renderer {
            Some(r) => r,
            None => return,
        };
        
        let cube = match &mut self.cube_renderer {
            Some(c) => c,
            None => return,
        };
        
        let window_size = self.window.as_ref().map(|w| w.inner_size());
        
        unsafe {
            renderer.device.wait_for_fences(
                &[renderer.in_flight_fences[renderer.current_frame]],
                true,
                u64::MAX,
            ).unwrap();
            
            let result = renderer.swapchain_fn.acquire_next_image(
                renderer.swapchain,
                u64::MAX,
                renderer.image_available_semaphores[renderer.current_frame],
                vk::Fence::null(),
            );
            
            let image_index = match result {
                Ok((index, suboptimal)) => {
                    if suboptimal {
                        renderer.framebuffer_resized = true;
                    }
                    index
                }
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    // Recreate swapchain
                    if let Some(size) = window_size {
                        let _ = renderer.recreate_swapchain(size.width, size.height);
                    }
                    return;
                }
                Err(e) => {
                    eprintln!("Failed to acquire image: {:?}", e);
                    return;
                }
            };
            
            renderer.device.reset_fences(
                &[renderer.in_flight_fences[renderer.current_frame]],
            ).unwrap();
            
            // Start command buffer
            let begin_info = vk::CommandBufferBeginInfo::default();
            renderer.device.begin_command_buffer(
                renderer.command_buffers[renderer.current_frame],
                &begin_info,
            ).unwrap();
            
            // Update uniform buffer with rotation FROM ECS
            if let Err(e) = cube.update_uniform_buffer(renderer, renderer.current_frame, rotation) {
                eprintln!("Failed to update uniform buffer: {}", e);
                return;
            }
            
            // Begin render pass
            let clear_values = [vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.53, 0.81, 0.92, 1.0],  // Sky blue
                },
            }];
            
            let render_pass_info = vk::RenderPassBeginInfo::default()
                .render_pass(renderer.render_pass)
                .framebuffer(renderer.framebuffers[image_index as usize])
                .render_area(vk::Rect2D {
                    offset: vk::Offset2D { x: 0, y: 0 },
                    extent: renderer.swapchain_extent,
                })
                .clear_values(&clear_values);
            
            renderer.device.cmd_begin_render_pass(
                renderer.command_buffers[renderer.current_frame],
                &render_pass_info,
                vk::SubpassContents::INLINE,
            );
            
            // Draw cube
            if let Err(e) = cube.draw(renderer, renderer.command_buffers[renderer.current_frame], renderer.current_frame) {
                eprintln!("Failed to draw cube: {:?}", e);
                renderer.device.cmd_end_render_pass(renderer.command_buffers[renderer.current_frame]);
                return;
            }
            
            // Render egui
            if let (Some(egui_int), Some(egui_vk), Some(window)) = 
                (&mut self.egui_integration, &mut self.egui_vulkan, &self.window) 
            {
                // Skip egui entirely if UI is hidden for max FPS
                if !egui_int.ui_visible {
                    renderer.device.cmd_end_render_pass(renderer.command_buffers[renderer.current_frame]);
                } else {
                    // Get stats before querying
                    let (fps, frame_time_ms) = {
                        let stats = self.world.resource::<PerformanceStats>();
                        (stats.fps, stats.frame_time_ms)
                    };
                    
                    let entity_count = self.world.entities().len() as usize;
                    
                    let component_counts = ComponentCounts {
                        transforms: self.world.query::<&Transform>().iter(&self.world).count(),
                        velocities: self.world.query::<&Velocity>().iter(&self.world).count(),
                        cameras: self.world.query::<&Camera>().iter(&self.world).count(),
                        renderables: self.world.query::<&Renderable>().iter(&self.world).count(),
                    };
                    
                    let ui_data = UiData {
                        fps,
                        frame_time_ms,
                        entity_count,
                        component_counts,
                        vulkan_version: renderer.vulkan_version.clone(),
                        gpu_name: renderer.gpu_name.clone(),
                    };
                    
                    let full_output = egui_int.build_ui(window, &ui_data);
                    let clipped_primitives = egui_int.ctx.tessellate(
                        full_output.shapes,
                        full_output.pixels_per_point,
                    );
                    
                    egui_vk.render(
                        &renderer.device,
                        renderer.command_buffers[renderer.current_frame],
                        renderer.swapchain_extent.width,
                        renderer.swapchain_extent.height,
                        clipped_primitives,
                        full_output.pixels_per_point,
                    );
                    
                    // End render pass after egui
                    renderer.device.cmd_end_render_pass(renderer.command_buffers[renderer.current_frame]);
                }
            } else {
                renderer.device.cmd_end_render_pass(renderer.command_buffers[renderer.current_frame]);
            }
            
            // End command buffer
            renderer.device.end_command_buffer(renderer.command_buffers[renderer.current_frame]).unwrap();
            
            // Submit command buffer
            let wait_semaphores = [renderer.image_available_semaphores[renderer.current_frame]];
            let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let command_buffers = [renderer.command_buffers[renderer.current_frame]];
            let signal_semaphores = [renderer.render_finished_semaphores[renderer.current_frame]];
            
            let submit_info = vk::SubmitInfo::default()
                .wait_semaphores(&wait_semaphores)
                .wait_dst_stage_mask(&wait_stages)
                .command_buffers(&command_buffers)
                .signal_semaphores(&signal_semaphores);
            
            renderer.device.queue_submit(
                renderer.graphics_queue,
                &[submit_info],
                renderer.in_flight_fences[renderer.current_frame],
            ).unwrap();
            
            // Present
            let swapchains = [renderer.swapchain];
            let image_indices = [image_index];
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&signal_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);
            
            let present_result = renderer.swapchain_fn.queue_present(
                renderer.present_queue,
                &present_info,
            );
            
            // Check if we need to recreate swapchain
            let should_recreate = match present_result {
                Ok(suboptimal) => suboptimal || renderer.framebuffer_resized,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => true,
                Err(e) => {
                    eprintln!("Present error: {:?}", e);
                    false
                }
            };
            
            if should_recreate {
                if let Some(size) = window_size {
                    let _ = renderer.recreate_swapchain(size.width, size.height);
                }
            }
            
            renderer.current_frame = (renderer.current_frame + 1) % renderer::MAX_FRAMES_IN_FLIGHT;
        }
        
        // Update window title
        let stats = self.world.resource::<PerformanceStats>();
        if stats.frame_count == 0 {
            self.update_window_title();
        }
    }
    
    fn cleanup(&mut self) {
        println!("\n👋 Shutting down...");
        
        if let Some(renderer) = &self.renderer {
            unsafe {
                renderer.device.device_wait_idle().unwrap();
                
                if let Some(egui_vk) = &self.egui_vulkan {
                    egui_vk.cleanup(&renderer.device);
                }
                
                if let Some(cube) = &mut self.cube_renderer {
                    cube.cleanup(renderer);
                }
            }
        }
        
        println!("✓ Cleanup complete");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}
