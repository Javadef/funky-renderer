# 🏰 Tiny Glade Renderer - Complete Secrets Guide

> **Source:** Tomasz Stachowiak (Pounce Light) - Strange Loop Conference
> **Tech Stack:** Rust + Vulkan + HLSL + Bevy ECS
> **Target:** 60 FPS on 10-year-old hardware (potato GPUs)

---

## 🎯 The Big Picture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        TINY GLADE RENDERER PIPELINE                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐                   │
│  │   GEOMETRY   │───▶│   LIGHTING   │───▶│    POST     │                   │
│  │  (GPU-Driven)│    │   (Hybrid)   │    │ (Ray March)  │                   │
│  └──────────────┘    └──────────────┘    └──────────────┘                   │
│         │                   │                   │                           │
│         ▼                   ▼                   ▼                           │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐                   │
│  │    NITE      │    │  Shadows +   │    │    DoF +     │                   │
│  │ (Culling/LOD)│    │   GI + AO    │    │  Tonemapping │                   │
│  └──────────────┘    └──────────────┘    └──────────────┘                   │
│                                                                             │
│  KEY INSIGHT: Everything is ray marching. Shadows, GI, DoF, reflections.    │
│  They unified their approach around one core technique.                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 📦 Part 1: Foundation & Architecture

### 1.1 Why Custom Engine?

Started as Anna (Anastasia Opara)'s hobby project to learn real-time rendering. The procedural generation approach (individual bricks, planks, pebbles) was set in stone before optimization concerns. This constraint drove everything else.

**Key constraint:** Hundreds of thousands of unique objects, each a separate mesh.

### 1.2 Tech Stack Choice

```
┌─────────────────────────────────────────────────────────────────┐
│ Rust        - Memory safety, no crashes in entire development   │
│ Vulkan      - Low-level GPU control for custom pipeline         │
│ HLSL        - Shader language (compiled to SPIR-V)              │
│ Bevy ECS    - Game logic, systems, scheduling                   │
│ Crates      - Fast iteration via Rust ecosystem                 │
└─────────────────────────────────────────────────────────────────┘
```

**Why Rust?**
- Complex generator chains with infinite undo/redo
- Fully deterministic procedural generation
- Zero memory corruption crashes during entire development
- Crate ecosystem for rapid prototyping

### 1.3 The Scale Problem

```
Initial trailer:  ~1,000 bricks
Shipped game:     ~1,000,000 bricks (scaling to 2,000,000)
Community builds: People recreate LOTR Minas Tirith
```

The better UX got, the crazier players built. No upper limit on complexity.

---

## 🔺 Part 2: GPU-Driven Rendering ("NITE")

### 2.1 Core Philosophy

```
CPU: "Here's ALL the geometry data in one buffer"
GPU: "I'll figure out what to draw"
```

No mesh synthesis, no skinning. Just millions of tiny meshes (bricks, planks, tiles).

### 2.2 Data Layout

```
┌─────────────────────────────────────────────────────────────────┐
│                    MATERIAL VERTEX BUFFER                        │
├─────────────────────────────────────────────────────────────────┤
│ All meshes for ONE material packed into SINGLE vertex buffer    │
│                                                                  │
│ ┌────────┬────────┬────────┬────────┬────────┐                  │
│ │ Brick  │ Brick  │ Brick  │ Brick  │ Brick  │  ... (LOD 0)    │
│ │ Mesh 0 │ Mesh 1 │ Mesh 2 │ Mesh 3 │ Mesh 4 │                  │
│ └────────┴────────┴────────┴────────┴────────┘                  │
│ ┌────────┬────────┬────────┬────────┬────────┐                  │
│ │ Brick  │ Brick  │ Brick  │ Brick  │ Brick  │  ... (LOD 1)    │
│ │ Mesh 0 │ Mesh 1 │ Mesh 2 │ Mesh 3 │ Mesh 4 │                  │
│ └────────┴────────┴────────┴────────┴────────┘                  │
│                                                                  │
│ LOD change = +1 to mesh array index (instant, no rebinding)     │
└─────────────────────────────────────────────────────────────────┘
```

