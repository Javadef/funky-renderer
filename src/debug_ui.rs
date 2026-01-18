//! Debug UI with egui - Beautiful system stats display
//! 
//! Displays GPU, CPU, memory, driver info, and performance metrics

use bevy::prelude::*;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::render::renderer::RenderAdapterInfo;
use bevy_egui::{egui, EguiContexts, EguiPlugin};
use sysinfo::System;
use std::time::Instant;
use std::collections::VecDeque;
use ash::vk;

/// Plugin for the debug UI overlay
pub struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .add_plugins(FrameTimeDiagnosticsPlugin::default())
            .init_resource::<DebugUiState>()
            .init_resource::<SystemInfo>()
            .init_resource::<PerformanceHistory>()
            .add_systems(Startup, setup_system_info)
            .add_systems(Update, (
                update_performance_stats,
                render_debug_ui,
            ).chain());
    }
}

/// State for the debug UI
#[derive(Resource)]
pub struct DebugUiState {
    pub visible: bool,
    pub show_fps_graph: bool,
    pub show_system_info: bool,
    pub show_gpu_info: bool,
    pub show_memory_info: bool,
    pub compact_mode: bool,
    pub panel_alpha: f32,
}

impl Default for DebugUiState {
    fn default() -> Self {
        Self {
            visible: true,
            show_fps_graph: true,
            show_system_info: true,
            show_gpu_info: true,
            show_memory_info: true,
            compact_mode: false,
            panel_alpha: 0.92,
        }
    }
}

/// Cached system information
#[derive(Resource, Default)]
pub struct SystemInfo {
    pub cpu_name: String,
    pub cpu_cores: usize,
    pub cpu_threads: usize,
    pub total_memory_gb: f64,
    pub os_name: String,
    pub os_version: String,
    pub hostname: String,
    // GPU info from Bevy's render adapter
    pub gpu_name: String,
    pub gpu_vendor: String,
    pub gpu_driver: String,
    pub gpu_driver_version: String,
    pub gpu_backend: String,
    pub gpu_device_type: String,
    pub vulkan_api_version: String,
}

/// Performance history for graphs
#[derive(Resource)]
pub struct PerformanceHistory {
    pub fps_history: VecDeque<f64>,
    pub frame_time_history: VecDeque<f64>,
    pub cpu_usage_history: VecDeque<f32>,
    pub memory_usage_history: VecDeque<f64>,
    pub max_history: usize,
    pub last_update: Instant,
    pub update_interval_ms: u64,
    // Running stats
    pub current_fps: f64,
    pub avg_fps: f64,
    pub min_fps: f64,
    pub max_fps: f64,
    pub frame_time_ms: f64,
    pub total_frames: u64,
    pub session_start: Instant,
    // System monitor
    pub system: System,
    pub cpu_usage: f32,
    pub used_memory_gb: f64,
}

impl Default for PerformanceHistory {
    fn default() -> Self {
        Self {
            fps_history: VecDeque::with_capacity(120),
            frame_time_history: VecDeque::with_capacity(120),
            cpu_usage_history: VecDeque::with_capacity(60),
            memory_usage_history: VecDeque::with_capacity(60),
            max_history: 120,
            last_update: Instant::now(),
            update_interval_ms: 100,  // Update less frequently for perf
            current_fps: 0.0,
            avg_fps: 0.0,
            min_fps: f64::MAX,
            max_fps: 0.0,
            frame_time_ms: 0.0,
            total_frames: 0,
            session_start: Instant::now(),
            system: System::new_all(),
            cpu_usage: 0.0,
            used_memory_gb: 0.0,
        }
    }
}

/// Setup system information on startup
fn setup_system_info(
    mut sys_info: ResMut<SystemInfo>,
    render_adapter: Option<Res<RenderAdapterInfo>>,
) {
    let mut system = System::new_all();
    system.refresh_all();
    
    // CPU info
    sys_info.cpu_name = system.cpus().first()
        .map(|cpu| cpu.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());
    sys_info.cpu_cores = system.physical_core_count().unwrap_or(0);
    sys_info.cpu_threads = system.cpus().len();
    
    // Memory info
    sys_info.total_memory_gb = system.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
    
    // OS info
    sys_info.os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
    sys_info.os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
    sys_info.hostname = System::host_name().unwrap_or_else(|| "Unknown".to_string());
    
    // GPU info from Bevy's render adapter
    if let Some(adapter) = render_adapter {
        sys_info.gpu_name = adapter.name.clone();
        sys_info.gpu_vendor = format!("{}", adapter.vendor);
        sys_info.gpu_driver = adapter.driver.clone();
        sys_info.gpu_driver_version = adapter.driver_info.clone();
        sys_info.gpu_backend = format!("{:?}", adapter.backend);
        sys_info.gpu_device_type = format!("{:?}", adapter.device_type);
    }
    
    // Query Vulkan API version directly using ash
    sys_info.vulkan_api_version = query_vulkan_version();
    
    info!("📊 System Info Loaded:");
    info!("   CPU: {} ({} cores, {} threads)", sys_info.cpu_name, sys_info.cpu_cores, sys_info.cpu_threads);
    info!("   RAM: {:.1} GB", sys_info.total_memory_gb);
    info!("   GPU: {} ({})", sys_info.gpu_name, sys_info.gpu_driver_version);
    info!("   Vulkan: {}", sys_info.vulkan_api_version);
}

