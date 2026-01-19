# Funky Renderer - Tiny Glade Style Roadmap

## Current Status: Week 1-2 Foundation — ~25% Complete

---

## ✅ Completed Features

### Core Infrastructure
- [x] Rust + Vulkan (ash 0.38) foundation
- [x] gpu-allocator memory management
- [x] winit 0.30 windowing
- [x] Bevy ECS for game logic
- [x] IMMEDIATE present mode (no vsync cap)
- [x] Per-swapchain-image depth buffers
- [x] Images-in-flight fence tracking

### Rendering
- [x] glTF loading + textures
- [x] Basic PBR-ish lighting (diffuse + specular + fill)
- [x] Push constants for per-object transforms
- [x] Ground plane + duck scene composition
- [x] Vulkan Y-flip projection fix
- [x] **Cascaded Shadow Maps (4 cascades, 2048×2048)**
- [x] **PCF shadow filtering (3×3 kernel)**
- [x] **Depth-only shadow pass with proper barriers**

### Debug/Tools
- [x] egui debug UI overlay
- [x] FPS counter, frame time display
- [x] Duck scale slider
- [x] Free camera (WASD + arrows, 360° rotation)

### Performance
- [x] ~4000 FPS baseline (minimal scene)
- [x] Release profile optimized (LTO, codegen-units=1)
- [x] Debug prints removed from hot path

---

## 🔨 In Progress

### Week 2: Shadows & AO
- [x] Cascaded shadow maps infrastructure
- [x] PCF filtering
- [x] Slope-scaled bias tuning (live depth bias + PCF bias sliders)
- [x] Shadow debug visualization (cascade colors)

---

## ❌ TODO — Critical Path (Weeks 2-4)

### Instanced Rendering
- [ ] Per-instance buffer (model matrix + material ID)
- [ ] Batch meshes by material
- [ ] Single draw call per material group
- [ ] Instance count in debug UI

### GPU Culling / Hi-Z
- [ ] CPU frustum culling (quick win)
- [ ] Hi-Z depth pyramid generation
- [ ] GPU frustum culling compute shader
- [ ] GPU occlusion culling via Hi-Z
- [ ] Indirect draw buffer

### XeGTAO (Screen-Space AO)
- [ ] Half-res depth buffer
- [ ] AO compute shader (XeGTAO algorithm)
- [ ] Spatial denoising pass
- [ ] Temporal accumulation
- [ ] AO apply pass (multiply with lighting)

---

## ❌ TODO — Visual Quality (Weeks 5-6)

### Ray Marching (Tiny Glade Core Technique)
- [ ] Screen-space ray marching framework
- [ ] **Linear + point sampling trick** (the "acne killer")
- [ ] Contact shadows (short-range ray march)
- [ ] Ray-marched reflections (SSR)
- [ ] Ray-marched GI (single bounce)
- [ ] Ray-marched DoF

### Temporal Anti-Aliasing (TAA)
- [ ] Motion vectors pass
- [ ] History buffer management
- [ ] Neighborhood clamping
- [ ] Temporal reprojection
- [ ] Jittered projection matrices

### Tonemapping & Color
- [ ] HDR render target (R16G16B16A16_SFLOAT)
- [ ] ACES / AgX tonemapping
- [ ] Exposure control (auto + manual)
- [ ] Color grading LUT support

---

## ❌ TODO — Polish (Weeks 7-8)

### Post-Processing Stack
- [ ] Bloom (threshold + blur + composite)
- [ ] Depth of Field (bokeh or gaussian)
- [ ] Vignette
- [ ] Film grain (subtle)
- [ ] Chromatic aberration (very subtle)

### Atmosphere & Sky
- [ ] Procedural sky gradient
- [ ] Sun disk rendering
- [ ] Atmospheric scattering (simple model)
- [ ] Cloud layer (optional)

### LOD System
- [ ] Mesh LOD loading from glTF
- [ ] Distance-based LOD selection
- [ ] LOD crossfade (dithering)

### Scene Management
- [ ] Multiple glTF model loading
- [ ] Scene graph / transform hierarchy
- [ ] Frustum-based streaming (large scenes)

---

## 📊 Performance Targets

| Feature | Budget | Current |
|---------|--------|---------|
| Shadow pass | 1-2ms | ~0.3ms (simple scene) |
| SSAO | 0.5-1ms | — |
| Main geometry | 2-3ms | ~0.1ms |
| Post-processing | 1ms | — |
| TAA | 0.5ms | — |
| **Total** | **8-10ms (100+ FPS)** | **~0.25ms** |

Target: **100-200 FPS** on RTX 4060 with full pipeline

---

## 🎯 Immediate Next Steps

1. **XeGTAO SSAO** — Ground the scene visually
2. **Instancing** — Before adding more geometry
3. **Linear+point sampling trick** — Foundation for all ray marching
4. **Contact shadows** — Quick visual win with ray marching infra

---

## 📁 Key Files

| File | Purpose |
|------|---------|
| `src/gltf_renderer.rs` | Main rendering pipeline + shadows |
| `src/main.rs` | App loop, ECS, camera |
| `shaders/gltf.vert/frag` | Main scene shaders with CSM sampling |
| `shaders/shadow.vert/frag` | Depth-only shadow pass |
| `src/egui_integration.rs` | Debug UI |

---

*Last updated: January 19, 2026*
