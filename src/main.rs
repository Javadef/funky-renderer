//! Funky Renderer - Bevy ECS + Custom Vulkan + egui
//! 
//! Uses Bevy's ECS for game logic, custom ash/Vulkan for rendering, egui for debug UI.

mod renderer;
mod cube;
mod multithreading;
mod egui_integration;
mod egui_vulkan;
mod gltf_loader;
mod gltf_renderer;

use renderer::VulkanRenderer;
use egui_integration::{EguiIntegration, UiData, ComponentCounts};
use egui_vulkan::EguiVulkanRenderer;
use gltf_loader::GltfScene;
use gltf_renderer::GltfRenderer;
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
pub struct GltfModel {
    pub path: String,
}

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

#[derive(Resource)]
pub struct CameraController {
    pub position: glam::Vec3,
    pub yaw: f32,   // Rotation around Y axis
    pub pitch: f32, // Rotation around X axis
    pub fov: f32,   // Field of view for zoom
    pub move_speed: f32,
    pub rotate_speed: f32,
    pub zoom_speed: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        // Spawn camera already looking at the origin (where we place the duck)
        // A slightly higher/farther view makes the ground plane visible.
        let position = glam::Vec3::new(0.0, 2.5, 10.0);
        let target = glam::Vec3::new(0.0, 0.6, 0.0);
        let dir = (target - position).normalize_or_zero();
        let yaw = dir.z.atan2(dir.x);
        let pitch = dir.y.asin().clamp(-89.0_f32.to_radians(), 89.0_f32.to_radians());

        Self {
            position,
            yaw,
            pitch,
            fov: 45.0_f32.to_radians(),
            move_speed: 5.0,
            rotate_speed: 3.0, // Fast enough for comfortable 360° rotation
            zoom_speed: 0.5,
        }
    }
}

#[derive(Resource)]
pub struct SceneObjects {
    pub gltf_scale: f32,
    pub gltf_min_y: f32,
}

impl Default for SceneObjects {
    fn default() -> Self {
        Self {
            gltf_scale: 0.01,
            gltf_min_y: 0.0,
        }
    }
}

#[derive(Resource, Clone, Copy)]
pub struct ShadowSettings {
    pub debug_cascades: bool,
    // Shadow softness radius in texels (higher = softer / more expensive).
    pub softness: f32,
}

impl Default for ShadowSettings {
    fn default() -> Self {
        Self {
            debug_cascades: false,
            softness: 2.5,
        }
    }
}

// ============================================================================
// SYSTEMS
// ============================================================================