/// Query Vulkan API version using ash
fn query_vulkan_version() -> String {
    unsafe {
        let entry = match ash::Entry::linked() {
            entry => entry,
        };
        
        // Query instance version (highest supported by loader)
        let instance_version = match entry.try_enumerate_instance_version() {
            Ok(Some(version)) => version,
            Ok(None) => vk::API_VERSION_1_0,
            Err(_) => vk::API_VERSION_1_0,
        };
        
        let major = vk::api_version_major(instance_version);
        let minor = vk::api_version_minor(instance_version);
        let patch = vk::api_version_patch(instance_version);
        
        format!("{}.{}.{}", major, minor, patch)
    }
}

/// Update performance statistics
fn update_performance_stats(
    diagnostics: Res<DiagnosticsStore>,
    mut history: ResMut<PerformanceHistory>,
    render_adapter: Option<Res<RenderAdapterInfo>>,
    mut sys_info: ResMut<SystemInfo>,
) {
    // Update GPU info if not set
    if sys_info.gpu_name.is_empty() {
        if let Some(adapter) = render_adapter {
            sys_info.gpu_name = adapter.name.clone();
            sys_info.gpu_vendor = format!("{}", adapter.vendor);
            sys_info.gpu_driver = adapter.driver.clone();
            sys_info.gpu_driver_version = adapter.driver_info.clone();
            sys_info.gpu_backend = format!("{:?}", adapter.backend);
            sys_info.gpu_device_type = format!("{:?}", adapter.device_type);
        }
    }
    
    // Get FPS from diagnostics
    if let Some(fps) = diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(value) = fps.smoothed() {
            history.current_fps = value;
            history.total_frames += 1;
            
            // Calculate frame time from FPS (more reliable)
            if value > 0.0 {
                history.frame_time_ms = 1000.0 / value;
                
                // Update min/max (skip first 60 frames for warmup)
                if history.total_frames > 60 {
                    history.min_fps = history.min_fps.min(value);
                    history.max_fps = history.max_fps.max(value);
                }
            }
            
            // Calculate running average
            let elapsed = history.session_start.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                history.avg_fps = history.total_frames as f64 / elapsed;
            }
        }
    }
    
    // Update history at intervals
    if history.last_update.elapsed().as_millis() >= history.update_interval_ms as u128 {
        history.last_update = Instant::now();
        
        // Copy values first to avoid borrow issues
        let current_fps = history.current_fps;
        let frame_time_ms = history.frame_time_ms;
        let max_history = history.max_history;
        
        // Add to history
        if history.fps_history.len() >= max_history {
            history.fps_history.pop_front();
        }
        history.fps_history.push_back(current_fps);
        
        if history.frame_time_history.len() >= max_history {
            history.frame_time_history.pop_front();
        }
        history.frame_time_history.push_back(frame_time_ms);
        
        // Update system stats less frequently
        history.system.refresh_cpu_usage();
        history.system.refresh_memory();
        
        // CPU usage
        let total_cpu: f32 = history.system.cpus().iter()
            .map(|cpu| cpu.cpu_usage())
            .sum::<f32>() / history.system.cpus().len() as f32;
        history.cpu_usage = total_cpu;
        
        let cpu_usage = history.cpu_usage;
        if history.cpu_usage_history.len() >= max_history {
            history.cpu_usage_history.pop_front();
        }
        history.cpu_usage_history.push_back(cpu_usage);
        
        // Memory usage
        history.used_memory_gb = history.system.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        
        let used_memory_gb = history.used_memory_gb;
        if history.memory_usage_history.len() >= max_history {
            history.memory_usage_history.pop_front();
        }
        history.memory_usage_history.push_back(used_memory_gb);
    }
}

