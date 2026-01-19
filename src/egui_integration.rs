//! egui integration for Bevy ECS + Vulkan renderer
//! 
//! Provides debug UI showing ECS stats and performance metrics.

use egui::Context;
use egui_winit::State as EguiWinitState;
use winit::window::Window;

/// egui integration manager
pub struct EguiIntegration {
    pub ctx: Context,
    pub state: EguiWinitState,
    pub ui_visible: bool,
}

impl EguiIntegration {
    pub fn new(window: &Window) -> Self {
        let ctx = Context::default();
        let state = egui_winit::State::new(
            ctx.clone(),
            egui::viewport::ViewportId::ROOT,
            window,
            None,
            None,
            None,
        );
        
        // Do a dummy run to initialize fonts
        let _ = ctx.run(egui::RawInput::default(), |_| {});
        
        Self {
            ctx,
            state,
            ui_visible: true,
        }
    }
    
    pub fn handle_event(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool {
        self.state.on_window_event(window, event).consumed
    }
    
    pub fn toggle_ui(&mut self) {
        self.ui_visible = !self.ui_visible;
    }
    
    /// Build the UI and return FullOutput and optional scale changes
    pub fn build_ui(&mut self, window: &Window, ui_data: &UiData) -> (egui::FullOutput, Option<(f32, f32)>) {
        let raw_input = self.state.take_egui_input(window);
        
        let mut scale_changed = None;
        
        let output = self.ctx.run(raw_input, |ctx| {
            if self.ui_visible {
                scale_changed = render_debug_ui(ctx, ui_data);
            }
        });
        
        (output, scale_changed)
    }
}

/// Data to display in UI
pub struct UiData {
    pub fps: f64,
    pub frame_time_ms: f64,
    pub entity_count: usize,
    pub component_counts: ComponentCounts,
    pub vulkan_version: String,
    pub gpu_name: String,
    pub gltf_scale: f32,
}

pub struct ComponentCounts {
    pub transforms: usize,
    pub velocities: usize,
    pub cameras: usize,
    pub renderables: usize,
}

fn render_debug_ui(ctx: &egui::Context, data: &UiData) -> Option<(f32, f32)> {
    let mut scale_changed = None;
    
    egui::Window::new("🎮 Funky Renderer Debug")
        .default_pos([10.0, 10.0])
        .default_width(300.0)
        .show(ctx, |ui| {
            ui.heading("Performance");
            ui.separator();
            
            ui.horizontal(|ui| {
                ui.label("FPS:");
                ui.colored_label(egui::Color32::GREEN, format!("{:.1}", data.fps));
            });
            
            ui.horizontal(|ui| {
                ui.label("Frame Time:");
                ui.colored_label(egui::Color32::LIGHT_BLUE, format!("{:.2} ms", data.frame_time_ms));
            });
            
            ui.add_space(10.0);
            ui.heading("Scene Objects");
            ui.separator();
            
            let mut gltf_scale = data.gltf_scale;
            
            ui.label("Duck Scale:");
            if ui.add(egui::Slider::new(&mut gltf_scale, 0.001..=0.5).text("scale").logarithmic(true)).changed() {
                scale_changed = Some((1.0, gltf_scale));
            }
            
            ui.add_space(10.0);
            ui.heading("Bevy ECS Stats");
            ui.separator();
            
            ui.horizontal(|ui| {
                ui.label("FPS:");
                ui.colored_label(egui::Color32::GREEN, format!("{:.1}", data.fps));
            });
            
            ui.horizontal(|ui| {
                ui.label("Frame Time:");
                ui.colored_label(egui::Color32::LIGHT_BLUE, format!("{:.2} ms", data.frame_time_ms));
            });
            
            ui.add_space(10.0);
            ui.heading("Bevy ECS Stats");
            ui.separator();
            
            ui.label(format!("Total Entities: {}", data.entity_count));
            
            ui.add_space(5.0);
            ui.label("Components:");
            ui.indent("components", |ui| {
                ui.label(format!("• Transforms: {}", data.component_counts.transforms));
                ui.label(format!("• Velocities: {}", data.component_counts.velocities));
                ui.label(format!("• Cameras: {}", data.component_counts.cameras));
                ui.label(format!("• Renderables: {}", data.component_counts.renderables));
            });
            
            ui.add_space(10.0);
            ui.heading("Vulkan Info");
            ui.separator();
            ui.label(format!("GPU: {}", data.gpu_name));
            ui.label(format!("Vulkan: {}", data.vulkan_version));
            
            ui.add_space(10.0);
            ui.label("🦀 Rust + Bevy ECS + ash (Vulkan)");
            ui.small("Press F3 to toggle UI");
        });
    
    scale_changed
}
