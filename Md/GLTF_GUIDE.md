# glTF Scene Rendering Guide

## Overview

Your Funky Renderer now supports loading and rendering glTF 2.0 scenes! glTF (GL Transmission Format) is a royalty-free standard for 3D models widely supported by modeling tools like Blender.

## Features

- ✅ Load glTF (.gltf) and GLB (.glb) files
- ✅ Multiple mesh support
- ✅ Vertex positions, normals, colors, and texture coordinates
- ✅ Material support (PBR metallic-roughness)
- ✅ Index buffer optimization
- ✅ Automatic format conversion to Vulkan rendering

## Quick Start

### 1. Prepare a glTF Model

Place your glTF model file in one of these locations:
- `models/scene.gltf`
- `models/model.gltf`
- `scene.gltf` (project root)
- `model.gltf` (project root)

The renderer will automatically detect and load the first available model.

### 2. Run the Renderer

```powershell
cargo run --release
```

The glTF model will render alongside the spinning cube!

## Getting glTF Models

### Free glTF Models
- [Sketchfab](https://sketchfab.com/3d-models?features=downloadable&sort_by=-likeCount) - Filter by "Downloadable" and select glTF format
- [glTF Sample Models](https://github.com/KhronosGroup/glTF-Sample-Models) - Official test models

### Creating Your Own

**Using Blender:**
1. Create or import your 3D model in Blender
2. File → Export → glTF 2.0 (.gltf/.glb)
3. In export settings:
   - Format: Choose "glTF Separate (.gltf + .bin + textures)" or "glTF Binary (.glb)"
   - Remember Exported Properties: ✓
   - Apply Modifiers: ✓
   - Include → Normals: ✓
4. Export to `models/scene.gltf`

## Supported Features

### Currently Supported ✅
- Vertex positions
- Vertex normals
- Vertex colors
- Texture coordinates (UV maps)
- Triangle meshes with indices
- Multiple meshes per scene
- PBR materials (base color, metallic, roughness)
- External .bin files
- Binary .glb format

### Not Yet Supported ⚠️
- Textures (will be added in future)
- Animations
- Skinning/bones
- Multiple scenes
- Cameras and lights from glTF

## File Structure

```
funkyrenderer/
├── src/
│   ├── gltf_loader.rs      # Loads glTF files into memory
│   ├── gltf_renderer.rs    # Vulkan rendering for glTF meshes
│   └── main.rs             # Integrated into main loop
├── models/                 # Place your models here
│   ├── scene.gltf
│   ├── scene.bin
│   └── textures/
└── Cargo.toml              # Added gltf and image crates
```

## Example Code

To load a custom glTF path:

```rust
let scene = GltfScene::load("path/to/your/model.gltf")?;
let gltf_renderer = GltfRenderer::new(&renderer, &scene)?;
```

## Troubleshooting

**"No glTF scene loaded"**
- Make sure your .gltf file is in one of the searched paths
- Check that any external .bin files are in the same directory
- Verify the file is valid glTF 2.0 format

**Model appears black or wrong colors**
- Check if your model has vertex colors or materials defined
- Default material is white (1.0, 1.0, 1.0)

**Model not visible**
- Model might be too large/small - check scale in Blender
- Model might be at origin (0,0,0) - try repositioning camera
- Check face normals are pointing outward

## Performance Tips

- Use .glb format for faster loading (single binary file)
- Optimize mesh density in your modeling tool
- The renderer uses the same pipeline as the cube for efficiency

## Next Steps

Future enhancements planned:
- [ ] Texture mapping support
- [ ] Animation playback
- [ ] Multiple instances with transforms
- [ ] Skeletal animation
- [ ] More material properties

Enjoy rendering glTF scenes! 🎨