/// Render the debug UI
fn render_debug_ui(
    mut contexts: EguiContexts,
    mut ui_state: ResMut<DebugUiState>,
    sys_info: Res<SystemInfo>,
    history: Res<PerformanceHistory>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    // Toggle UI with F3
    if keys.just_pressed(KeyCode::F3) {
        ui_state.visible = !ui_state.visible;
    }
    
    // Toggle compact mode with F4
    if keys.just_pressed(KeyCode::F4) {
        ui_state.compact_mode = !ui_state.compact_mode;
    }
    
    if !ui_state.visible {
        return;
    }
    
    let ctx = contexts.ctx_mut();
    
    // Configure style
    let mut style = (*ctx.style()).clone();
    style.visuals.window_fill = egui::Color32::from_rgba_unmultiplied(20, 25, 35, (ui_state.panel_alpha * 255.0) as u8);
    style.visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 80, 120));
    style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(35, 45, 60);
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(45, 55, 75);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 70, 95);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(70, 100, 140);
    ctx.set_style(style);
    
    if ui_state.compact_mode {
        render_compact_ui(ctx, &history, &sys_info);
    } else {
        render_full_ui(ctx, &mut ui_state, &history, &sys_info);
    }
}

/// Render compact FPS overlay
fn render_compact_ui(
    ctx: &egui::Context,
    history: &PerformanceHistory,
    sys_info: &SystemInfo,
) {
    egui::Area::new(egui::Id::new("compact_fps"))
        .fixed_pos(egui::pos2(10.0, 10.0))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(20, 25, 35, 230))
                .rounding(egui::Rounding::same(8.0))
                .inner_margin(egui::Margin::same(12.0))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 80, 120)))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // FPS with color coding
                        let fps_color = fps_color(history.current_fps);
                        ui.colored_label(fps_color, 
                            egui::RichText::new(format!("{:.0} FPS", history.current_fps))
                                .size(18.0)
                                .strong());
                        ui.separator();
                        ui.label(egui::RichText::new(format!("{:.2}ms", history.frame_time_ms))
                            .size(14.0)
                            .color(egui::Color32::from_rgb(180, 180, 200)));
                        ui.separator();
                        ui.label(egui::RichText::new(&sys_info.gpu_name)
                            .size(12.0)
                            .color(egui::Color32::from_rgb(120, 140, 180)));
                    });
                });
        });
}

