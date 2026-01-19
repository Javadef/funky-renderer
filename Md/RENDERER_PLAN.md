# Stylized Renderer - Technical Implementation Plan

## 🎯 Performance Targets

| Metric | Target | Tiny Glade Reference |
|--------|--------|---------------------|
| FPS | 60+ stable | 60+ on GTX 1060 |
| Draw Calls | < 100 per frame | ~50-100 |
| VRAM Usage | < 500MB | ~400MB |
| Disk Size | < 100MB total | ~80MB |
| Load Time | < 3 seconds | ~2 seconds |

## 🏗️ Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        FRAME PIPELINE                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐        │
│  │ Cull &  │──▶│ Depth   │──▶│ GBuffer │──▶│ Lighting│        │
│  │ Batch   │   │ Prepass │   │  Pass   │   │  Pass   │        │
│  └─────────┘   └─────────┘   └─────────┘   └─────────┘        │
│       │                           │              │              │
│       │                           ▼              ▼              │
│       │                    ┌─────────────────────────┐         │
│       │                    │      XeGTAO SSAO        │         │
│       │                    └─────────────────────────┘         │
│       │                                  │                      │
│       ▼                                  ▼                      │
│  ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐        │
│  │ Shadow  │──▶│ Outline │──▶│  Atmo   │──▶│  Post   │──▶ OUT │
│  │  Maps   │   │  Pass   │   │  Pass   │   │ Process │        │
│  └─────────┘   └─────────┘   └─────────┘   └─────────┘        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 📦 Phase 1: Foundation (Week 1-2)

### 1.1 GPU Memory Management

**Library:** `gpu-allocator`

```rust
// Purpose: Efficient VRAM allocation, no fragmentation
// Location: crates/renderer/src/gpu/allocator.rs

struct GpuMemoryManager {
    allocator: Allocator,
    
    // Pre-allocated pools for common sizes
    uniform_pool: BufferPool,      // 256B aligned, for per-frame data
    vertex_pool: BufferPool,       // Large blocks for geometry
    texture_pool: TexturePool,     // Atlased textures
    
    // Render targets (reused every frame)
    rt_pool: RenderTargetPool,
}

// Budget allocation:
// - Geometry: 100MB (instanced, shared meshes)
// - Textures: 200MB (compressed, atlased)  
// - Render targets: 150MB (half-res where possible)
// - Uniforms: 16MB (ring buffer)
// - Overhead: 34MB
// Total: ~500MB
```

**Key Optimizations:**
- Ring buffer for per-frame uniforms (no allocation per frame)
- Texture atlasing (reduce bind calls)
- Render target reuse (ping-pong buffers)

### 1.2 Texture System

**Libraries:** `image`, `png`, `ddsfile`

```rust
// Purpose: Compressed textures, minimal VRAM
// Location: crates/renderer/src/assets/textures.rs

enum TextureFormat {
    // Runtime formats (GPU compressed)
    BC1,      // RGB, 4bpp - foliage, terrain
    BC3,      // RGBA, 8bpp - UI, decals
    BC5,      // RG, 8bpp - normal maps
    BC7,      // RGBA high quality, 8bpp - hero textures
    
    // Procedural (generated at load)
    Noise2D,  // SSAO noise, dithering
    Noise3D,  // Volumetric effects
    LUT,      // Color grading
}

struct TextureAtlas {
    // Pack multiple textures into one
    // Reduces texture binds from 100s to ~10
    atlas_texture: Texture,
    uv_remap: HashMap<AssetId, Rect>,
}

// Size budget per texture type:
// - Terrain atlas: 2048x2048 BC1 = 2MB
// - Props atlas: 2048x2048 BC7 = 16MB
// - Normal atlas: 2048x2048 BC5 = 8MB
// - UI atlas: 1024x1024 BC3 = 2MB
// - Noise textures: 256x256 R8 = 64KB
// Total textures: ~30MB
```

### 1.3 Shader Preprocessing

**Library:** `shader-prepper`

