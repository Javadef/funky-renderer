# Funky Renderer - Custom Vulkan Integration Plan

## Goal
Replace Bevy's wgpu renderer with our custom ash/Vulkan renderer while keeping a full-featured debug UI.

---

## Current State

| Version | Renderer | FPS | Debug UI |
|---------|----------|-----|----------|
| Standalone (`main.rs`) | Custom Vulkan (ash) | ~5000-6000 | ✅ egui (direct Vulkan) |
| Bevy (`bevy_main.rs`) | wgpu (Bevy's renderer) | ~1200-1600 | ✅ bevy_egui |

**Problem**: Bevy version uses wgpu which adds overhead and limits FPS.

---

## Options

### Option A: Enhanced Standalone (Recommended) ⭐
Keep the standalone Vulkan renderer, enhance it with ECS-like architecture manually.

**Pros:**
- Maximum performance (5000-6000+ FPS)
- Full control over rendering
- Already working!

**Cons:**
- No Bevy ecosystem (plugins, assets, etc.)
- Manual ECS implementation needed

### Option B: Bevy + Custom Vulkan Backend
Create a custom Bevy render backend using ash.

**Pros:**
- Keep Bevy's ECS, plugins, asset system
- Custom Vulkan rendering

**Cons:**
- Extremely complex (6-12 months of work)
- Fighting against Bevy's architecture
- Maintenance burden with Bevy updates

### Option C: Bevy Headless + Custom Renderer
Use Bevy for ECS only, disable its renderer, use your own.

**Pros:**
- Bevy's ECS benefits
- Your Vulkan renderer

**Cons:**
- Awkward integration
- Still some Bevy overhead

---

## Recommended Plan: Option A (Enhanced Standalone)

### Phase 1: Clean Up Project Structure ✅ (Done)
- [x] Standalone Vulkan renderer working
- [x] egui debug UI integrated
- [x] Direct Vulkan egui rendering

### Phase 2: Remove Bevy Dependencies
1. Remove `bevy_main.rs`
2. Remove `bevy_plugin` feature from Cargo.toml
3. Remove bevy and bevy_egui dependencies
4. Clean up `debug_ui.rs` (Bevy-specific)

### Phase 3: Enhance Standalone Architecture
1. Add simple component system for game objects
2. Add transform hierarchy
3. Add input handling system
4. Add scene management

### Phase 4: Feature Parity
1. Multiple 3D objects support
2. Camera controls
3. Lighting system
4. Material system

### Phase 5: Optimize
1. Frustum culling
2. Instanced rendering
3. GPU-driven rendering
4. Compute shaders for physics

---

## Implementation Steps

### Step 1: Remove Bevy Files and Dependencies

**Files to delete:**
```
src/bevy_main.rs
src/debug_ui.rs (Bevy-specific, we have egui_renderer.rs now)
```

**Cargo.toml changes:**
- Remove `bevy_plugin` feature
- Remove `bevy` dependency
- Remove `bevy_egui` dependency
- Make `standalone` the only option

### Step 2: Update Cargo.toml

```toml
[package]
name = "funkyrenderer"
version = "0.1.0"
edition = "2021"

[dependencies]
# Vulkan
ash = { version = "0.38", features = ["linked"] }
ash-window = "0.13"
gpu-allocator = { version = "0.28", default-features = false, features = ["vulkan", "std"] }
winit = "0.30"
raw-window-handle = "0.6"
glam = "0.29"
parking_lot = "0.12"
rayon = "1.11"
num_cpus = "1.16"

# UI
egui = "0.29"
egui-winit = "0.29"

# System info
sysinfo = "0.33"

[[bin]]
name = "funkyrenderer"
path = "src/main.rs"
```

### Step 3: Create Simple ECS (Optional)

If you want ECS-like architecture without Bevy:

```rust
// src/ecs.rs - Simple component storage
use std::any::{Any, TypeId};
use std::collections::HashMap;

pub type Entity = u32;

pub struct World {
    entities: Vec<Entity>,
    next_entity: Entity,
    components: HashMap<TypeId, HashMap<Entity, Box<dyn Any>>>,
}

impl World {
    pub fn new() -> Self { ... }
    pub fn spawn(&mut self) -> Entity { ... }
    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T) { ... }
    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> { ... }
    pub fn query<T: 'static>(&self) -> impl Iterator<Item = (Entity, &T)> { ... }
}
```

### Step 4: Add Multiple Objects Support

Modify `cube.rs` to support multiple instances:

```rust
pub struct SceneRenderer {
    objects: Vec<RenderObject>,
    instance_buffer: vk::Buffer,
    // ...
}

pub struct RenderObject {
    pub mesh: MeshHandle,
    pub transform: Transform,
    pub material: MaterialHandle,
}
```

### Step 5: Add Camera System

```rust
pub struct Camera {
    pub position: Vec3,
    pub rotation: Quat,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    pub fn view_matrix(&self) -> Mat4 { ... }
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 { ... }
}
```

### Step 6: Input Handling

```rust
pub struct InputState {
    pub keys_pressed: HashSet<KeyCode>,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,
    pub mouse_buttons: HashSet<MouseButton>,
}
```

---

## File Structure After Cleanup

```
funkyrenderer/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point
│   ├── renderer.rs          # Vulkan renderer
│   ├── cube.rs              # Cube mesh (expand to scene renderer)
│   ├── egui_renderer.rs     # egui integration & debug UI
│   ├── egui_vulkan.rs       # Vulkan backend for egui
│   ├── multithreading.rs    # Thread pool (optional)
│   ├── camera.rs            # Camera system (new)
│   ├── input.rs             # Input handling (new)
│   └── ecs.rs               # Simple ECS (optional, new)
└── shaders/
    ├── shader.vert
    └── shader.frag
```

---

## Quick Start Commands

```bash
# After cleanup, build and run:
cargo build --release
cargo run --release

# Or with explicit binary:
cargo run --release --bin funkyrenderer
```

---

## Timeline Estimate

| Phase | Time | Status |
|-------|------|--------|
| Phase 1: Cleanup | 1 hour | Ready |
| Phase 2: Remove Bevy | 30 min | Ready |
| Phase 3: Architecture | 2-4 hours | Next |
| Phase 4: Features | 1-2 days | Later |
| Phase 5: Optimize | Ongoing | Later |

---

## Next Action

Run the cleanup to remove Bevy and simplify the project:

```bash
# The standalone version already works!
cargo run --release --features standalone --bin funkyrenderer
```

Do you want me to execute the cleanup now?