/// Render full debug UI
fn render_full_ui(
    ctx: &egui::Context,
    ui_state: &mut DebugUiState,
    history: &PerformanceHistory,
    sys_info: &SystemInfo,
) {
    egui::Window::new(">> Funky Renderer Debug")
        .default_pos(egui::pos2(10.0, 10.0))
        .default_width(380.0)
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui| {
            // Header with main FPS
            ui.horizontal(|ui| {
                let fps_color = fps_color(history.current_fps);
                ui.colored_label(fps_color,
                    egui::RichText::new(format!("{:.0}", history.current_fps))
                        .size(42.0)
                        .strong());
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("FPS").size(16.0).color(egui::Color32::from_rgb(150, 150, 170)));
                    ui.label(egui::RichText::new(format!("{:.2} ms", history.frame_time_ms))
                        .size(14.0)
                        .color(egui::Color32::from_rgb(180, 180, 200)));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("F3: Toggle | F4: Compact")
                        .size(10.0)
                        .color(egui::Color32::from_rgb(100, 100, 120)));
                });
            });
            
            ui.separator();
            
            // Performance section
            ui.collapsing(egui::RichText::new("[+] Performance").size(14.0).strong(), |ui| {
                ui.horizontal(|ui| {
                    stat_box(ui, "AVG", format!("{:.1}", history.avg_fps), egui::Color32::from_rgb(100, 180, 255));
                    stat_box(ui, "MIN", format!("{:.1}", if history.min_fps == f64::MAX { 0.0 } else { history.min_fps }), egui::Color32::from_rgb(255, 130, 100));
                    stat_box(ui, "MAX", format!("{:.1}", history.max_fps), egui::Color32::from_rgb(130, 255, 130));
                });
                
                ui.add_space(8.0);
                
                // FPS Graph
                if ui_state.show_fps_graph && !history.fps_history.is_empty() {
                    ui.label(egui::RichText::new("FPS History").size(11.0).color(egui::Color32::from_rgb(140, 140, 160)));
                    let fps_data: Vec<f64> = history.fps_history.iter().copied().collect();
                    plot_graph(ui, &fps_data, egui::Color32::from_rgb(100, 200, 255), 60.0);
                    
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("Frame Time (ms)").size(11.0).color(egui::Color32::from_rgb(140, 140, 160)));
                    let ft_data: Vec<f64> = history.frame_time_history.iter().copied().collect();
                    plot_graph(ui, &ft_data, egui::Color32::from_rgb(255, 180, 100), 60.0);
                }
                
                ui.add_space(4.0);
                let elapsed = history.session_start.elapsed();
                ui.label(egui::RichText::new(format!("Session: {:02}:{:02}:{:02} | Frames: {}", 
                    elapsed.as_secs() / 3600,
                    (elapsed.as_secs() % 3600) / 60,
                    elapsed.as_secs() % 60,
                    history.total_frames))
                    .size(11.0)
                    .color(egui::Color32::from_rgb(120, 120, 140)));
            });
            
            // GPU section
            if ui_state.show_gpu_info {
                ui.collapsing(egui::RichText::new("[G] GPU Info").size(14.0).strong(), |ui| {
                    info_row(ui, "Device", &sys_info.gpu_name);
                    info_row(ui, "Type", &sys_info.gpu_device_type);
                    info_row(ui, "Vendor ID", &sys_info.gpu_vendor);
                    info_row(ui, "Driver", &sys_info.gpu_driver);
                    info_row(ui, "Driver Ver", &sys_info.gpu_driver_version);
                    info_row(ui, "Vulkan", &format!("{} ({})", sys_info.vulkan_api_version, sys_info.gpu_backend));
                });
            }
            
            // System section
            if ui_state.show_system_info {
                ui.collapsing(egui::RichText::new("[S] System Info").size(14.0).strong(), |ui| {
                    info_row(ui, "CPU", &sys_info.cpu_name);
                    info_row(ui, "Cores", &format!("{} physical, {} logical", sys_info.cpu_cores, sys_info.cpu_threads));
                    info_row(ui, "OS", &format!("{} {}", sys_info.os_name, sys_info.os_version));
                    info_row(ui, "Host", &sys_info.hostname);
                });
            }
            
            // Memory section  
            if ui_state.show_memory_info {
                ui.collapsing(egui::RichText::new("[R] Resources").size(14.0).strong(), |ui| {
                    // CPU usage bar
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("CPU:").size(12.0));
                        let cpu_pct = history.cpu_usage / 100.0;
                        let cpu_color = if cpu_pct > 0.8 { 
                            egui::Color32::from_rgb(255, 100, 100) 
                        } else if cpu_pct > 0.5 { 
                            egui::Color32::from_rgb(255, 200, 100) 
                        } else { 
                            egui::Color32::from_rgb(100, 200, 255) 
                        };
                        ui.add(egui::ProgressBar::new(cpu_pct)
                            .fill(cpu_color)
                            .text(format!("{:.1}%", history.cpu_usage)));
                    });
                    
                    // Memory usage bar
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("RAM:").size(12.0));
                        let mem_pct = (history.used_memory_gb / sys_info.total_memory_gb) as f32;
                        let mem_color = if mem_pct > 0.8 { 
                            egui::Color32::from_rgb(255, 100, 100) 
                        } else if mem_pct > 0.6 { 
                            egui::Color32::from_rgb(255, 200, 100) 
                        } else { 
                            egui::Color32::from_rgb(100, 255, 150) 
                        };
                        ui.add(egui::ProgressBar::new(mem_pct)
                            .fill(mem_color)
                            .text(format!("{:.1} / {:.1} GB", history.used_memory_gb, sys_info.total_memory_gb)));
                    });
                    
                    // Resource graphs
                    if !history.cpu_usage_history.is_empty() {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("CPU History").size(10.0).color(egui::Color32::from_rgb(120, 120, 140)));
                        let cpu_data: Vec<f64> = history.cpu_usage_history.iter().map(|&x| x as f64).collect();
                        plot_graph(ui, &cpu_data, egui::Color32::from_rgb(100, 200, 255), 40.0);
                    }
                });
            }
            
            ui.separator();
            
            // Settings
            ui.collapsing(egui::RichText::new("[=] Settings").size(14.0), |ui| {
                ui.checkbox(&mut ui_state.show_fps_graph, "Show FPS Graph");
                ui.checkbox(&mut ui_state.show_gpu_info, "Show GPU Info");
                ui.checkbox(&mut ui_state.show_system_info, "Show System Info");
                ui.checkbox(&mut ui_state.show_memory_info, "Show Resources");
                ui.add(egui::Slider::new(&mut ui_state.panel_alpha, 0.5..=1.0).text("Panel Opacity"));
            });
        });
}

