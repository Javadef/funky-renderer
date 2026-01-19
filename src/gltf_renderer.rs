use ash::vk;
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme};
use gpu_allocator::MemoryLocation;
use crate::renderer::{VulkanRenderer, UniformBufferObject, MAX_FRAMES_IN_FLIGHT};
use crate::gltf_loader::GltfScene;
use std::ffi::CString;

// Vertex format for glTF with tex coords
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GltfVertex {
    pub pos: [f32; 3],
    pub color: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],
}

pub struct GltfRenderer {
    pub meshes: Vec<GltfMeshBuffers>,
    pub texture: Option<TextureResources>,
    pub pipeline: vk::Pipeline,
    pub pipeline_layout: vk::PipelineLayout,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptor_sets: Vec<vk::DescriptorSet>,
    pub uniform_buffers: Vec<vk::Buffer>,
    pub uniform_allocations: Vec<Option<Allocation>>,
    pub depth_images: Vec<vk::Image>,
    pub depth_image_views: Vec<vk::ImageView>,
    pub depth_allocations: Vec<Option<Allocation>>,
    pub render_pass: vk::RenderPass,
    pub framebuffers: Vec<vk::Framebuffer>,
}

pub struct GltfMeshBuffers {
    pub vertex_buffer: vk::Buffer,
    pub vertex_allocation: Option<Allocation>,
    pub index_buffer: vk::Buffer,
    pub index_allocation: Option<Allocation>,
    pub index_count: u32,
}

pub struct TextureResources {
    pub image: vk::Image,
    pub image_view: vk::ImageView,
    pub sampler: vk::Sampler,
    pub allocation: Option<Allocation>,
}

impl GltfRenderer {
    pub unsafe fn new(
        renderer: &VulkanRenderer,
        scene: &GltfScene,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Create one depth buffer per swapchain image
        let depth_format = vk::Format::D32_SFLOAT;
        let image_count = renderer.swapchain_image_views.len();
        let mut depth_images = Vec::new();
        let mut depth_image_views = Vec::new();
        let mut depth_allocations = Vec::new();
        
        for _i in 0..image_count {
            let (depth_image, depth_image_view, depth_allocation) = Self::create_depth_resources(
                renderer,
                renderer.swapchain_extent.width,
                renderer.swapchain_extent.height,
                depth_format,
            )?;
            depth_images.push(depth_image);
            depth_image_views.push(depth_image_view);
            depth_allocations.push(Some(depth_allocation));
        }
        
        // Create render pass with depth attachment
        let render_pass = Self::create_render_pass(&renderer.device, renderer.swapchain_format, depth_format)?;
        
        // Create framebuffers with depth attachment (one per swapchain image with its own depth)
        let mut framebuffers = Vec::new();
        for (i, &color_view) in renderer.swapchain_image_views.iter().enumerate() {
            let attachments = [color_view, depth_image_views[i]];
            let framebuffer_info = vk::FramebufferCreateInfo::default()
                .render_pass(render_pass)
                .attachments(&attachments)
                .width(renderer.swapchain_extent.width)
                .height(renderer.swapchain_extent.height)
                .layers(1);
            framebuffers.push(renderer.device.create_framebuffer(&framebuffer_info, None)?);
        }
        
        // Load texture if available
        let texture = if !scene.textures.is_empty() {
            Some(Self::create_texture(renderer, &scene.textures[0])?)
        } else {
            // Create a white 1x1 fallback texture
            Some(Self::create_fallback_texture(renderer)?)
        };
        
        // Create descriptor set layout (UBO + sampler)
        let ubo_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT);
        
