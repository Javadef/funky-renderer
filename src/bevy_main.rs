//! Bevy integration example for Funky Renderer
//! 
//! Run with: cargo run --bin funkyrenderer_bevy --features bevy_plugin --no-default-features

mod renderer;
mod cube;
mod multithreading;
mod bevy_plugin;
mod debug_ui;

use bevy::prelude::*;
use bevy::pbr::StandardMaterial;
use bevy::window::PresentMode;
use bevy_plugin::{FunkyRendererPlugin, FunkyCube};
use debug_ui::DebugUiPlugin;

fn main() {
    println!("🚀 Funky Renderer - Bevy Integration");
    println!("════════════════════════════════════════════");
    
    App::new()
        // Core Bevy plugins with vsync disabled for max FPS
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Funky Renderer (Bevy) | Initializing...".into(),
                resolution: (1280., 720.).into(),
                resizable: true,
                present_mode: PresentMode::AutoNoVsync, // Disable vsync for max FPS!
                ..default()
            }),
            ..default()
        }))
        // Debug UI with egui
        .add_plugins(DebugUiPlugin)
        // Our custom Vulkan renderer plugin
        .add_plugins(FunkyRendererPlugin)
        // Setup systems
        .add_systems(Startup, setup_scene)
        .add_systems(Update, (
            handle_input,
            rotate_bevy_cubes,
            log_cube_state,
        ))
        .run();
}

/// Setup the 3D scene with Bevy's built-in rendering
fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    info!("Setting up Bevy scene...");
    
    // Single cube - minimal scene for max FPS
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.9, 0.9), // Light cyan
            unlit: true,  // Skip lighting calculations for speed
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        RotatingCube { speed: 1.0 },
        Name::new("Bevy Cube"),
    ));
    
    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(3.0, 3.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    
    // Cornflower blue background (matching our Vulkan renderer)
    commands.insert_resource(ClearColor(Color::srgb(0.39, 0.58, 0.93)));
    
    info!("✓ Scene setup complete");
    info!("\n🎮 Controls:");
    info!("   ESC - Exit");
    info!("   SPACE - Toggle cube rotation");
    info!("   R - Reset rotation");
    info!("   F3 - Toggle debug UI");
    info!("   F4 - Compact mode");
}

/// Component for Bevy's built-in rotating cube
#[derive(Component)]
struct RotatingCube {
    speed: f32,
}

/// Component to track if rotation is paused
#[derive(Resource, Default)]
struct RotationPaused(bool);

/// System to handle keyboard input
fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut rotation_paused: Local<bool>,
    mut cubes: Query<&mut FunkyCube>,
    mut bevy_cubes: Query<&mut RotatingCube>,
) {
    if keys.just_pressed(KeyCode::Space) {
        *rotation_paused = !*rotation_paused;
        
        // Update Funky cubes
        for mut cube in cubes.iter_mut() {
            cube.rotation_speed = if *rotation_paused { 0.0 } else { 1.0 };
        }
        
        // Update Bevy cubes
        for mut cube in bevy_cubes.iter_mut() {
            cube.speed = if *rotation_paused { 0.0 } else { 1.0 };
        }
        
        info!("Rotation {}", if *rotation_paused { "paused" } else { "resumed" });
    }
    
    if keys.just_pressed(KeyCode::KeyR) {
        for mut cube in cubes.iter_mut() {
            cube.current_rotation = 0.0;
        }
        info!("Rotation reset");
    }
}

/// System to rotate Bevy cubes
fn rotate_bevy_cubes(
    time: Res<Time>,
    mut cubes: Query<(&RotatingCube, &mut Transform)>,
) {
    for (cube, mut transform) in cubes.iter_mut() {
        transform.rotate_y(cube.speed * time.delta_secs());
        transform.rotate_x(cube.speed * 0.5 * time.delta_secs());
    }
}

/// System to log cube state periodically (for debugging)
fn log_cube_state(
    time: Res<Time>,
    cubes: Query<(&FunkyCube, &Name)>,
    mut last_log: Local<f32>,
) {
    *last_log += time.delta_secs();
    if *last_log > 5.0 {
        for (cube, name) in cubes.iter() {
            debug!(
                "{}: rotation = {:.2} rad, speed = {:.1}",
                name, cube.current_rotation, cube.rotation_speed
            );
        }
        *last_log = 0.0;
    }
}