### 2.3 Two-Pass Occlusion Culling

```
FRAME N:
┌─────────────────────────────────────────────────────────────────┐
│ PASS 1: Draw everything visible in frame N-1                    │
│         (stable mesh IDs between frames)                        │
│                              │                                   │
│                              ▼                                   │
│         Build Hierarchical Depth Pyramid (Hi-Z)                 │
│                              │                                   │
│                              ▼                                   │
│ PASS 2: Test ALL objects against depth pyramid                  │
│         Draw only the NEW visible objects (misses)              │
│                              │                                   │
│                              ▼                                   │
│         Store visible set for frame N+1                         │
└─────────────────────────────────────────────────────────────────┘
```

### 2.4 Draw Call Generation

**Simple Path (Bricks):**
```
- One draw list per material+mesh type
- Atomic increment into indirection map
- Single draw_indexed_indirect per material
- Draw count = number of mesh types
- Instance count = visible instances of each type
```

**Sorted Path (Trees with overdraw):**
```
┌─────────────────────────────────────────────────────────────────┐
│ 1. Bucket sort visible meshes into 1024 depth slices            │
│    (single-pass radix / counting sort)                          │
│                                                                  │
│ 2. Process in groups, build mini-histogram per group            │
│    - Same mesh × 42 instances = 1 draw                          │
│    - Mixed meshes = multiple draws                              │
│                                                                  │
│ 3. Output: draw_indexed_indirect_count                          │
│    - Tunable sorting vs draw count tradeoff                     │
│    - ~65% faster on AMD for tree rendering                      │
└─────────────────────────────────────────────────────────────────┘
```

### 2.5 LOD Secret: Ray Marching as Last LOD

```
Brick LOD 0: Full mesh (~100 triangles)
Brick LOD 1: Simplified (~20 triangles)
Brick LOD 2: 6 TRIANGLES (half cube, always front-facing)
             ↓
             Pixel shader becomes ray march dispatcher!
             Analytically intersect rounded box
             Synthesize bevels, catch light on edges
             Same visual quality, fraction of geometry
```

---

## 🌑 Part 3: Shadows

### 3.1 Why Not Variance Shadow Maps?

```
VSM/MSM: Light leaking with multiple depth layers
         ↓
Tiny Glade = "worst case generator for VSM"
         ↓
UGC means players CREATE artifact-causing scenes
         ↓
Must use PCF (despite bias headaches)
```

### 3.2 PCSS (Percentage Closer Soft Shadows)

Standard PCSS for contact hardening. But still has temporal aliasing:

```
Problem: Continuous time-of-day + low shadow resolution
         = horrible flickering/crawling on edges
```

### 3.3 🔥 SECRET: Temporal Anti-Aliasing for SHADOWS

```
Standard TAA: Derive statistics from screen-space neighborhood
              Doesn't work for shadows (features larger than 3×3)
              
SHADOW TAA:   Derive statistics from SHADOW KERNEL SPACE
              ↓
              You're already sampling for PCF!
              ↓
              Track VARIANCE alongside MEAN
              (mean = first moment, variance = second moment)
              ↓
              Build bounding box from variance (Marc Salvi's technique)
              ↓
              Clamp history to bounding box
              ↓
              Shrink box in soft shadow areas (prevents ghosting)
```

Result: Stable shadows from same resolution data.

### 3.4 🔥 SECRET: Contact Shadows via Ray Marching

```
Problem: Shadow maps lack fine detail (bricks sticking out)
Solution: Ray march toward light source in screen space

Before: Blurry shadow blob
After:  All brick detail pops, looks like added geometry
        (They actually had to SHRINK bricks - overcompensating!)
```

**vs Bend Studio (Days Gone) approach:**
- Bend: Slightly better detail resolution
- Tiny Glade: Fewer taps, can angle rays for soft shadows, fully temporally stable

### 3.5 🔥🔥 SECRET: Linear + Point Sampling Trick

**THE ACNE KILLER:**