```rust
// Purpose: Modular shaders, quality variants
// Location: crates/renderer/build.rs (compile-time)

// Shader structure:
// assets/shaders/
// ├── includes/
// │   ├── common.wgsl      # Math, noise
// │   ├── lighting.wgsl    # PBR functions
// │   ├── sampling.wgsl    # Texture sampling
// │   └── tonemapping.wgsl # Color transforms
// ├── passes/
// │   ├── gbuffer.wgsl
// │   ├── lighting.wgsl
// │   ├── ssao.wgsl
// │   └── post.wgsl
// └── materials/
//     ├── stylized.wgsl
//     └── terrain.wgsl

// Quality variants generated at build:
// - LOW: 8 SSAO samples, no soft shadows
// - MEDIUM: 16 SSAO samples, 16 shadow samples
// - HIGH: 32 SSAO samples, 32 shadow samples
// - ULTRA: 64 SSAO samples, 64 shadow samples
```

---

## 🎨 Phase 2: Draw Call Optimization (Week 2-3)

### 2.1 Instanced Rendering

```rust
// Purpose: Draw 1000s of objects in 1 draw call
// Location: crates/renderer/src/batching/instancer.rs

struct InstanceBatch {
    mesh: Handle<Mesh>,
    material: Handle<Material>,
    
    // Per-instance data (GPU buffer)
    transforms: Vec<Mat4>,      // 64 bytes each
    colors: Vec<Vec4>,          // 16 bytes each  
    custom_data: Vec<Vec4>,     // Material params
    
    // Indirect drawing
    indirect_buffer: Buffer,    // DrawIndexedIndirect
}

// Tiny Glade batching strategy:
// 1. Group by mesh type (all walls together)
// 2. Group by material (all stone together)
// 3. Use instance IDs for variation

// Example scene:
// - 500 wall pieces → 1 draw call
// - 200 roof tiles → 1 draw call  
// - 100 foliage instances → 1 draw call
// - 50 unique props → 50 draw calls
// Total: ~55 draw calls for complex scene
```

### 2.2 GPU Culling

```rust
// Purpose: Don't draw what's not visible
// Location: crates/renderer/src/culling/

struct GpuCuller {
    // Compute shader culling
    frustum_cull_pipeline: ComputePipeline,
    occlusion_cull_pipeline: ComputePipeline,
    
    // Hierarchical-Z for occlusion
    hi_z_pyramid: Texture,  // Downsampled depth
}

// Culling pipeline:
// 1. Frustum cull (compute shader) - 0.1ms
// 2. Build Hi-Z pyramid - 0.2ms
// 3. Occlusion cull (compute shader) - 0.2ms
// 4. Compact visible list - 0.1ms
// Total: ~0.6ms for 10,000 objects
```

### 2.3 Bindless Textures (Optional - Vulkan)

```rust
// Purpose: Eliminate texture bind overhead
// Only if using ash/vulkan directly

struct BindlessTextureArray {
    // All textures in one descriptor
    textures: Vec<TextureView>,
    sampler: Sampler,
    
    // Shaders use texture_index instead of binding
    // texture(textures[material.texture_index], uv)
}

// Benefit: 0 texture binds per draw call
// All materials can batch together
```

---

## 🌟 Phase 3: XeGTAO Implementation (Week 3-4)

### 3.1 GTAO Algorithm

```rust
// Purpose: State-of-the-art ambient occlusion
// Location: crates/renderer/src/passes/gtao/

struct GtaoPass {
    // Two-pass algorithm
    main_pass: ComputePipeline,     // AO calculation
    denoise_pass: ComputePipeline,  // Spatial-temporal denoise
    
    // Render targets
    ao_texture: Texture,            // R8, half resolution
    depth_mip_chain: Texture,       // For fast depth sampling
    
    // History for temporal filtering
    history_ao: Texture,
    history_depth: Texture,
}

// XeGTAO quality levels:
// LOW:    4 directions, 2 steps = 8 samples
// MEDIUM: 6 directions, 3 steps = 18 samples  
// HIGH:   8 directions, 4 steps = 32 samples
// ULTRA:  12 directions, 5 steps = 60 samples

// Performance (1080p, GTX 1060):
// LOW:    0.3ms
// MEDIUM: 0.5ms
// HIGH:   0.8ms
// ULTRA:  1.2ms
```

### 3.2 GTAO WGSL Shader

