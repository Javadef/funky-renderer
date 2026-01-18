//! Funky Renderer - Standalone Vulkan Application
//! 
//! Run with: cargo run --release

mod renderer;
mod cube;
mod multithreading;

use renderer::VulkanRenderer;
use cube::CubeRenderer;
use multithreading::MultiThreadedRenderer;
use ash::vk;
use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

struct App {
    window: Option<Window>,
    renderer: Option<VulkanRenderer>,
    cube_renderer: Option<CubeRenderer>,
    mt_renderer: Option<MultiThreadedRenderer>,
    start_time: Instant,
    frame_count: u64,
    last_fps_time: Instant,
    last_frame_time: Instant,
    fps: f64,
    frame_time_ms: f64,
    minimized: bool,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            renderer: None,
            cube_renderer: None,
            mt_renderer: None,
            start_time: Instant::now(),
            frame_count: 0,
            last_fps_time: Instant::now(),
            last_frame_time: Instant::now(),
            fps: 0.0,
            frame_time_ms: 0.0,
            minimized: false,
        }
    }
    
    fn update_window_title(&self) {
        if let Some(window) = &self.window {
            let title = format!(
                "Funky Renderer | FPS: {:.1} | Frame: {:.2}ms | Vulkan + Multi-threading",
                self.fps,
                self.frame_time_ms
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
        
        println!("🚀 Funky Vulkan Renderer - Multi-threaded");
        println!("════════════════════════════════════════════");
        
        // Create window with resizable
        let window_attributes = Window::default_attributes()
            .with_title("Funky Renderer | Initializing...")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
            .with_resizable(true);
        
        let window = event_loop.create_window(window_attributes).unwrap();
        
        // Initialize Vulkan
        unsafe {
            match VulkanRenderer::new(&window) {
                Ok(renderer) => {
                    println!("✓ Vulkan renderer initialized");
                    println!("  Resolution: {}x{}", 
                        renderer.swapchain_extent.width, 
                        renderer.swapchain_extent.height);
                    
                    // Initialize multi-threading
                    match MultiThreadedRenderer::new(
                        &renderer.device,
                        renderer.graphics_queue_family_index,
                    ) {
                        Ok(mt) => {
                            println!("✓ Multi-threading system ready");
                            self.mt_renderer = Some(mt);
                        }
                        Err(e) => {
                            eprintln!("⚠ Multi-threading init failed: {:?}", e);
                        }
                    }
                    
                    // Initialize cube
                    match CubeRenderer::new(&renderer) {
                        Ok(cube) => {
                            println!("✓ Cube geometry created");
                            self.cube_renderer = Some(cube);
                        }
                        Err(e) => {
                            eprintln!("✗ Failed to create cube: {}", e);
                        }
                    }
                    
                    self.renderer = Some(renderer);
                }
                Err(e) => {
                    eprintln!("✗ Failed to initialize Vulkan: {}", e);
                    event_loop.exit();
                    return;
                }
            }
        }
        
        println!("\n🎮 Controls:");
        println!("   ESC - Exit");
        println!("   F11 - Toggle Fullscreen");
        println!("   Resize window freely!\n");
        
        self.window = Some(window);
    }
    
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
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
        let frame_start = Instant::now();
        
        let renderer = match &mut self.renderer {
            Some(r) => r,
            None => return,
        };
        
        let cube = match &mut self.cube_renderer {
            Some(c) => c,
            None => return,
        };
        
        // Get current window size for potential resize
        let window_size = self.window.as_ref().map(|w| w.inner_size());
        
        unsafe {
            // Wait for previous frame
            renderer.device.wait_for_fences(
                &[renderer.in_flight_fences[renderer.current_frame]],
                true,
                u64::MAX,
            ).unwrap();
            
            // Acquire next image
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
            
            // Reset fence
            renderer.device.reset_fences(
                &[renderer.in_flight_fences[renderer.current_frame]],
            ).unwrap();
            
            // Update uniform buffer with rotation
            let elapsed = self.start_time.elapsed().as_secs_f32();
            let rotation = elapsed * 1.0; // 1 radian per second
            
            if let Err(e) = cube.update_uniform_buffer(renderer, renderer.current_frame, rotation) {
                eprintln!("Failed to update uniform buffer: {}", e);
                return;
            }
            
            // Record command buffer
            if let Err(e) = cube.record_commands(
                renderer,
                renderer.command_buffers[renderer.current_frame],
                renderer.framebuffers[image_index as usize],
                renderer.current_frame,
            ) {
                eprintln!("Failed to record commands: {:?}", e);
                return;
            }
            
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
            
            // Update frame counter
            renderer.current_frame = (renderer.current_frame + 1) % renderer::MAX_FRAMES_IN_FLIGHT;
        }
        
        // Calculate frame time
        self.frame_time_ms = frame_start.elapsed().as_secs_f64() * 1000.0;
        self.frame_count += 1;
        
        // Update FPS counter every 100ms for smoother display
        let fps_elapsed = self.last_fps_time.elapsed();
        if fps_elapsed.as_millis() >= 100 {
            self.fps = self.frame_count as f64 / fps_elapsed.as_secs_f64();
            self.frame_count = 0;
            self.last_fps_time = Instant::now();
            self.update_window_title();
        }
        
        self.last_frame_time = Instant::now();
    }
    
    fn cleanup(&mut self) {
        println!("\n👋 Shutting down...");
        
        if let Some(renderer) = &self.renderer {
            unsafe {
                renderer.device.device_wait_idle().unwrap();
                
                if let Some(mt) = &self.mt_renderer {
                    mt.cleanup(&renderer.device);
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