```
Standard point sampling:
┌─────────────────────────────────────────┐
│     ┌───┐     ┌───┐     ┌───┐          │
│     │   │     │   │     │   │  ← Stair steps
│ ────┘   └─────┘   └─────┘   └────      │  = accidental hits
│                                         │  = ACNE
└─────────────────────────────────────────┘

Standard linear sampling:
┌─────────────────────────────────────────┐
│      ╱╲      ╱╲      ╱╲                 │
│     ╱  ╲    ╱  ╲    ╱  ╲   ← Shrink-wrapped
│    ╱    ╲──╱    ╲──╱    ╲──            │  = false occlusion
│                                         │  = ARTIFACTS
└─────────────────────────────────────────┘

🔥 THE TRICK: Use BOTH samplers! 🔥
┌─────────────────────────────────────────────────────────────────┐
│ float depth_linear = texture(depth_map, uv);        // Linear   │
│ float depth_point  = texelFetch(depth_map, coord);  // Point    │
│                                                                  │
│ // For INTERSECTION test: use FURTHEST (max)                    │
│ float depth_intersect = max(depth_linear, depth_point);         │
│                                                                  │
│ // For THICKNESS test: use CLOSEST (min)                        │
│ float depth_thickness = min(depth_linear, depth_point);         │
│                                                                  │
│ Result: Quantization stair-steps GONE                           │
│         Discontinuity artifacts GONE                            │
│         Detail PRESERVED                                        │
│         NO BIAS NEEDED                                          │
└─────────────────────────────────────────────────────────────────┘
```

Tom admits he doesn't fully understand WHY it works, but it does. Hand-wave explanation:
- Linear cuts off stair-step corners
- Point reintroduces vertical discontinuities for miss detection
- Combined = smooth continuous surface that still has edges

**They use this for ALL ray marching in the game.**

---

## ☀️ Part 4: Global Illumination

### 4.1 Evolution of Approaches

```
Attempt 1: Top-down lightmap (Motor GP 2004 technique)
           → Fails with shape layering, cold/wrong colors

Attempt 2: DDGI (Dynamic Diffuse GI)
           → Not enough probe density
           → Aliasing (small shapes miss probes)
           → Visibility leaking
           → Ghosting when resizing buildings
           → Time-of-day forced constant updates anyway

Attempt 3: Screen-space probes (Lumen-style)
           → Promising but finicky
           → Low spatial resolution = flat lighting
           → Probes stuck in creases
           → Strange metallic sheen from projection errors

Attempt 4: ReSTIR GI
           → Complete overkill!
           → Outdoor scenes = low variance
           → Temporal-only ReSTIR worked
           → Spatial resampling actually HURT (rejected samples = waste)
           → Ended up with "full potato mode"
```

### 4.2 Final GI Pipeline: "Full Potato Mode"