```wgsl
// Simplified XeGTAO algorithm
// Full implementation in assets/shaders/renderer/gtao.wgsl

@compute @workgroup_size(8, 8, 1)
fn gtao_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let uv = (vec2<f32>(id.xy) + 0.5) / screen_size;
    let depth = sample_depth(uv);
    let position = reconstruct_position(uv, depth);
    let normal = sample_normal(uv);
    
    var ao = 0.0;
    let slice_count = QUALITY_SLICES;  // 4-12
    let step_count = QUALITY_STEPS;    // 2-5
    
    // Rotate sampling pattern per pixel
    let noise = blue_noise(id.xy);
    let rotation = noise.x * TAU;
    
    for (var slice = 0u; slice < slice_count; slice++) {
        let phi = (f32(slice) + noise.y) * PI / f32(slice_count);
        let direction = vec2(cos(phi + rotation), sin(phi + rotation));
        
        // Horizon angles (ground truth)
        var h1 = -1.0;
        var h2 = -1.0;
        
        for (var step = 0u; step < step_count; step++) {
            let t = (f32(step) + noise.z) / f32(step_count);
            let offset = direction * t * ao_radius;
            
            // Sample at offset
            let sample_uv = uv + offset / screen_size;
            let sample_depth = sample_depth_mip(sample_uv, t);
            let sample_pos = reconstruct_position(sample_uv, sample_depth);
            
            // Update horizon
            let delta = sample_pos - position;
            let dist = length(delta);
            let angle = atan2(delta.z, length(delta.xy));
            
            h1 = max(h1, angle);
            h2 = max(h2, -angle);
        }
        
        // Integrate visibility
        let n_dot_slice = dot(normal, vec3(direction, 0.0));
        ao += integrate_arc(h1, h2, n_dot_slice);
    }
    
    ao = ao / f32(slice_count);
    ao = pow(ao, ao_power) * ao_intensity;
    
    textureStore(ao_output, id.xy, vec4(ao));
}
```

---

## 🖼️ Phase 4: Render Targets & Memory (Week 4-5)

### 4.1 Render Target Layout

```rust
// Purpose: Minimize VRAM for render targets
// Location: crates/renderer/src/gpu/render_targets.rs

struct RenderTargets {
    // Full resolution (1920x1080 example)
    depth: Texture,           // D32F = 8MB
    gbuffer_albedo: Texture,  // RGBA8 = 8MB
    gbuffer_normal: Texture,  // RG16F = 8MB (octahedral)
    gbuffer_material: Texture,// RGBA8 = 8MB (roughness, metal, ao, ?)
    
    // Half resolution (960x540)
    ssao: Texture,            // R8 = 0.5MB
    ssao_blurred: Texture,    // R8 = 0.5MB
    
    // Quarter resolution (480x270) - bloom
    bloom_chain: [Texture; 5],// R11G11B10F = 0.3MB total
    
    // Shadows (cascaded)
    shadow_cascade: [Texture; 3], // D16 2048x2048 = 24MB
    
    // Total: ~58MB render targets
}

// Reuse strategy:
// - SSAO texture reused for outline pass
// - Bloom chain ping-ponged
// - Shadow maps shared between frames (cached)
```

### 4.2 Resolution Scaling

```rust
// Purpose: Dynamic resolution for consistent FPS
// Location: crates/renderer/src/gpu/resolution.rs

struct DynamicResolution {
    target_fps: f32,          // 60.0
    min_scale: f32,           // 0.5 (50% resolution)
    max_scale: f32,           // 1.0 (100% resolution)
    current_scale: f32,
    
    // History for smoothing
    frame_times: RingBuffer<f32, 30>,
}

impl DynamicResolution {
    fn update(&mut self, frame_time: f32) {
        let avg_time = self.frame_times.average();
        let target_time = 1.0 / self.target_fps;
        
        if avg_time > target_time * 1.1 {
            // Too slow, reduce resolution
            self.current_scale = (self.current_scale - 0.05).max(self.min_scale);
        } else if avg_time < target_time * 0.9 {
            // Headroom, increase resolution
            self.current_scale = (self.current_scale + 0.02).min(self.max_scale);
        }
    }
}
```

---

## 📝 Phase 5: Procedural Textures (Week 5)

### 5.1 Noise Generation

**Library:** `tiny-skia` (CPU generation at load time)