fn setup_scene(mut commands: Commands) {
    println!("🎬 Setting up scene with Bevy ECS...");
    commands.spawn((Camera::default(), Transform::new()));

    println!("✓ Scene setup complete - 1 camera");
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
    // 500ms window for smoother FPS display
    if elapsed.as_millis() >= 500 {
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
    gltf_renderer: Option<GltfRenderer>,
    
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
    
    // Input state
    keys_pressed: std::collections::HashSet<KeyCode>,
}

impl App {
    fn new() -> Self {
        let mut world = World::new();
        world.insert_resource(PerformanceStats::default());
        world.insert_resource(FrameTiming::default());
        world.insert_resource(CameraController::default());
        world.insert_resource(SceneObjects::default());
        world.insert_resource(ShadowSettings::default());
        
        let mut startup_schedule = Schedule::default();
        startup_schedule.add_systems(setup_scene);
        
        let mut schedule = Schedule::default();
        schedule.add_systems((rotation_system, update_performance_stats));
        
        Self {
            window: None,
            renderer: None,
            gltf_renderer: None,
            world,
            schedule,
            startup_schedule,
            startup_done: false,
            egui_integration: None,
            egui_vulkan: None,
            last_frame_time: Instant::now(),
            minimized: false,
            keys_pressed: std::collections::HashSet::new(),
        }
    }
    
    fn update_camera(&mut self) {
        let delta = {
            let timing = self.world.resource::<FrameTiming>();
            timing.delta_time
        };
        
        let mut camera = self.world.resource_mut::<CameraController>();
        let speed = camera.move_speed * delta;
        let rot_speed = camera.rotate_speed * delta;
        
        // Movement should match the same yaw/pitch convention used by the renderer.
        // (Previously movement used a different yaw basis, which made A/D feel swapped
        // and W/S not align with the camera view.)
        let mut forward = glam::Vec3::new(
            camera.yaw.cos() * camera.pitch.cos(),
            0.0,
            camera.yaw.sin() * camera.pitch.cos(),
        );
        if forward.length_squared() < 1e-6 {
            forward = glam::Vec3::Z;
        }
        forward = forward.normalize();

        // Right-handed: right = forward x up
        let right = forward.cross(glam::Vec3::Y).normalize();
        
        // WASD movement
        if self.keys_pressed.contains(&KeyCode::KeyW) {
            camera.position += forward * speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyS) {
            camera.position -= forward * speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyA) {
            camera.position -= right * speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyD) {
            camera.position += right * speed;
        }
        
        // QE for up/down
        if self.keys_pressed.contains(&KeyCode::KeyQ) {
            camera.position.y -= speed;
        }
        if self.keys_pressed.contains(&KeyCode::KeyE) {
            camera.position.y += speed;
        }
        
        // Arrow keys for rotation - yaw is unbounded for full 360° horizontal rotation
        if self.keys_pressed.contains(&KeyCode::ArrowLeft) {
            camera.yaw -= rot_speed;
        }
        if self.keys_pressed.contains(&KeyCode::ArrowRight) {
            camera.yaw += rot_speed;
        }

        // Pitch is clamped to prevent gimbal lock / camera flip
        const MAX_PITCH: f32 = 89.0_f32.to_radians();
        if self.keys_pressed.contains(&KeyCode::ArrowUp) {
            camera.pitch = (camera.pitch + rot_speed).clamp(-MAX_PITCH, MAX_PITCH);
        }
        if self.keys_pressed.contains(&KeyCode::ArrowDown) {
            camera.pitch = (camera.pitch - rot_speed).clamp(-MAX_PITCH, MAX_PITCH);
        }

        // Keep yaw in [0, 2π) to avoid float precision issues over time
        camera.yaw = camera.yaw.rem_euclid(std::f32::consts::TAU);
        
        // Z/X keys for zoom (adjust FOV)
        if self.keys_pressed.contains(&KeyCode::KeyZ) {
            camera.fov = (camera.fov - camera.zoom_speed * delta).clamp(10.0_f32.to_radians(), 120.0_f32.to_radians());
        }
        if self.keys_pressed.contains(&KeyCode::KeyX) {
            camera.fov = (camera.fov + camera.zoom_speed * delta).clamp(10.0_f32.to_radians(), 120.0_f32.to_radians());
        }
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
                    
                    // Load glTF scene (if available)
                    let gltf_paths = [
                        "models/scene.gltf",
                        "models/model.gltf",
                        "scene.gltf",
                        "model.gltf",
                    ];
                    
                    for path in &gltf_paths {
                        if std::path::Path::new(path).exists() {
                            println!("📦 Loading glTF scene from: {}", path);
                            match GltfScene::load(path) {
                                Ok(scene) => {
                                    // Store model bounds so we can place it on the ground plane.
                                    {
                                        let mut objects = self.world.resource_mut::<SceneObjects>();
                                        objects.gltf_min_y = scene.bounds_min[1];
                                    }
                                    match GltfRenderer::new(&renderer, &scene) {
                                        Ok(gltf_renderer) => {
                                            println!("  ✓ glTF renderer created with textures");
                                            self.gltf_renderer = Some(gltf_renderer);
                                            break;
                                        }
                                        Err(e) => {
                                            eprintln!("  ✗ Failed to create glTF renderer: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("  ✗ Failed to load glTF: {}", e);
                                }
                            }
                            break;
                        }
                    }
                    
                    if self.gltf_renderer.is_none() {
                        println!("ℹ No glTF scene loaded. Place a model.gltf in the project root or models/ folder.");
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
        
        println!("\n🎮 Controls:");        println!("   WASD - Move camera");
        println!("   Q/E - Move up/down");
        println!("   Arrow Keys - Rotate camera");        println!("   ESC - Exit");
        println!("   F3 - Toggle UI");
        println!("   F11 - Toggle Fullscreen\n");
        
        // Request initial redraw
        window.request_redraw();
        
        self.window = Some(window);
    }
    
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Let egui handle events first. We still want to keep camera controls responsive,
        // so we only suppress *key presses* when egui actively wants keyboard input.
        let mut egui_consumed = false;
        let mut egui_wants_keyboard = false;
        if let (Some(egui), Some(window)) = (&mut self.egui_integration, &self.window) {
            egui_consumed = egui.handle_event(window, &event);
            egui_wants_keyboard = egui.ui_visible && egui.ctx.wants_keyboard_input();
        }

        // If egui consumed this event and it's not keyboard input, don't also handle it here.
        // (We still handle keyboard input so camera controls remain usable.)
        if egui_consumed {
            if !matches!(event, WindowEvent::KeyboardInput { .. }) {
                return;
            }
        }
        
        match event {
            WindowEvent::CloseRequested => {
                self.cleanup();
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(keycode) = event.physical_key {
                    if event.state.is_pressed() {
                        // Always allow app-level hotkeys, but avoid stealing input from egui
                        // when it is editing a text field.
                        let is_app_hotkey = matches!(keycode, KeyCode::Escape | KeyCode::F3 | KeyCode::F11);
                        if is_app_hotkey || !egui_wants_keyboard {
                            self.keys_pressed.insert(keycode);
                        }
                        
                        match keycode {
                            KeyCode::Escape => {
                                self.cleanup();
                                event_loop.exit();
                            }
                            KeyCode::F3 => {
                                if let Some(egui) = &mut self.egui_integration {
                                    egui.toggle_ui();
                                }
                            }
                            KeyCode::F11 => {
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
                    } else {
                        // Always remove on release to avoid stuck movement if egui consumed
                        // the press side of the event.
                        self.keys_pressed.remove(&keycode);
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll_amount = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y * 0.1,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => (pos.y as f32) * 0.01,
                };
                
                let mut camera = self.world.resource_mut::<CameraController>();
                camera.fov = (camera.fov - scroll_amount).clamp(10.0_f32.to_radians(), 120.0_f32.to_radians());
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

                // Drive continuous animation even while the window is being interacted with.
                // (Relying only on about_to_wait can stall during certain OS modal loops.)
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }

    }
    
    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // No-op: redraws are chained from RedrawRequested.
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
        
        // Update camera from input
        self.update_camera();
        
        let renderer = match &mut self.renderer {
            Some(r) => r,
            None => return,
        };
        
        let window_size = self.window.as_ref().map(|w| w.inner_size());
        let aspect_ratio = renderer.swapchain_extent.width as f32 / renderer.swapchain_extent.height as f32;
        
        unsafe {
            // Wait for previous frame with timeout to prevent indefinite blocking
            let timeout = 1_000_000_000; // 1 second in nanoseconds
            match renderer.device.wait_for_fences(
                &[renderer.in_flight_fences[renderer.current_frame]],
                true,
                timeout,
            ) {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("Fence wait timeout or error: {:?}", e);
                    return;
                }
            }
            
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
                        if let Err(e) = renderer.recreate_swapchain(size.width, size.height) {
                            eprintln!("Swapchain recreate failed: {:?}", e);
                            return;
                        }
                        // Also recreate gltf renderer's swapchain resources
                        if let Some(gltf) = &mut self.gltf_renderer {
                            if let Err(e) = gltf.recreate_swapchain_resources(renderer) {
                                eprintln!("glTF swapchain resource recreate failed: {}", e);
                                return;
                            }
                        }
                    }
                    return;
                }
                Err(e) => {
                    eprintln!("Failed to acquire image: {:?}", e);
                    return;
                }
            };
            
            // Wait for any previous frame that is using this swapchain image.
            // With IMMEDIATE present mode the swapchain can return the same image index again
            // before the GPU is finished with it.
            let image_fence = renderer.images_in_flight[image_index as usize];
            if image_fence != vk::Fence::null() {
                if let Err(e) = renderer
                    .device
                    .wait_for_fences(&[image_fence], true, timeout)
                {
                    eprintln!("Fence wait for image_in_flight failed: {:?}", e);
                    return;
                }
            }

            // Mark this image as being used by the current frame's fence
            renderer.images_in_flight[image_index as usize] = renderer.in_flight_fences[renderer.current_frame];
            
            renderer.device.reset_fences(
                &[renderer.in_flight_fences[renderer.current_frame]],
            ).unwrap();
            
            // Start command buffer
            let begin_info = vk::CommandBufferBeginInfo::default();
            renderer.device.begin_command_buffer(
                renderer.command_buffers[renderer.current_frame],
                &begin_info,
            ).unwrap();
            
            // Get camera controller
            let (camera_pos, camera_yaw, camera_pitch, camera_fov) = {
                let camera = self.world.resource::<CameraController>();
                (camera.position, camera.yaw, camera.pitch, camera.fov)
            };
            
            // Get object scales
            let (gltf_scale, gltf_min_y) = {
                let objects = self.world.resource::<SceneObjects>();
                (objects.gltf_scale, objects.gltf_min_y)
            };

            let shadow_settings = *self.world.resource::<ShadowSettings>();

            // Put the duck on the ground plane (Y=0). Account for user scale.
            let duck_pos = glam::Vec3::new(0.0, -gltf_min_y * gltf_scale, 0.0);
            let duck_pos = duck_pos + glam::Vec3::new(0.0, 0.001, 0.0);
            
            // Draw glTF model with its own pipeline and depth buffer
            if let Some(gltf_renderer) = &mut self.gltf_renderer {
                // Update uniform buffer
                if let Err(e) = gltf_renderer.update_uniform_buffer(
                    renderer.current_frame,
                    duck_pos,
                    camera_pos,
                    camera_yaw,
                    camera_pitch,
                    camera_fov,
                    gltf_scale,
                    aspect_ratio,
                    shadow_settings.debug_cascades,
                    shadow_settings.softness,
                ) {
                    eprintln!("Failed to update glTF uniform buffer: {}", e);
                }
                
                // Render glTF (this starts its own render pass with depth)
                gltf_renderer.render(
                    &renderer.device,
                    renderer.command_buffers[renderer.current_frame],
                    renderer.swapchain_extent,
                    image_index,
                    renderer.current_frame,
                );
                
                // End glTF render pass
                gltf_renderer.end_render_pass(&renderer.device, renderer.command_buffers[renderer.current_frame]);
            }
            
            // Render egui (in the old render pass for overlays)
            if let (Some(egui_int), Some(egui_vk), Some(window)) = 
                (&mut self.egui_integration, &mut self.egui_vulkan, &self.window) 
            {
                // Skip egui entirely if UI is hidden for max FPS
                if egui_int.ui_visible {
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
                    
                    let current_gltf_scale = {
                        let objects = self.world.resource::<SceneObjects>();
                        objects.gltf_scale
                    };

                    let shadow_settings = *self.world.resource::<ShadowSettings>();
                    
                    let ui_data = UiData {
                        fps,
                        frame_time_ms,
                        entity_count,
                        component_counts,
                        vulkan_version: renderer.vulkan_version.clone(),
                        gpu_name: renderer.gpu_name.clone(),
                        gltf_scale: current_gltf_scale,
                        shadow_debug_cascades: shadow_settings.debug_cascades,
                        shadow_softness: shadow_settings.softness,
                    };

                    let (full_output, ui_changes) = egui_int.build_ui(window, &ui_data);

                    if let Some(new_gltf_scale) = ui_changes.gltf_scale {
                        let mut objects = self.world.resource_mut::<SceneObjects>();
                        objects.gltf_scale = new_gltf_scale;
                    }

                    if ui_changes.shadow_settings_changed {
                        let mut s = self.world.resource_mut::<ShadowSettings>();
                        s.debug_cascades = ui_changes.shadow_debug_cascades;
                        s.softness = ui_changes.shadow_softness;
                    }

                    // Keep Vulkan font atlas in sync with egui
                    if !full_output.textures_delta.set.is_empty() {
                        // Wait for device idle before updating textures
                        let _ = renderer.device.device_wait_idle();
                    }
                    egui_vk.update_textures(
                        &renderer.device,
                        &renderer.instance,
                        renderer.physical_device,
                        renderer.graphics_queue,
                        renderer.graphics_queue_family_index,
                        &full_output.textures_delta,
                    );

                    let clipped_primitives = egui_int.ctx.tessellate(
                        full_output.shapes,
                        full_output.pixels_per_point,
                    );
                    
                    // Begin render pass for egui overlay
                    let clear_values = [vk::ClearValue {
                        color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 0.0] },
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
                    
                    egui_vk.render(
                        &renderer.device,
                        renderer.command_buffers[renderer.current_frame],
                        renderer.swapchain_extent.width,
                        renderer.swapchain_extent.height,
                        clipped_primitives,
                        full_output.pixels_per_point,
                    );
                    
                    renderer.device.cmd_end_render_pass(renderer.command_buffers[renderer.current_frame]);
                }
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
                    if let Err(e) = renderer.recreate_swapchain(size.width, size.height) {
                        eprintln!("Swapchain recreate failed: {:?}", e);
                        return;
                    }

                    // Recreate swapchain-dependent resources for custom renderers.
                    if let Some(gltf) = &mut self.gltf_renderer {
                        if let Err(e) = gltf.recreate_swapchain_resources(renderer) {
                            eprintln!("glTF swapchain resource recreate failed: {}", e);
                            return;
                        }
                    }
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
                
                if let Some(gltf_renderer) = &mut self.gltf_renderer {
                    gltf_renderer.cleanup(renderer);
                }
            }
        }
        
        println!("✓ Cleanup complete");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up panic hook to show stack trace
    std::panic::set_hook(Box::new(|panic_info| {
        eprintln!("PANIC: {}", panic_info);
        if let Some(location) = panic_info.location() {
            eprintln!("  at {}:{}:{}", location.file(), location.line(), location.column());
        }
    }));
    
    let event_loop = EventLoop::new()?;
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}