        let sampler_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(1)
            .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::FRAGMENT);
        
        let bindings = [ubo_binding, sampler_binding];
        let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
        let descriptor_set_layout = renderer.device.create_descriptor_set_layout(&layout_info, None)?;
        
        // Create pipeline layout
        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(std::slice::from_ref(&descriptor_set_layout));
        let pipeline_layout = renderer.device.create_pipeline_layout(&pipeline_layout_info, None)?;
        
        // Create pipeline
        let pipeline = Self::create_pipeline(&renderer.device, render_pass, pipeline_layout)?;
        
        // Create descriptor pool
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: MAX_FRAMES_IN_FLIGHT as u32,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: MAX_FRAMES_IN_FLIGHT as u32,
            },
        ];
        
        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(&pool_sizes)
            .max_sets(MAX_FRAMES_IN_FLIGHT as u32);
        let descriptor_pool = renderer.device.create_descriptor_pool(&pool_info, None)?;
        
        // Create uniform buffers and descriptor sets
        let mut uniform_buffers = Vec::new();
        let mut uniform_allocations = Vec::new();
        let ubo_size = std::mem::size_of::<UniformBufferObject>() as u64;
        
        let layouts = vec![descriptor_set_layout; MAX_FRAMES_IN_FLIGHT];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&layouts);
        let descriptor_sets = renderer.device.allocate_descriptor_sets(&alloc_info)?;
        
        for i in 0..MAX_FRAMES_IN_FLIGHT {
            // Create uniform buffer
            let buffer_info = vk::BufferCreateInfo::default()
                .size(ubo_size)
                .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            
            let buffer = renderer.device.create_buffer(&buffer_info, None)?;
            let requirements = renderer.device.get_buffer_memory_requirements(buffer);
            
            let allocation = renderer.allocator.lock().allocate(&AllocationCreateDesc {
                name: &format!("glTF Uniform Buffer {}", i),
                requirements,
                location: MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })?;
            
            renderer.device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())?;
            
            uniform_buffers.push(buffer);
            uniform_allocations.push(Some(allocation));
            
            // Update descriptor sets
            let buffer_info_desc = vk::DescriptorBufferInfo {
                buffer,
                offset: 0,
                range: ubo_size,
            };
            
            let image_info = vk::DescriptorImageInfo {
                sampler: texture.as_ref().unwrap().sampler,
                image_view: texture.as_ref().unwrap().image_view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            };
            
            let descriptor_writes = [
                vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_sets[i])
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                    .buffer_info(std::slice::from_ref(&buffer_info_desc)),
                vk::WriteDescriptorSet::default()
                    .dst_set(descriptor_sets[i])
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(std::slice::from_ref(&image_info)),
            ];
            
            renderer.device.update_descriptor_sets(&descriptor_writes, &[]);
        }
        
        // Create mesh buffers
        let mut meshes = Vec::new();
        for gltf_mesh in &scene.meshes {
            let vertices: Vec<GltfVertex> = gltf_mesh
                .vertices
                .iter()
                .map(|v| {
                    let color = if let Some(mat_idx) = gltf_mesh.material_index {
                        if let Some(material) = scene.materials.get(mat_idx) {
                            [material.base_color[0], material.base_color[1], material.base_color[2]]
                        } else {
                            v.color
                        }
                    } else {
                        v.color
                    };
                    
                    GltfVertex {
                        pos: v.position,
                        color,
                        normal: v.normal,
                        tex_coord: v.tex_coord,
                    }
                })
                .collect();
            
            let indices = &gltf_mesh.indices;
            
            // Create vertex buffer
            let vertex_buffer_size = (std::mem::size_of::<GltfVertex>() * vertices.len()) as u64;
            
            let vertex_buffer_info = vk::BufferCreateInfo::default()
                .size(vertex_buffer_size)
                .usage(vk::BufferUsageFlags::VERTEX_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            
            let vertex_buffer = renderer.device.create_buffer(&vertex_buffer_info, None)?;
            let vertex_requirements = renderer.device.get_buffer_memory_requirements(vertex_buffer);
            
            let vertex_allocation = renderer.allocator.lock().allocate(&AllocationCreateDesc {
                name: "gltf_vertex_buffer",
                requirements: vertex_requirements,
                location: MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })?;
            
            renderer.device.bind_buffer_memory(
                vertex_buffer,
                vertex_allocation.memory(),
                vertex_allocation.offset(),
            )?;
            
            let vertex_data_ptr = vertex_allocation.mapped_ptr().unwrap().as_ptr() as *mut GltfVertex;
            std::ptr::copy_nonoverlapping(vertices.as_ptr(), vertex_data_ptr, vertices.len());
            
            // Create index buffer
            let index_buffer_size = (std::mem::size_of::<u32>() * indices.len()) as u64;
            
            let index_buffer_info = vk::BufferCreateInfo::default()
                .size(index_buffer_size)
                .usage(vk::BufferUsageFlags::INDEX_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            
            let index_buffer = renderer.device.create_buffer(&index_buffer_info, None)?;
            let index_requirements = renderer.device.get_buffer_memory_requirements(index_buffer);
            
            let index_allocation = renderer.allocator.lock().allocate(&AllocationCreateDesc {
                name: "gltf_index_buffer",
                requirements: index_requirements,
                location: MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })?;
            
            renderer.device.bind_buffer_memory(
                index_buffer,
                index_allocation.memory(),
                index_allocation.offset(),
            )?;
            
            let index_data_ptr = index_allocation.mapped_ptr().unwrap().as_ptr() as *mut u32;
            std::ptr::copy_nonoverlapping(indices.as_ptr(), index_data_ptr, indices.len());
            
            meshes.push(GltfMeshBuffers {
                vertex_buffer,
                vertex_allocation: Some(vertex_allocation),
                index_buffer,
                index_allocation: Some(index_allocation),
                index_count: indices.len() as u32,
            });
        }
        
        Ok(Self {
            meshes,
            texture,
            pipeline,
            pipeline_layout,
            descriptor_set_layout,
            descriptor_pool,
            descriptor_sets,
            uniform_buffers,
            uniform_allocations,
            depth_images,
            depth_image_views,
            depth_allocations,
            render_pass,
            framebuffers,
        })
    }
    
    unsafe fn create_depth_resources(
        renderer: &VulkanRenderer,
        width: u32,
        height: u32,
        format: vk::Format,
    ) -> Result<(vk::Image, vk::ImageView, Allocation), Box<dyn std::error::Error>> {
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(format)
            .extent(vk::Extent3D { width, height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        
        let image = renderer.device.create_image(&image_info, None)?;
        let requirements = renderer.device.get_image_memory_requirements(image);
        
        let allocation = renderer.allocator.lock().allocate(&AllocationCreateDesc {
            name: "depth_buffer",
            requirements,
            location: MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        
        renderer.device.bind_image_memory(image, allocation.memory(), allocation.offset())?;
        
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::DEPTH,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        
        let image_view = renderer.device.create_image_view(&view_info, None)?;
        
        Ok((image, image_view, allocation))
    }
    
    unsafe fn create_render_pass(
        device: &ash::Device,
        color_format: vk::Format,
        depth_format: vk::Format,
    ) -> Result<vk::RenderPass, vk::Result> {
        let attachments = [
            // Color attachment
            vk::AttachmentDescription::default()
                .format(color_format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::STORE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::PRESENT_SRC_KHR),
            // Depth attachment
            vk::AttachmentDescription::default()
                .format(depth_format)
                .samples(vk::SampleCountFlags::TYPE_1)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::DONT_CARE)
                .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
                .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
                .initial_layout(vk::ImageLayout::UNDEFINED)
                .final_layout(vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL),
        ];
        
        let color_ref = vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        };
        
        let depth_ref = vk::AttachmentReference {
            attachment: 1,
            layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        };
        
        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&color_ref))
            .depth_stencil_attachment(&depth_ref);
        
        let dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(
                vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT
                    | vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            )
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE
                    | vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            );
        
        let render_pass_info = vk::RenderPassCreateInfo::default()
            .attachments(&attachments)
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(std::slice::from_ref(&dependency));
        
        device.create_render_pass(&render_pass_info, None)
    }
    
    unsafe fn create_framebuffers(
        device: &ash::Device,
        render_pass: vk::RenderPass,
        swapchain_image_views: &[vk::ImageView],
        depth_image_view: vk::ImageView,
        extent: vk::Extent2D,
    ) -> Result<Vec<vk::Framebuffer>, vk::Result> {
        swapchain_image_views
            .iter()
            .map(|&color_view| {
                let attachments = [color_view, depth_image_view];
                let framebuffer_info = vk::FramebufferCreateInfo::default()
                    .render_pass(render_pass)
                    .attachments(&attachments)
                    .width(extent.width)
                    .height(extent.height)
                    .layers(1);
                device.create_framebuffer(&framebuffer_info, None)
            })
            .collect()
    }
    
    unsafe fn create_pipeline(
        device: &ash::Device,
        render_pass: vk::RenderPass,
        pipeline_layout: vk::PipelineLayout,
    ) -> Result<vk::Pipeline, Box<dyn std::error::Error>> {
        let vert_code = include_bytes!("../shaders/gltf.vert.spv");
        let frag_code = include_bytes!("../shaders/gltf.frag.spv");
        
        let vert_module = Self::create_shader_module(device, vert_code)?;
        let frag_module = Self::create_shader_module(device, frag_code)?;
        
        let main_name = CString::new("main")?;
        
        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vert_module)
                .name(&main_name),
            vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(frag_module)
                .name(&main_name),
        ];
        
        // Vertex input - position, color, normal, texcoord
        let binding = vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<GltfVertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX);
        
        let attributes = [
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 0, // pos
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 1,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 12, // color
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 2,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 24, // normal
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 3,
                format: vk::Format::R32G32_SFLOAT,
                offset: 36, // tex_coord
            },
        ];
        
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(std::slice::from_ref(&binding))
            .vertex_attribute_descriptions(&attributes);
        
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
        
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state = vk::PipelineDynamicStateCreateInfo::default()
            .dynamic_states(&dynamic_states);
        
        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            // Avoid 'see-through' artifacts from mismatched winding/handedness.
            // Once the camera/projection conventions are fully standardized, this can be
            // switched back to BACK with the correct front_face.
            .cull_mode(vk::CullModeFlags::NONE)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE);
        
        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        
        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);
        
        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false);
        
        let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(std::slice::from_ref(&color_blend_attachment));
        
        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .depth_stencil_state(&depth_stencil)
            .color_blend_state(&color_blending)
            .dynamic_state(&dynamic_state)
            .layout(pipeline_layout)
            .render_pass(render_pass)
            .subpass(0);
        
        let pipeline = device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
            .map_err(|(_, e)| e)?[0];
        
        device.destroy_shader_module(vert_module, None);
        device.destroy_shader_module(frag_module, None);
        
        Ok(pipeline)
    }
    
    unsafe fn create_shader_module(
        device: &ash::Device,
        code: &[u8],
    ) -> Result<vk::ShaderModule, vk::Result> {
        let code_u32: Vec<u32> = code
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        let create_info = vk::ShaderModuleCreateInfo::default().code(&code_u32);
        device.create_shader_module(&create_info, None)
    }
    
    unsafe fn create_texture(
        renderer: &VulkanRenderer,
        tex: &crate::gltf_loader::GltfTexture,
    ) -> Result<TextureResources, Box<dyn std::error::Error>> {
        let (width, height) = (tex.width, tex.height);
        let data = &tex.data;
        
        // Create staging buffer
        let buffer_size = (width * height * 4) as u64;
        let staging_buffer_info = vk::BufferCreateInfo::default()
            .size(buffer_size)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let staging_buffer = renderer.device.create_buffer(&staging_buffer_info, None)?;
        let staging_reqs = renderer.device.get_buffer_memory_requirements(staging_buffer);
        
        let staging_allocation = renderer.allocator.lock().allocate(&AllocationCreateDesc {
            name: "texture_staging",
            requirements: staging_reqs,
            location: MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        
        renderer.device.bind_buffer_memory(
            staging_buffer,
            staging_allocation.memory(),
            staging_allocation.offset(),
        )?;
        
        let ptr = staging_allocation.mapped_ptr().unwrap().as_ptr() as *mut u8;
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
        
        // Create image
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_SRGB)
            .extent(vk::Extent3D { width, height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::SAMPLED)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        
        let image = renderer.device.create_image(&image_info, None)?;
        let image_reqs = renderer.device.get_image_memory_requirements(image);
        
        let image_allocation = renderer.allocator.lock().allocate(&AllocationCreateDesc {
            name: "texture_image",
            requirements: image_reqs,
            location: MemoryLocation::GpuOnly,
            linear: false,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        
        renderer.device.bind_image_memory(image, image_allocation.memory(), image_allocation.offset())?;
        
        // Copy staging buffer to image
        Self::transition_image_layout(
            renderer,
            image,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        )?;
        
        Self::copy_buffer_to_image(renderer, staging_buffer, image, width, height)?;
        
        Self::transition_image_layout(
            renderer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )?;
        
        // Clean up staging
        renderer.device.destroy_buffer(staging_buffer, None);
        renderer.allocator.lock().free(staging_allocation)?;
        
        // Create image view
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_SRGB)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        
        let image_view = renderer.device.create_image_view(&view_info, None)?;
        
        // Create sampler
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::REPEAT)
            .address_mode_v(vk::SamplerAddressMode::REPEAT)
            .address_mode_w(vk::SamplerAddressMode::REPEAT)
            .anisotropy_enable(false)
            .border_color(vk::BorderColor::INT_OPAQUE_BLACK)
            .unnormalized_coordinates(false)
            .compare_enable(false)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR);
        
        let sampler = renderer.device.create_sampler(&sampler_info, None)?;
        
        Ok(TextureResources {
            image,
            image_view,
            sampler,
            allocation: Some(image_allocation),
        })
    }
    
    unsafe fn create_fallback_texture(
        renderer: &VulkanRenderer,
    ) -> Result<TextureResources, Box<dyn std::error::Error>> {
        let tex = crate::gltf_loader::GltfTexture {
            width: 1,
            height: 1,
            data: vec![255, 255, 255, 255],
        };
        Self::create_texture(renderer, &tex)
    }
    
    unsafe fn transition_image_layout(
        renderer: &VulkanRenderer,
        image: vk::Image,
        old_layout: vk::ImageLayout,
        new_layout: vk::ImageLayout,
    ) -> Result<(), vk::Result> {
        let cmd_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(renderer.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        
        let cmd = renderer.device.allocate_command_buffers(&cmd_info)?[0];
        
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        renderer.device.begin_command_buffer(cmd, &begin_info)?;
        
        let (src_access, dst_access, src_stage, dst_stage) = match (old_layout, new_layout) {
            (vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL) => (
                vk::AccessFlags::empty(),
                vk::AccessFlags::TRANSFER_WRITE,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
            ),
            (vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
                vk::AccessFlags::TRANSFER_WRITE,
                vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
            ),
            _ => (
                vk::AccessFlags::empty(),
                vk::AccessFlags::empty(),
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TOP_OF_PIPE,
            ),
        };
        
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(old_layout)
            .new_layout(new_layout)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .image(image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })
            .src_access_mask(src_access)
            .dst_access_mask(dst_access);
        
        renderer.device.cmd_pipeline_barrier(
            cmd,
            src_stage,
            dst_stage,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[barrier],
        );
        
        renderer.device.end_command_buffer(cmd)?;
        
        let submit_info = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&cmd));
        renderer.device.queue_submit(renderer.graphics_queue, &[submit_info], vk::Fence::null())?;
        renderer.device.queue_wait_idle(renderer.graphics_queue)?;
        
        renderer.device.free_command_buffers(renderer.command_pool, &[cmd]);
        
        Ok(())
    }
    
    unsafe fn copy_buffer_to_image(
        renderer: &VulkanRenderer,
        buffer: vk::Buffer,
        image: vk::Image,
        width: u32,
        height: u32,
    ) -> Result<(), vk::Result> {
        let cmd_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(renderer.command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        
        let cmd = renderer.device.allocate_command_buffers(&cmd_info)?[0];
        
        let begin_info = vk::CommandBufferBeginInfo::default()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        renderer.device.begin_command_buffer(cmd, &begin_info)?;
        
        let region = vk::BufferImageCopy {
            buffer_offset: 0,
            buffer_row_length: 0,
            buffer_image_height: 0,
            image_subresource: vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
            image_extent: vk::Extent3D { width, height, depth: 1 },
        };
        
        renderer.device.cmd_copy_buffer_to_image(
            cmd,
            buffer,
            image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[region],
        );
        
        renderer.device.end_command_buffer(cmd)?;
        
        let submit_info = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&cmd));
        renderer.device.queue_submit(renderer.graphics_queue, &[submit_info], vk::Fence::null())?;
        renderer.device.queue_wait_idle(renderer.graphics_queue)?;
        
        renderer.device.free_command_buffers(renderer.command_pool, &[cmd]);
        
        Ok(())
    }
    
    pub unsafe fn update_uniform_buffer(
        &self,
        current_frame: usize,
        position: glam::Vec3,
        camera_pos: glam::Vec3,
        camera_yaw: f32,
        camera_pitch: f32,
        camera_fov: f32,
        scale: f32,
        aspect_ratio: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Calculate camera direction from yaw and pitch
        let camera_front = glam::Vec3::new(
            camera_yaw.cos() * camera_pitch.cos(),
            camera_pitch.sin(),
            camera_yaw.sin() * camera_pitch.cos(),
        ).normalize();
        
        let target = camera_pos + camera_front;
        
        let model = glam::Mat4::from_translation(position)
            * glam::Mat4::from_scale(glam::Vec3::splat(scale));
        
        let view = glam::Mat4::look_at_rh(camera_pos, target, glam::Vec3::Y);

        // Vulkan clip space has inverted Y compared to the typical math conventions used by
        // many helper functions. Flip Y so "up" on input corresponds to "up" on screen.
        let mut proj = glam::Mat4::perspective_rh(camera_fov, aspect_ratio, 0.1, 100.0);
        proj.y_axis.y *= -1.0;
        
        let ubo = UniformBufferObject {
            model,
            view,
            proj,
            camera_pos: glam::Vec4::new(camera_pos.x, camera_pos.y, camera_pos.z, 0.0),
            light_dir: glam::Vec4::new(0.5, 1.0, 0.3, 0.0).normalize(),
        };
        
        if let Some(allocation) = &self.uniform_allocations[current_frame] {
            let ptr = allocation.mapped_ptr().unwrap().as_ptr() as *mut UniformBufferObject;
            std::ptr::copy_nonoverlapping(&ubo, ptr, 1);
        }
        
        Ok(())
    }
    
    pub unsafe fn render(
        &self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
        extent: vk::Extent2D,
        image_index: u32,
        current_frame: usize,
    ) {
        // Begin render pass
        let clear_values = [
            vk::ClearValue {
                color: vk::ClearColorValue { float32: [0.53, 0.81, 0.92, 1.0] },
            },
            vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue { depth: 1.0, stencil: 0 },
            },
        ];
        
        let render_pass_info = vk::RenderPassBeginInfo::default()
            .render_pass(self.render_pass)
            .framebuffer(self.framebuffers[image_index as usize])
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent,
            })
            .clear_values(&clear_values);
        
        device.cmd_begin_render_pass(command_buffer, &render_pass_info, vk::SubpassContents::INLINE);
        
        // Bind pipeline
        device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
        
        // Set viewport and scissor
        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: extent.width as f32,
            height: extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        device.cmd_set_viewport(command_buffer, 0, &[viewport]);
        
        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent,
        };
        device.cmd_set_scissor(command_buffer, 0, &[scissor]);
        
        // Bind descriptor set
        device.cmd_bind_descriptor_sets(
            command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.pipeline_layout,
            0,
            &[self.descriptor_sets[current_frame]],
            &[],
        );
        
        // Draw all meshes
        for mesh in &self.meshes {
            device.cmd_bind_vertex_buffers(command_buffer, 0, &[mesh.vertex_buffer], &[0]);
            device.cmd_bind_index_buffer(command_buffer, mesh.index_buffer, 0, vk::IndexType::UINT32);
            device.cmd_draw_indexed(command_buffer, mesh.index_count, 1, 0, 0, 0);
        }
    }
    
    pub unsafe fn end_render_pass(&self, device: &ash::Device, command_buffer: vk::CommandBuffer) {
        device.cmd_end_render_pass(command_buffer);
    }
    
    pub unsafe fn cleanup(&mut self, renderer: &VulkanRenderer) {
        // Cleanup meshes
        for mesh in &mut self.meshes {
            renderer.device.destroy_buffer(mesh.vertex_buffer, None);
            if let Some(allocation) = mesh.vertex_allocation.take() {
                let _ = renderer.allocator.lock().free(allocation);
            }
            
            renderer.device.destroy_buffer(mesh.index_buffer, None);
            if let Some(allocation) = mesh.index_allocation.take() {
                let _ = renderer.allocator.lock().free(allocation);
            }
        }
        
        // Cleanup texture
        if let Some(tex) = &mut self.texture {
            renderer.device.destroy_sampler(tex.sampler, None);
            renderer.device.destroy_image_view(tex.image_view, None);
            renderer.device.destroy_image(tex.image, None);
            if let Some(allocation) = tex.allocation.take() {
                let _ = renderer.allocator.lock().free(allocation);
            }
        }
        
        // Cleanup uniform buffers
        for (buffer, allocation) in self.uniform_buffers.iter().zip(self.uniform_allocations.iter_mut()) {
            renderer.device.destroy_buffer(*buffer, None);
            if let Some(alloc) = allocation.take() {
                let _ = renderer.allocator.lock().free(alloc);
            }
        }
        
        // Cleanup depth resources (one per swapchain image)
        for ((&image, &view), allocation) in self.depth_images.iter()
            .zip(self.depth_image_views.iter())
            .zip(self.depth_allocations.iter_mut())
        {
            renderer.device.destroy_image_view(view, None);
            renderer.device.destroy_image(image, None);
            if let Some(alloc) = allocation.take() {
                let _ = renderer.allocator.lock().free(alloc);
            }
        }
        
        // Cleanup framebuffers
        for &fb in &self.framebuffers {
            renderer.device.destroy_framebuffer(fb, None);
        }
        
        // Cleanup pipeline and layout
        renderer.device.destroy_pipeline(self.pipeline, None);
        renderer.device.destroy_pipeline_layout(self.pipeline_layout, None);
        renderer.device.destroy_render_pass(self.render_pass, None);
        renderer.device.destroy_descriptor_pool(self.descriptor_pool, None);
        renderer.device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
    }
    
    pub unsafe fn recreate_swapchain_resources(
        &mut self,
        renderer: &VulkanRenderer,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Cleanup old framebuffers
        for &fb in &self.framebuffers {
            renderer.device.destroy_framebuffer(fb, None);
        }
        
        // Cleanup old depth resources (one per swapchain image)
        for ((&image, &view), allocation) in self.depth_images.iter()
            .zip(self.depth_image_views.iter())
            .zip(self.depth_allocations.iter_mut())
        {
            renderer.device.destroy_image_view(view, None);
            renderer.device.destroy_image(image, None);
            if let Some(alloc) = allocation.take() {
                renderer.allocator.lock().free(alloc)?;
            }
        }
        
        // Recreate depth resources (one per swapchain image)
        let depth_format = vk::Format::D32_SFLOAT;
        let image_count = renderer.swapchain_image_views.len();
        self.depth_images.clear();
        self.depth_image_views.clear();
        self.depth_allocations.clear();
        
        for _i in 0..image_count {
            let (depth_image, depth_image_view, depth_allocation) = Self::create_depth_resources(
                renderer,
                renderer.swapchain_extent.width,
                renderer.swapchain_extent.height,
                depth_format,
            )?;
            self.depth_images.push(depth_image);
            self.depth_image_views.push(depth_image_view);
            self.depth_allocations.push(Some(depth_allocation));
        }
        
        // Recreate framebuffers (each with its own depth image view)
        self.framebuffers.clear();
        for (i, &color_view) in renderer.swapchain_image_views.iter().enumerate() {
            let attachments = [color_view, self.depth_image_views[i]];
            let framebuffer_info = vk::FramebufferCreateInfo::default()
                .render_pass(self.render_pass)
                .attachments(&attachments)
                .width(renderer.swapchain_extent.width)
                .height(renderer.swapchain_extent.height)
                .layers(1);
            self.framebuffers.push(renderer.device.create_framebuffer(&framebuffer_info, None)?);
        }
        
        Ok(())
    }
}

impl Drop for GltfRenderer {
    fn drop(&mut self) {
        // Allocations will be cleaned up by cleanup()
    }
}