```rust
// Purpose: Generate noise textures, no disk storage
// Location: crates/renderer/src/assets/procedural.rs

struct ProceduralTextures {
    // Generated once at startup
    blue_noise_256: Texture,      // 256x256 RGBA8 - SSAO/TAA
    white_noise_64: Texture,      // 64x64 R8 - general
    perlin_512: Texture,          // 512x512 R8 - terrain blend
    voronoi_256: Texture,         // 256x256 R8 - grass clumps
    
    // Precomputed LUTs
    brdf_lut: Texture,            // 256x256 RG16F - PBR
    color_grade_lut: Texture,     // 32x32x32 RGB8 - post
}

fn generate_blue_noise(size: u32) -> Vec<u8> {
    // Void-and-cluster algorithm for best quality
    // Or use pre-baked sequences for speed
    let mut pixmap = tiny_skia::Pixmap::new(size, size).unwrap();
    
    // ... generation algorithm ...
    
    pixmap.data().to_vec()
}

// Storage saved:
// - 10+ noise textures = 0MB on disk (generated)
// - ~2MB VRAM (runtime only)
```

---

## 💬 Phase 6: Text Rendering (Week 6)

### 6.1 Font System

**Libraries:** `cosmic-text`, `swash`

```rust
// Purpose: Efficient UI text, minimal draw calls
// Location: crates/renderer/src/ui/text.rs

struct TextRenderer {
    font_system: FontSystem,
    swash_cache: SwashCache,
    
    // Glyph atlas (all glyphs in one texture)
    glyph_atlas: TextureAtlas,    // 1024x1024 R8 = 1MB
    glyph_cache: HashMap<GlyphKey, GlyphInfo>,
    
    // Batched text rendering
    text_batch: Vec<TextInstance>,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
}

// All UI text in 1-2 draw calls:
// 1. Build glyph instances
// 2. Upload to GPU
// 3. Single instanced draw
```

---

## ⏱️ Frame Budget (16.67ms for 60 FPS)

```
┌─────────────────────────────────────────────────────────────┐
│                    FRAME TIMELINE (GTX 1060)                │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  CPU Work (parallel with prev frame GPU)         │ 2.0ms   │
│  ├── Scene update                                │ 0.5ms   │
│  ├── Culling (CPU side)                          │ 0.3ms   │
│  ├── Batch building                              │ 0.5ms   │
│  └── Command recording                           │ 0.7ms   │
│                                                             │
│  GPU Work                                        │ 12.0ms  │
│  ├── Depth Prepass                               │ 0.8ms   │
│  ├── Shadow Maps (3 cascades)                    │ 1.5ms   │
│  ├── GBuffer Pass                                │ 1.5ms   │
│  ├── GTAO (half res)                             │ 0.8ms   │
│  ├── GTAO Denoise                                │ 0.3ms   │
│  ├── Lighting + Shadows                          │ 2.0ms   │
│  ├── Outline Pass                                │ 0.4ms   │
│  ├── Atmosphere                                  │ 0.3ms   │
│  ├── Bloom (5 passes)                            │ 0.8ms   │
│  ├── Post Process                                │ 0.5ms   │
│  └── UI                                          │ 0.3ms   │
│                                                             │
│  Present + Overhead                              │ 2.67ms  │
│                                                             │
│  TOTAL                                           │ 16.67ms │
│  TARGET FPS                                      │ 60 FPS  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 📁 File Structure

```
crates/renderer/
├── Cargo.toml
├── build.rs                    # Shader preprocessing
├── src/
│   ├── lib.rs                  # Main plugin
│   ├── settings.rs             # Quality presets
│   │
│   ├── gpu/
│   │   ├── mod.rs
│   │   ├── allocator.rs        # gpu-allocator wrapper
│   │   ├── buffers.rs          # Ring buffers, pools
│   │   ├── textures.rs         # Texture management
│   │   └── render_targets.rs   # RT allocation
│   │
│   ├── assets/
│   │   ├── mod.rs
│   │   ├── textures.rs         # image/dds loading
│   │   ├── procedural.rs       # tiny-skia generation
│   │   └── atlasing.rs         # Texture packing
│   │
│   ├── batching/
│   │   ├── mod.rs
│   │   ├── instancer.rs        # Instance batching
│   │   ├── indirect.rs         # Indirect drawing
│   │   └── material_sort.rs    # Draw call sorting
│   │
│   ├── culling/
│   │   ├── mod.rs
│   │   ├── frustum.rs          # Frustum culling
│   │   ├── occlusion.rs        # Hi-Z occlusion
│   │   └── lod.rs              # LOD selection
│   │
│   ├── passes/
│   │   ├── mod.rs
│   │   ├── depth_prepass.rs
│   │   ├── gbuffer.rs
│   │   ├── gtao.rs             # XeGTAO
│   │   ├── shadows.rs          # Cascaded soft shadows
│   │   ├── lighting.rs
│   │   ├── outline.rs
│   │   ├── atmosphere.rs
│   │   └── post_process.rs
│   │
│   ├── ui/
│   │   ├── mod.rs
│   │   └── text.rs             # cosmic-text
│   │
│   └── pipeline/
│       ├── mod.rs
│       └── graph.rs            # Render graph
│
└── assets/
    └── shaders/
        ├── includes/           # Shared shader code
        └── passes/             # Per-pass shaders
