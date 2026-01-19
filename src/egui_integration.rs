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
    
    /// Build the UI and return FullOutput and optional changes
    pub fn build_ui(&mut self, window: &Window, ui_data: &UiData) -> (egui::FullOutput, UiChanges) {
        let raw_input = self.state.take_egui_input(window);
        
        let mut changes = UiChanges::default();
        
        let output = self.ctx.run(raw_input, |ctx| {
            if self.ui_visible {
                changes = render_debug_ui(ctx, ui_data);
            }
        });
        
        (output, changes)
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

    // Shadows
    pub shadow_debug_cascades: bool,
    pub shadow_softness: f32,
    pub shadow_use_pcss: bool,
    pub shadow_use_taa: bool,
}

#[derive(Default, Clone, Copy)]
pub struct UiChanges {
    pub gltf_scale: Option<f32>,

    pub shadow_settings_changed: bool,
    pub shadow_debug_cascades: bool,
    pub shadow_softness: f32,
    pub shadow_use_pcss: bool,
    pub shadow_use_taa: bool,
}

pub struct ComponentCounts {
    pub transforms: usize,
    pub velocities: usize,
    pub cameras: usize,
    pub renderables: usize,
}

fn render_debug_ui(ctx: &egui::Context, data: &UiData) -> UiChanges {
    let mut changes = UiChanges {
        gltf_scale: None,

        shadow_settings_changed: false,
        shadow_debug_cascades: data.shadow_debug_cascades,
        shadow_softness: data.shadow_softness,
        shadow_use_pcss: data.shadow_use_pcss,
        shadow_use_taa: data.shadow_use_taa,
    };
    
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
                changes.gltf_scale = Some(gltf_scale);
            }

            ui.add_space(10.0);
            ui.heading("Shadows");
            ui.separator();

            let mut debug_cascades = data.shadow_debug_cascades;
            if ui.checkbox(&mut debug_cascades, "Debug cascades").changed() {
                changes.shadow_settings_changed = true;
                changes.shadow_debug_cascades = debug_cascades;
            }

            let mut use_pcss = data.shadow_use_pcss;
            if ui.checkbox(&mut use_pcss, "PCSS (contact hardening)").changed() {
                changes.shadow_settings_changed = true;
                changes.shadow_use_pcss = use_pcss;
            }
            ui.small("Tiny Glade style: soft near, sharp at contact");

            let mut use_taa = data.shadow_use_taa;
            if ui.checkbox(&mut use_taa, "Shadow TAA (stabilize penumbra)").changed() {
                changes.shadow_settings_changed = true;
                changes.shadow_use_taa = use_taa;
            }
            ui.small("Temporal filter with variance clamp; reduces crawl");

            let mut softness = data.shadow_softness;
            if ui
                .add(egui::Slider::new(&mut softness, 0.5..=8.0).text("Light size (texels)"))
                .changed()
            {
                changes.shadow_settings_changed = true;
                changes.shadow_softness = softness;
            }
            ui.small("Controls penumbra width");
            
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

    changes
}