/// Helper to draw a stat box
fn stat_box(ui: &mut egui::Ui, label: &str, value: String, color: egui::Color32) {
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(30, 40, 55))
        .rounding(egui::Rounding::same(6.0))
        .inner_margin(egui::Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(label).size(10.0).color(egui::Color32::from_rgb(140, 140, 160)));
                ui.label(egui::RichText::new(value).size(16.0).strong().color(color));
            });
        });
}

/// Helper to draw info row
fn info_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{}:", label)).size(12.0).color(egui::Color32::from_rgb(140, 140, 160)));
        ui.label(egui::RichText::new(value).size(12.0).color(egui::Color32::from_rgb(200, 200, 220)));
    });
}

/// Helper to plot a simple line graph
fn plot_graph(ui: &mut egui::Ui, data: &[f64], color: egui::Color32, height: f32) {
    if data.len() < 2 {
        return;
    }
    
    let (response, painter) = ui.allocate_painter(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    
    let rect = response.rect;
    let padding = 2.0;
    let graph_height = rect.height() - padding * 2.0;
    
    // Background
    painter.rect_filled(rect, egui::Rounding::same(4.0), egui::Color32::from_rgb(25, 30, 40));
    
    // Find min/max for scaling with some padding
    let min_val = data.iter().copied().fold(f64::INFINITY, f64::min);
    let max_val = data.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = (max_val - min_val).max(0.1);
    
    // Add 10% padding to range
    let padded_min = min_val - range * 0.05;
    let padded_range = range * 1.1;
    
    // Calculate points
    let point_count = data.len();
    let points: Vec<egui::Pos2> = data.iter().enumerate().map(|(i, &val)| {
        let x = rect.left() + padding + (i as f32 / (point_count - 1).max(1) as f32) * (rect.width() - padding * 2.0);
        let normalized = ((val - padded_min) / padded_range) as f32;
        let y = rect.bottom() - padding - normalized * graph_height;
        egui::pos2(x, y.clamp(rect.top() + padding, rect.bottom() - padding))
    }).collect();
    
    // Draw gradient fill using vertical lines (more reliable than polygon)
    let fill_color = color.gamma_multiply(0.15);
    for i in 0..points.len().saturating_sub(1) {
        let p1 = points[i];
        let p2 = points[i + 1];
        
        // Draw a quad as two triangles for the fill
        let bottom_y = rect.bottom() - padding;
        painter.add(egui::Shape::convex_polygon(
            vec![p1, p2, egui::pos2(p2.x, bottom_y), egui::pos2(p1.x, bottom_y)],
            fill_color,
            egui::Stroke::NONE,
        ));
    }
    
    // Draw the line on top
    painter.add(egui::Shape::line(points, egui::Stroke::new(2.0, color)));
    
    // Draw horizontal grid lines
    let grid_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 15);
    for i in 1..4 {
        let y = rect.top() + padding + (i as f32 / 4.0) * graph_height;
        painter.line_segment(
            [egui::pos2(rect.left() + padding, y), egui::pos2(rect.right() - padding, y)],
            egui::Stroke::new(1.0, grid_color),
        );
    }
    
    // Draw current value with background
    if let Some(&last) = data.last() {
        let text = format!("{:.1}", last);
        let text_pos = egui::pos2(rect.right() - 6.0, rect.top() + 4.0);
        
        // Background for text
        let text_galley = painter.layout_no_wrap(text.clone(), egui::FontId::proportional(11.0), color);
        let text_rect = egui::Rect::from_min_size(
            egui::pos2(text_pos.x - text_galley.size().x - 4.0, text_pos.y - 1.0),
            egui::vec2(text_galley.size().x + 6.0, text_galley.size().y + 2.0),
        );
        painter.rect_filled(text_rect, egui::Rounding::same(3.0), egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180));
        
        painter.text(
            text_pos,
            egui::Align2::RIGHT_TOP,
            text,
            egui::FontId::proportional(11.0),
            color,
        );
    }
}

/// Get color based on FPS value
fn fps_color(fps: f64) -> egui::Color32 {
    if fps >= 120.0 {
        egui::Color32::from_rgb(100, 255, 150)  // Green - Excellent
    } else if fps >= 60.0 {
        egui::Color32::from_rgb(150, 255, 100)  // Light green - Good
    } else if fps >= 30.0 {
        egui::Color32::from_rgb(255, 220, 100)  // Yellow - Acceptable
    } else {
        egui::Color32::from_rgb(255, 100, 100)  // Red - Poor
    }
}