```

---

## 📊 Memory Budget Summary

```
┌────────────────────────────────────────┐
│          VRAM BUDGET (~500MB)          │
├────────────────────────────────────────┤
│ Geometry (meshes)          │  80MB     │
│ Texture Atlases            │  30MB     │
│ Render Targets             │  60MB     │
│ Shadow Maps                │  25MB     │
│ Depth Mip Chain            │  15MB     │
│ Uniform Buffers            │  16MB     │
│ Instance Buffers           │  32MB     │
│ Glyph Atlas               │   1MB     │
│ Procedural Textures        │   2MB     │
│ Overhead/Fragmentation     │  39MB     │
├────────────────────────────────────────┤
│ TOTAL                      │ ~300MB    │
│ BUDGET                     │  500MB    │
│ HEADROOM                   │  200MB    │
└────────────────────────────────────────┘

┌────────────────────────────────────────┐
│        DISK STORAGE (<100MB)           │
├────────────────────────────────────────┤
│ Compiled Shaders (SPIRV)   │   2MB     │
│ Compressed Textures (DDS)  │  30MB     │
│ Meshes (compressed)        │  15MB     │
│ Fonts                      │   3MB     │
│ Audio                      │  20MB     │
│ Config/Data                │   5MB     │
├────────────────────────────────────────┤
│ TOTAL                      │  ~75MB    │
└────────────────────────────────────────┘
```

---

## 🚀 Implementation Order

### Week 1-2: Foundation
- [ ] Set up `gpu-allocator` memory management
- [ ] Implement texture loading with `image`/`ddsfile`
- [ ] Create buffer pools (uniform, vertex, instance)
- [ ] Set up `shader-prepper` build system

### Week 3: Batching
- [ ] Implement instance batching system
- [ ] Add indirect drawing support
- [ ] Create material sorting for minimal state changes
- [ ] GPU frustum culling compute shader

### Week 4: XeGTAO
- [ ] Port XeGTAO main pass to WGSL
- [ ] Implement spatial denoiser
- [ ] Add temporal filtering
- [ ] Quality presets (LOW/MED/HIGH/ULTRA)

### Week 5: Render Pipeline
- [ ] Depth prepass with Hi-Z generation
- [ ] GBuffer pass (albedo, normal, material)
- [ ] Deferred lighting with PCSS shadows
- [ ] Integrate GTAO into lighting

### Week 6: Post & Polish
- [ ] Outline pass (reusing SSAO buffer)
- [ ] Atmosphere (fog, sky)
- [ ] Post-process (bloom, color grade, vignette)
- [ ] Dynamic resolution scaling

### Week 7: UI & Text
- [ ] Integrate `cosmic-text` for text layout
- [ ] Glyph atlas generation with `swash`
- [ ] Batched UI rendering
- [ ] Debug overlays

### Week 8: Optimization
- [ ] Profile on GTX 1060
- [ ] Tune quality presets
- [ ] Memory optimization pass
- [ ] Final testing

---

## ✅ Success Criteria

- [ ] 60 FPS stable on GTX 1060 at 1080p MEDIUM
- [ ] < 100 draw calls for typical scene
- [ ] < 500MB VRAM usage
- [ ] < 100MB install size
- [ ] < 3 second load time
- [ ] Visual quality matches Tiny Glade style