```
┌─────────────────────────────────────────────────────────────────┐
│                    TINY GLADE GI PIPELINE                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ STEP 1: Quarter-Resolution Ray Tracing (1 ray per 16 px)│    │
│  │                                                          │    │
│  │   • Start with RAY MARCH (cheap)                        │    │
│  │   • If ray march fails → switch to RAY TRACE            │    │
│  │   • If hit is on-screen → sample screen radiance        │    │
│  │     (hides potato proxy geometry)                       │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ STEP 2: Project to Spherical Harmonics (SH2, L=2)       │    │
│  │         4 components per channel                         │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ STEP 3: Spatial Reconstruction Filter                   │    │
│  │         8 quarter-res samples → full resolution         │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ STEP 4: Denoise in SH Space (à la Metro Exodus)         │    │
│  └─────────────────────────────────────────────────────────┘    │
│                              │                                   │
│                              ▼                                   │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │ STEP 5: Evaluate SH with Cross-Bilateral Filter         │    │
│  │         "Hallucinated" SH for final radiance            │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 4.3 🔥 SECRET: Stabilizing Recurrent Blur Denoiser

From DZY (pronunciation unknown). Self-feeding filter:

```
┌─────────────────────────────────────────────────────────────────┐
│ PASS 1: Generate new radiance samples (ray trace)               │
│                                                                  │
│ PASS 2: Temporal reprojection                                   │
│         • Reproject history                                     │
│         • If fails → gap fill from neighbors                    │
│         • If gap fill fails → sample from OUTPUT texture        │
│           (race condition, but doesn't matter here)             │
│                                                                  │
│ PASS 3: Spatial filter (SMALL kernel)                           │
│         • Track accumulated sample count                        │
│         • LOW count → LARGE kernel (need more blur)             │
│         • HIGH count → SMALL kernel (tighten detail)            │
│         • Kernel grows CONICALLY over time                      │
└─────────────────────────────────────────────────────────────────┘
```

### 4.4 🔥 SECRET: SSAO as Denoiser Guide

```
Standard denoise: Blur everything equally
                  ↓
                  Corners lose definition

Tiny Glade:      Use XeGTAO output as cross-bilateral weight!
                  ↓
                  Corners have high AO = low blur weight
                  ↓
                  Sharp detail preserved in corners

Final = Denoised_GI × (1 + SSAO × 0.2)
        ↑ GI                ↑ Tiny bit of extra AO for contact detail
```

### 4.5 Software Ray Tracing

```
┌─────────────────────────────────────────────────────────────────┐
│ WHY SOFTWARE?                                                    │
│ • Min spec = potato GPUs (no RTX)                               │
│ • Same look on all platforms                                    │
│                                                                  │
│ PROXY GEOMETRY:                                                  │
│ • Reuse collision proxies (already generating them!)            │
│ • No roofs, simplified shapes                                   │
│ • Surprisingly good results when combined with screen-space     │
│                                                                  │
│ BVH BUILDING:                                                    │
│ • Originally: Embree (Intel)                                    │
│ • Now: obvhs crate (pure Rust, Griffin's work)                  │
│                                                                  │
│ TRAVERSAL:                                                       │
│ • Wide BVH (based on Ylitie Karras "CUDA Path Tracer")          │
│ • Compute shader                                                │
└─────────────────────────────────────────────────────────────────┘
```

---

## 🌊 Part 5: Water & Ice

### 5.1 Water Pipeline

```
Runs AFTER deferred lighting (forward-lit water)
Currently flat (plans for waves)

REFLECTION:
┌─────────────────────────────────────────────────────────────────┐
│ 1. SSR first: 8 linear steps + 3 bisection (cheap)              │
│                                                                  │
│ 2. Detect "ugly" (SSR misses)                                   │
│                                                                  │
│ 3. Write misses to buffer, COMPACT them                         │
│                                                                  │
│ 4. Separate dispatch: Ray trace ONLY misses at HALF resolution  │
│    (maintains GPU occupancy vs inline ray tracing)              │
│                                                                  │
│ 5. If ray trace hit is on-screen → sample screen radiance       │
│                                                                  │
│ 🔥 SECRET: "Hallucinate" hits from neighbor quads               │
│    • If ANY ray in quad hits, assume others hit at same depth   │
│    • Misses happen per-quad → enables half-res ray trace        │
│                                                                  │
│ 🔥 SECRET: Fall back to BLACK, not traced color                 │
│    • Traced results can look jarring (wrong lighting)           │
│    • Black + waves = looks fine, hides proxy potato geometry    │
└─────────────────────────────────────────────────────────────────┘

REFRACTION:
┌─────────────────────────────────────────────────────────────────┐
│ 🔥 SECRET: DON'T do real refraction!                            │
│                                                                  │
│ Real refraction:                                                 │
│ • Ray shortening makes ponds look shallow                       │
│ • Bent rays sample off-screen data                              │
│ • More expensive                                                 │
│                                                                  │
│ Their hack:                                                      │
│ • Just DISTORT ray by wave normal                               │
│ • Ray march the distorted ray (3 steps + 2 bisection)           │
│ • Sample from underwater-only depth copy                        │
│ • Looks great, super cheap                                      │
└─────────────────────────────────────────────────────────────────┘

CAUSTICS:
┌─────────────────────────────────────────────────────────────────┐
│ Complete fakery:                                                 │
│ • Random threshold on wave textures                             │
│ • Project onto water bed using sun angle                        │
│ • "Good enough"                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 5.2 Ice Pipeline

```
Similar to water but ROUGH reflections/refractions

Surface pass outputs:
• Diffuse lighting
• Diffuse shadows (with TAA - outputs shadow map as MRT!)
• Deferred reflection data
• Deferred refraction data

REFLECTION:
• Ray march at half resolution
• Spherical cap GGX sampling (70% bias - cut edges)
• More steps than water (need definition for reeds in ice)
• Compact misses → ray trace
• Denoise with lobe-projected kernel radius

REFRACTION:
🔥 SECRET: Reflection UPSIDE DOWN
• Take reflection BRDF lobe
• Flip it
• Trace "reflection" downward
• Simpler lobe = simpler math
• Temporal filter (ghosting OK - everything blurry under ice)

ICE CRACKS:
• Just meshes with fake normals pointing at light
• Always catch light
• Blurry refraction makes them look volumetric
• Beer-Lambert absorption for depth coloring
```

---

## 🎥 Part 6: Depth of Field

### 6.1 Journey to Solution

```
Attempt 1: Dennis Gustafsson's single-pass shader
           → Foreground too sharp (feature, not bug initially)
           → Inspired by Ghibli painterly backgrounds

Attempt 2: Various scatter/gather algorithms
           → None handle foreground defocus well in real-time

Attempt 3: OIT-style sorting
           → Couldn't make it work

Attempt 4: Accumulation buffer (jitter viewpoint)
           → Works but need hundreds of views
           → Can synthesize views with parallax, but filling is hard
```

### 6.2 🔥🔥 SECRET: Ray Marched DoF

**Core insight:** Treat DoF as ray marching, not scatter/gather!

```
┌─────────────────────────────────────────────────────────────────┐
│ PATH TRACER DoF:                                                 │
│ • Sample points on aperture                                     │
│ • Mirror around focal point                                     │
│ • Trace rays                                                    │
│                                                                  │
│ SCREEN-SPACE VERSION:                                           │
│ • Do the same with RAY MARCHING                                 │
│ • Tons of rays = 100ms (cache thrashing)                        │
│                                                                  │
│ OPTIMIZATION INSIGHT:                                           │
│ • Look at 1D radial slices of DoF kernel                        │
│ • Rays at different angles trace SIMILAR data                   │
│ • Can PREFETCH and REUSE between rays                           │
└─────────────────────────────────────────────────────────────────┘

ALGORITHM:
┌─────────────────────────────────────────────────────────────────┐
│ 1. Work in 1D radial slices of kernel                           │
│                                                                  │
│ 2. Prefetch CoC/depth values into LDS (shared memory)           │
│                                                                  │
│ 3. For each slice:                                               │
│    • Calculate O×M intersections                                │
│    • Each ray = different slope, simple division difference     │
│    • Store ray-hit as BITMASK in VGPR                           │
│                                                                  │
│ 4. Find intersections: first_bit_low / first_bit_high           │
│                                                                  │
│ 5. Refine with second intersection test                         │
│                                                                  │
│ 6. Combine all radial views                                     │
└─────────────────────────────────────────────────────────────────┘

EDGE CASE FIX:
┌─────────────────────────────────────────────────────────────────┐
│ Problem: Rays traveling under surface all use same fallback     │
│          → Sharpening artifact at edges                         │
│                                                                  │
│ 🔥 HACK: Check if intersection is far from focal point          │
│          → If yes, ray traveled under surface                   │
│          → Fall back to PRE-BLURRED background                  │
│          → Artifact mostly gone                                 │
└─────────────────────────────────────────────────────────────────┘

OPTIMIZATIONS:
• Tile classification (half-res for low CoC variance)
• MIP pre-filtering for hotspots
• TAA on output (their HDR is low, so works)

PERFORMANCE: 1.3-1.5ms at 1440p on RTX 2080
             (vs 100ms brute force)
```

---

## 🎨 Part 7: Image Formation & Tonemapping

### 7.1 The "Notorious Six" Problem

```
Per-channel tonemapping → colors converge to sRGB cube corners:
Red, Green, Blue, Cyan, Magenta, Yellow

These aren't artistic choices - they're engineering artifacts!

┌─────────────────────────────────────────────────────────────────┐
│ Clamp:    Yellow "rat piss" color                               │
│ ACES:     Same yellow, plus shifts                              │
│ Reinhard: Sickly, unsaturated strange colors                    │
└─────────────────────────────────────────────────────────────────┘
```

### 7.2 🔥 SECRET: tony-mc-mapface Tonemapper

Tom's custom tonemapper (made before Tiny Glade, perfect fit):

```
┌─────────────────────────────────────────────────────────────────┐
│ Instead of saturating to cube corners...                        │
│ SHIFT toward WHITE                                              │
│                                                                  │
│ Implementation: Simple 3D LUT applied AFTER Reinhard            │
│                                                                  │
│ Caveats:                                                         │
│ • sRGB only (no HDR output)                                     │
│ • Subjective (Tom's preferences)                                │
│                                                                  │
│ Alternative: AgX by Troy Sobotka (similar characteristics)      │
│                                                                  │
│ Open source: https://github.com/h3r2tic/tony-mc-mapface         │
└─────────────────────────────────────────────────────────────────┘
```

---

## ☁️ Part 8: Sky & Atmosphere

### 8.1 No Fancy Atmospheric Scattering

```
┌─────────────────────────────────────────────────────────────────┐
│ Game isn't realistic → don't need realistic sky                 │
│                                                                  │
│ SKY:                                                             │
│ • "Bunch of blobs" blended together                             │
│ • Handcrafted timelines per level                               │
│ • Artist-driven, not physically based                           │
│                                                                  │
│ CLOUDS:                                                          │
│ • Sprites scattered on cylinder                                 │
│ • 3 textures for different light angles                         │
│ • Blend based on sun direction                                  │
│ • Standard particle lighting trick                              │
│                                                                  │
│ FAKE BAG (?) LIGHTING:                                          │
│ • Bunch of spheres composed                                     │
│ • Painted over in Houdini                                       │
│                                                                  │
│ KEY: Sky becomes light source for GI                            │
└─────────────────────────────────────────────────────────────────┘
```

---

## 🔦 Part 9: Local Lights

```
┌─────────────────────────────────────────────────────────────────┐
│ Simple approach (for now):                                       │
│ • for loop over lights                                          │
│ • Ray march toward each light                                   │
│ • Same ray marcher as everything else                           │
│                                                                  │
│ Problem: UGC → players place 100,000+ lights                    │
│                                                                  │
│ Future: Looking into ReSTIR DI                                  │
│ • Light tree for importance sampling                            │
│ • Reservoir resampling between pixels                           │
│ • Early tests "mildly promising" but too noisy                  │
│                                                                  │
│ Current hack: Limit to 42 lights (silently ignore extras)       │
└─────────────────────────────────────────────────────────────────┘
```

---

## 🧪 Part 10: Reference Mode & Validation

### 10.1 Why Reference Mode is Critical

```
┌─────────────────────────────────────────────────────────────────┐
│ "Even if you cannot match reference, you NEED it to know        │
│  whether tweaks make things BETTER or WORSE"                    │
│                                                                  │
│ Without reference:                                               │
│ • Is this AO amount correct?                                    │
│ • Should shadows be softer?                                     │
│ • Is this lighting "right" or completely wrong?                 │
│                                                                  │
│ Examples shown: Same scene looked "fine" but completely wrong   │
│                 when compared to path traced reference          │
└─────────────────────────────────────────────────────────────────┘
```

### 10.2 Their Reference Implementation

```
• Path tracer in-game (not full RTX mode)
• Uses same ray marcher + ray tracer
• ~3 bounces (enough for outdoor scenes)
• Verified against Mitsuba renderer
• Screen-space reference for quick iteration
```

---

## 🔗 Part 11: How Everything Connects

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           THE UNIFIED VISION                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│                        ┌─────────────────────┐                              │
│                        │    RAY MARCHING     │                              │
│                        │  (The One Technique)│                              │
│                        └──────────┬──────────┘                              │
│                                   │                                          │
│       ┌───────────────────────────┼───────────────────────────┐             │
│       │                           │                           │             │
│       ▼                           ▼                           ▼             │
│  ┌─────────┐               ┌─────────────┐              ┌─────────┐        │
│  │ SHADOWS │               │     GI      │              │   DoF   │        │
│  │ Contact │               │  Fallback   │              │  Novel  │        │
│  └─────────┘               │  to RT      │              │ Approach│        │
│       │                    └─────────────┘              └─────────┘        │
│       │                           │                           │             │
│       │                    ┌──────┴──────┐                    │             │
│       │                    │             │                    │             │
│       ▼                    ▼             ▼                    ▼             │
│  ┌─────────┐          ┌─────────┐  ┌─────────┐         ┌─────────┐        │
│  │REFLEC-  │          │ WATER   │  │   ICE   │         │ LOCAL   │        │
│  │TIONS    │          │ Reflect │  │ Refract │         │ LIGHTS  │        │
│  │ SSR     │          │ Refract │  │         │         │         │        │
│  └─────────┘          └─────────┘  └─────────┘         └─────────┘        │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                     LINEAR + POINT SAMPLING                           │  │
│  │                     (Used by ALL ray marching)                        │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                    TEMPORAL ANTI-ALIASING                             │  │
│  │              (Shadows, GI, Reflections, DoF output)                   │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                     GPU-DRIVEN EVERYTHING                             │  │
│  │               (Culling, sorting, LOD, draw generation)                │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 📋 Implementation Checklist

### Phase 1: Foundation
- [ ] GPU-driven rendering system (NITE equivalent)
- [ ] Material vertex buffer packing
- [ ] Two-pass occlusion culling
- [ ] Hierarchical depth pyramid (Hi-Z)
- [ ] Draw call generation (simple + sorted paths)

### Phase 2: Shadows
- [ ] PCF soft shadows
- [ ] PCSS contact hardening
- [ ] **Temporal shadow AA** (variance tracking)
- [ ] **Contact shadow ray marching**
- [ ] **Linear + point sampling trick**

### Phase 3: Global Illumination
- [ ] Software BVH ray tracing (obvhs crate)
- [ ] Hybrid ray march → ray trace fallback
- [ ] Quarter-res tracing
- [ ] SH projection and denoising
- [ ] **Stabilizing recurrent blur**
- [ ] **SSAO as denoise weight**
- [ ] XeGTAO integration ✅ (already added!)

### Phase 4: Water/Ice
- [ ] SSR with miss compaction
- [ ] Half-res ray trace fallback
- [ ] Quad hit hallucination
- [ ] Fake refraction (distorted ray march)
- [ ] Fake caustics (threshold projection)
- [ ] Ice with rough reflections

### Phase 5: Post Processing
- [ ] **Ray marched DoF** (radial slice optimization)
- [ ] Tile classification for DoF
- [ ] **tony-mc-mapface tonemapper**
- [ ] Final TAA pass

### Phase 6: Polish
- [ ] Reference mode (path tracer)
- [ ] Performance profiling
- [ ] Potato GPU testing

---

## 🔗 Resources

| Resource | URL |
|----------|-----|
| Linear+Point Sampling Code | https://gist.github.com/h3r2tic |
| DoF Shader Code | https://gist.github.com/h3r2tic |
| tony-mc-mapface | https://github.com/h3r2tic/tony-mc-mapface |
| obvhs (Rust BVH) | https://crates.io/crates/obvhs |
| XeGTAO | https://github.com/GameTechDev/XeGTAO |
| CUDA Path Tracer (BVH ref) | Ylitie/Karras wide BVH |
| AgX Tonemapper | Troy Sobotka |
| Stabilizing Recurrent Blur | DZY paper |
| Marc Salvi's Variance TAA | SIGGRAPH/GDC archives |

---

## 💡 Key Philosophies

1. **"Solve all problems with ray marching"** - One technique, many applications
2. **"Full potato mode wins"** - Simpler often beats complex (ReSTIR → brute force)
3. **"UGC is worst-case generator"** - If players CAN break it, they WILL
4. **"Reference mode is mandatory"** - Can't optimize what you can't measure
5. **"Render target as painting"** - Not photorealism, artistic medium
6. **"Rust is superpower"** - Zero crashes, fast iteration via crates
7. **"Jake warning"** - If it sounds wrong, it probably is (not enough research time)
