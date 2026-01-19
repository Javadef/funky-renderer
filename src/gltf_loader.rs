use std::path::Path;
use std::io::BufReader;
use std::fs::File;

#[derive(Clone, Debug)]
pub struct GltfVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],
    pub color: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct GltfMesh {
    pub vertices: Vec<GltfVertex>,
    pub indices: Vec<u32>,
    pub material_index: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct GltfMaterial {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub base_color_texture_index: Option<usize>,
}

impl Default for GltfMaterial {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 1.0,
            base_color_texture_index: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GltfTexture {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,  // RGBA8
}

#[derive(Debug)]
pub struct GltfScene {
    pub meshes: Vec<GltfMesh>,
    pub materials: Vec<GltfMaterial>,
    pub textures: Vec<GltfTexture>,
    /// Axis-aligned bounds (model space) across all mesh vertex positions.
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
}

impl GltfScene {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let file = File::open(path.as_ref())?;
        let reader = BufReader::new(file);
        let gltf = gltf::Gltf::from_reader(reader)?;
        
        // Get the directory containing the gltf file for loading buffers
        let base_path = path.as_ref().parent().unwrap_or(Path::new(""));
        
        // Load all buffer data
        let mut buffer_data = Vec::new();
        for buffer in gltf.buffers() {
            match buffer.source() {
                gltf::buffer::Source::Uri(uri) => {
                    if uri.starts_with("data:") {
                        return Err("Embedded data URIs not yet supported".into());
                    } else {
                        let buffer_path = base_path.join(uri);
                        let data = std::fs::read(buffer_path)?;
                        buffer_data.push(data);
                    }
                }
                gltf::buffer::Source::Bin => {
                    if let Some(blob) = gltf.blob.as_ref() {
                        buffer_data.push(blob.clone());
                    } else {
                        return Err("Missing binary blob for GLB file".into());
                    }
                }
            }
        }
        
        // Load textures
        let mut textures = Vec::new();
        for image in gltf.images() {
            match image.source() {
                gltf::image::Source::Uri { uri, .. } => {
                    if uri.starts_with("data:") {
                        println!("  ⚠ Embedded texture data URIs not yet supported");
                        continue;
                    }
                    let image_path = base_path.join(uri);
                    println!("  📷 Loading texture: {}", uri);
                    
                    let img = image::open(&image_path)?;
                    let rgba = img.to_rgba8();
                    let (width, height) = rgba.dimensions();
                    
                    textures.push(GltfTexture {
                        width,
                        height,
                        data: rgba.into_raw(),
                    });
                }
                gltf::image::Source::View { view, .. } => {
                    let buffer_idx = view.buffer().index();
                    let offset = view.offset();
                    let length = view.length();
                    let data = &buffer_data[buffer_idx][offset..offset + length];
                    
                    let img = image::load_from_memory(data)?;
                    let rgba = img.to_rgba8();
                    let (width, height) = rgba.dimensions();
                    
                    textures.push(GltfTexture {
                        width,
                        height,
                        data: rgba.into_raw(),
                    });
                }
            }
        }
        
        // Load materials
        let mut materials = Vec::new();
        for material in gltf.materials() {
            let pbr = material.pbr_metallic_roughness();
            let base_color = pbr.base_color_factor();
            let metallic = pbr.metallic_factor();
            let roughness = pbr.roughness_factor();
            
            // Get texture index if available
            let base_color_texture_index = pbr.base_color_texture().map(|info| {
                info.texture().index()
            });
            
            materials.push(GltfMaterial {
                base_color,
                metallic,
                roughness,
                base_color_texture_index,
            });
        }
        
        // If no materials, add a default one
        if materials.is_empty() {
            materials.push(GltfMaterial::default());
        }
        
        // Load meshes
        let mut meshes = Vec::new();

        let mut bounds_min = [f32::INFINITY, f32::INFINITY, f32::INFINITY];
        let mut bounds_max = [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
        
        for mesh in gltf.meshes() {
            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffer_data[buffer.index()]));
                
                // Read positions
                let positions: Vec<[f32; 3]> = reader
                    .read_positions()
                    .map(|iter| iter.collect())
                    .unwrap_or_default();

                // Update bounds
                for p in &positions {
                    bounds_min[0] = bounds_min[0].min(p[0]);
                    bounds_min[1] = bounds_min[1].min(p[1]);
                    bounds_min[2] = bounds_min[2].min(p[2]);
                    bounds_max[0] = bounds_max[0].max(p[0]);
                    bounds_max[1] = bounds_max[1].max(p[1]);
                    bounds_max[2] = bounds_max[2].max(p[2]);
                }
                
                // Read normals
                let normals: Vec<[f32; 3]> = reader
                    .read_normals()
                    .map(|iter| iter.collect())
                    .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
                
                // Read texture coordinates
                let tex_coords: Vec<[f32; 2]> = reader
                    .read_tex_coords(0)
                    .map(|coords| coords.into_f32().collect())
                    .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
                
                // Read colors (if available)
                let colors: Vec<[f32; 3]> = reader
                    .read_colors(0)
                    .map(|colors| {
                        colors.into_rgb_f32().map(|c| [c[0], c[1], c[2]]).collect()
                    })
                    .unwrap_or_else(|| vec![[1.0, 1.0, 1.0]; positions.len()]);
                
                // Combine into vertices
                let vertices: Vec<GltfVertex> = positions
                    .iter()
                    .zip(normals.iter())
                    .zip(tex_coords.iter())
                    .zip(colors.iter())
                    .map(|(((pos, norm), tex), col)| GltfVertex {
                        position: *pos,
                        normal: *norm,
                        tex_coord: *tex,
                        color: *col,
                    })
                    .collect();
                
                // Read indices
                let indices: Vec<u32> = reader
                    .read_indices()
                    .map(|indices| indices.into_u32().collect())
                    .unwrap_or_else(|| (0..vertices.len() as u32).collect());
                
                let material_index = primitive.material().index();
                
                meshes.push(GltfMesh {
                    vertices,
                    indices,
                    material_index,
                });
            }
        }
        
        println!("  ✓ Loaded {} meshes, {} materials, {} textures", 
                 meshes.len(), materials.len(), textures.len());
        
        // If the model had no positions, provide safe defaults.
        if !bounds_min[0].is_finite() {
            bounds_min = [0.0, 0.0, 0.0];
            bounds_max = [0.0, 0.0, 0.0];
        }

        Ok(GltfScene {
            meshes,
            materials,
            textures,
            bounds_min,
            bounds_max,
        })
    }
}
