//! Vulkan rendering backend for egui
//! 
//! Renders egui primitives directly using ash/Vulkan.

use ash::vk;
use std::ffi::CStr;
use std::mem::size_of;

/// Vertex for egui rendering (matches egui::epaint::Vertex)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct EguiVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

/// Push constants for egui rendering
#[repr(C)]
#[derive(Clone, Copy)]
pub struct EguiPushConstants {
    pub screen_size: [f32; 2],
}

/// Vulkan egui renderer
pub struct EguiVulkanRenderer {
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: vk::DescriptorSet,
    
    // Font texture
    font_image: vk::Image,
    font_image_memory: vk::DeviceMemory,
    font_image_view: vk::ImageView,
    font_sampler: vk::Sampler,
    
    // Vertex/index buffers
    vertex_buffer: vk::Buffer,
    vertex_buffer_memory: vk::DeviceMemory,
    vertex_buffer_size: usize,
    index_buffer: vk::Buffer,
    index_buffer_memory: vk::DeviceMemory,
    index_buffer_size: usize,
}

impl EguiVulkanRenderer {
    pub fn new(
        device: &ash::Device,
        physical_device: vk::PhysicalDevice,
        instance: &ash::Instance,
        render_pass: vk::RenderPass,
        ctx: &egui::Context,
        graphics_queue: vk::Queue,
        graphics_queue_family_index: u32,
    ) -> Self {
        unsafe {
            let memory_properties = instance.get_physical_device_memory_properties(physical_device);
            
            let pool_info = vk::CommandPoolCreateInfo::default()
                .queue_family_index(graphics_queue_family_index)
                .flags(vk::CommandPoolCreateFlags::TRANSIENT);
            let setup_command_pool = device.create_command_pool(&pool_info, None).unwrap();
            
            // Descriptor set layout
            let sampler_binding = vk::DescriptorSetLayoutBinding::default()
                .binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::FRAGMENT);
            
            let bindings = [sampler_binding];
            let layout_info = vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings);
            let descriptor_set_layout = device.create_descriptor_set_layout(&layout_info, None).unwrap();
            
            // Pipeline layout
            let push_constant_range = vk::PushConstantRange::default()
                .stage_flags(vk::ShaderStageFlags::VERTEX)
                .offset(0)
                .size(size_of::<EguiPushConstants>() as u32);
            
            let push_constant_ranges = [push_constant_range];
            let set_layouts = [descriptor_set_layout];
            let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
                .set_layouts(&set_layouts)
                .push_constant_ranges(&push_constant_ranges);
            let pipeline_layout = device.create_pipeline_layout(&pipeline_layout_info, None).unwrap();
            
            // Load compiled SPIR-V shaders
            let vert_code = load_spirv_file(include_bytes!("../shaders/egui.vert.spv"));
            let frag_code = load_spirv_file(include_bytes!("../shaders/egui.frag.spv"));
            
            let vert_module_info = vk::ShaderModuleCreateInfo::default().code(&vert_code);
            let frag_module_info = vk::ShaderModuleCreateInfo::default().code(&frag_code);
            let vert_shader = device.create_shader_module(&vert_module_info, None).unwrap();
            let frag_shader = device.create_shader_module(&frag_module_info, None).unwrap();
            
            let shader_entry_name = CStr::from_bytes_with_nul_unchecked(b"main\0");
            let vert_stage = vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vert_shader)
                .name(shader_entry_name);
            let frag_stage = vk::PipelineShaderStageCreateInfo::default()
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(frag_shader)
                .name(shader_entry_name);
            let shader_stages = [vert_stage, frag_stage];
            
            // Vertex input
            let binding_desc = vk::VertexInputBindingDescription::default()
                .binding(0)
                .stride(size_of::<EguiVertex>() as u32)
                .input_rate(vk::VertexInputRate::VERTEX);
            
            let position_attr = vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(0)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(0);
            let uv_attr = vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(1)
                .format(vk::Format::R32G32_SFLOAT)
                .offset(8);
            let color_attr = vk::VertexInputAttributeDescription::default()
                .binding(0)
                .location(2)
                .format(vk::Format::R8G8B8A8_UNORM)
                .offset(16);
            
            let binding_descs = [binding_desc];
            let attr_descs = [position_attr, uv_attr, color_attr];
            let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default()
                .vertex_binding_descriptions(&binding_descs)
                .vertex_attribute_descriptions(&attr_descs);
            
            let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
                .topology(vk::PrimitiveTopology::TRIANGLE_LIST);
            
            let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state_info = vk::PipelineDynamicStateCreateInfo::default().dynamic_states(&dynamic_states);
            
            let viewport_state = vk::PipelineViewportStateCreateInfo::default()
                .viewport_count(1)
                .scissor_count(1);
            
            let rasterization = vk::PipelineRasterizationStateCreateInfo::default()
                .polygon_mode(vk::PolygonMode::FILL)
                .line_width(1.0)
                .cull_mode(vk::CullModeFlags::NONE)
                .front_face(vk::FrontFace::COUNTER_CLOCKWISE);
            
            let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);
            
            // Alpha blending
            let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
                .blend_enable(true)
                .src_color_blend_factor(vk::BlendFactor::ONE)
                .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .color_blend_op(vk::BlendOp::ADD)
                .src_alpha_blend_factor(vk::BlendFactor::ONE)
                .dst_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                .alpha_blend_op(vk::BlendOp::ADD)
                .color_write_mask(vk::ColorComponentFlags::RGBA);
            
            let attachments = [color_blend_attachment];
            let color_blending = vk::PipelineColorBlendStateCreateInfo::default().attachments(&attachments);
            
            let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
                .depth_test_enable(false)
                .depth_write_enable(false);
            
            let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
                .stages(&shader_stages)
                .vertex_input_state(&vertex_input_info)
                .input_assembly_state(&input_assembly)
                .viewport_state(&viewport_state)
                .rasterization_state(&rasterization)
                .multisample_state(&multisampling)
                .color_blend_state(&color_blending)
                .depth_stencil_state(&depth_stencil)
                .dynamic_state(&dynamic_state_info)
                .layout(pipeline_layout)
                .render_pass(render_pass)
                .subpass(0);
            
            let pipeline = device.create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None).unwrap()[0];
            
            device.destroy_shader_module(vert_shader, None);
            device.destroy_shader_module(frag_shader, None);
            
            // Create font texture
            let (font_image_vk, font_image_memory, font_image_view, font_sampler) = {
                let font_image = ctx.fonts(|fonts| {
                    let image = fonts.image();
                    let pixels: Vec<u8> = image.pixels.iter().flat_map(|&r| {
                        let byte = (r * 255.0) as u8;
                        [255u8, 255u8, 255u8, byte]
                    }).collect();
                    (image.width() as u32, image.height() as u32, pixels)
                });
                
                create_font_texture(device, &memory_properties, font_image.0, font_image.1, &font_image.2, 
                                   setup_command_pool, graphics_queue)
            };
            
            device.destroy_command_pool(setup_command_pool, None);
            
            // Descriptor pool and set
            let pool_sizes = [vk::DescriptorPoolSize::default()
                .ty(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .descriptor_count(1)];
            let pool_info = vk::DescriptorPoolCreateInfo::default()
                .max_sets(1)
                .pool_sizes(&pool_sizes);
            let descriptor_pool = device.create_descriptor_pool(&pool_info, None).unwrap();
            
            let alloc_info = vk::DescriptorSetAllocateInfo::default()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&set_layouts);
            let descriptor_set = device.allocate_descriptor_sets(&alloc_info).unwrap()[0];
            
            let image_info = vk::DescriptorImageInfo::default()
                .sampler(font_sampler)
                .image_view(font_image_view)
                .image_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            let image_infos = [image_info];
            let write_set = vk::WriteDescriptorSet::default()
                .dst_set(descriptor_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&image_infos);
            device.update_descriptor_sets(&[write_set], &[]);
            
            // Buffers
            let (vertex_buffer, vertex_buffer_memory) = create_buffer(
                device, &memory_properties, 1024 * 1024,
                vk::BufferUsageFlags::VERTEX_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            );
            
            let (index_buffer, index_buffer_memory) = create_buffer(
                device, &memory_properties, 512 * 1024,
                vk::BufferUsageFlags::INDEX_BUFFER,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
            );
            
            Self {
                pipeline_layout,
                pipeline,
                descriptor_set_layout,
                descriptor_pool,
                descriptor_set,
                font_image: font_image_vk,
                font_image_memory,
                font_image_view,
                font_sampler,
                vertex_buffer,
                vertex_buffer_memory,
                vertex_buffer_size: 1024 * 1024,
                index_buffer,
                index_buffer_memory,
                index_buffer_size: 512 * 1024,
            }
        }
    }
    
    pub fn render(
        &mut self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
        screen_width: u32,
        screen_height: u32,
        clipped_meshes: Vec<egui::ClippedPrimitive>,
        pixels_per_point: f32,
    ) {
        if clipped_meshes.is_empty() {
            return;
        }
        
        unsafe {
            let mut all_vertices: Vec<EguiVertex> = Vec::new();
            let mut all_indices: Vec<u32> = Vec::new();
            let mut mesh_infos: Vec<(usize, usize, egui::Rect)> = Vec::new();
            
            for clipped in &clipped_meshes {
                if let egui::epaint::Primitive::Mesh(mesh) = &clipped.primitive {
                    let vertex_offset = all_vertices.len();
                    let index_offset = all_indices.len();
                    
                    for v in &mesh.vertices {
                        all_vertices.push(EguiVertex {
                            pos: [v.pos.x, v.pos.y],
                            uv: [v.uv.x, v.uv.y],
                            color: v.color.to_array(),
                        });
                    }
                    
                    for idx in &mesh.indices {
                        all_indices.push(*idx + vertex_offset as u32);
                    }
                    
                    mesh_infos.push((index_offset, mesh.indices.len(), clipped.clip_rect));
                }
            }
            
            if all_vertices.is_empty() {
                return;
            }
            
            // Upload data
            let ptr = device.map_memory(self.vertex_buffer_memory, 0, 
                (all_vertices.len() * size_of::<EguiVertex>()) as u64, vk::MemoryMapFlags::empty()).unwrap() as *mut EguiVertex;
            std::ptr::copy_nonoverlapping(all_vertices.as_ptr(), ptr, all_vertices.len());
            device.unmap_memory(self.vertex_buffer_memory);
            
            let ptr = device.map_memory(self.index_buffer_memory, 0,
                (all_indices.len() * size_of::<u32>()) as u64, vk::MemoryMapFlags::empty()).unwrap() as *mut u32;
            std::ptr::copy_nonoverlapping(all_indices.as_ptr(), ptr, all_indices.len());
            device.unmap_memory(self.index_buffer_memory);
            
            // Render
            device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::GRAPHICS, self.pipeline);
            device.cmd_bind_descriptor_sets(command_buffer, vk::PipelineBindPoint::GRAPHICS,
                self.pipeline_layout, 0, &[self.descriptor_set], &[]);
            
            let push_constants = EguiPushConstants {
                screen_size: [screen_width as f32, screen_height as f32],
            };
            let push_data = std::slice::from_raw_parts(&push_constants as *const _ as *const u8, size_of::<EguiPushConstants>());
            device.cmd_push_constants(command_buffer, self.pipeline_layout, vk::ShaderStageFlags::VERTEX, 0, push_data);
            
            let viewport = vk::Viewport::default()
                .width(screen_width as f32)
                .height(screen_height as f32)
                .min_depth(0.0)
                .max_depth(1.0);
            device.cmd_set_viewport(command_buffer, 0, &[viewport]);
            
            device.cmd_bind_vertex_buffers(command_buffer, 0, &[self.vertex_buffer], &[0]);
            device.cmd_bind_index_buffer(command_buffer, self.index_buffer, 0, vk::IndexType::UINT32);
            
            for (index_offset, index_count, clip_rect) in mesh_infos {
                let min_x = (clip_rect.min.x * pixels_per_point).max(0.0) as i32;
                let min_y = (clip_rect.min.y * pixels_per_point).max(0.0) as i32;
                let max_x = (clip_rect.max.x * pixels_per_point).min(screen_width as f32) as u32;
                let max_y = (clip_rect.max.y * pixels_per_point).min(screen_height as f32) as u32;
                
                if max_x <= min_x as u32 || max_y <= min_y as u32 {
                    continue;
                }
                
                let scissor = vk::Rect2D {
                    offset: vk::Offset2D { x: min_x, y: min_y },
                    extent: vk::Extent2D { width: max_x - min_x as u32, height: max_y - min_y as u32 },
                };
                device.cmd_set_scissor(command_buffer, 0, &[scissor]);
                device.cmd_draw_indexed(command_buffer, index_count as u32, 1, index_offset as u32, 0, 0);
            }
        }
    }
    
    pub unsafe fn cleanup(&self, device: &ash::Device) {
        device.destroy_buffer(self.index_buffer, None);
        device.free_memory(self.index_buffer_memory, None);
        device.destroy_buffer(self.vertex_buffer, None);
        device.free_memory(self.vertex_buffer_memory, None);
        device.destroy_sampler(self.font_sampler, None);
        device.destroy_image_view(self.font_image_view, None);
        device.destroy_image(self.font_image, None);
        device.free_memory(self.font_image_memory, None);
        device.destroy_descriptor_pool(self.descriptor_pool, None);
        device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
        device.destroy_pipeline(self.pipeline, None);
        device.destroy_pipeline_layout(self.pipeline_layout, None);
    }
}

// Helper functions
fn create_font_texture(
    device: &ash::Device,
    memory_properties: &vk::PhysicalDeviceMemoryProperties,
    width: u32,
    height: u32,
    pixels: &[u8],
    command_pool: vk::CommandPool,
    queue: vk::Queue,
) -> (vk::Image, vk::DeviceMemory, vk::ImageView, vk::Sampler) {
    unsafe {
        let image_size = (width * height * 4) as u64;
        
        // Create staging buffer
        let staging_buffer_info = vk::BufferCreateInfo::default()
            .size(image_size)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let staging_buffer = device.create_buffer(&staging_buffer_info, None).unwrap();
        let staging_mem_requirements = device.get_buffer_memory_requirements(staging_buffer);
        
        let staging_alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(staging_mem_requirements.size)
            .memory_type_index(find_memory_type(memory_properties, staging_mem_requirements.memory_type_bits,
                vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT));
        let staging_memory = device.allocate_memory(&staging_alloc_info, None).unwrap();
        device.bind_buffer_memory(staging_buffer, staging_memory, 0).unwrap();
        
        // Upload pixels to staging buffer
        let ptr = device.map_memory(staging_memory, 0, image_size, vk::MemoryMapFlags::empty()).unwrap() as *mut u8;
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), ptr, pixels.len());
        device.unmap_memory(staging_memory);
        
        // Create image with OPTIMAL tiling (proper GPU layout)
        let image_info = vk::ImageCreateInfo::default()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .extent(vk::Extent3D { width, height, depth: 1 })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
            .sharing_mode(vk::SharingMode::EXCLUSIVE)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        
        let image = device.create_image(&image_info, None).unwrap();
        let mem_requirements = device.get_image_memory_requirements(image);
        
        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_requirements.size)
            .memory_type_index(find_memory_type(memory_properties, mem_requirements.memory_type_bits,
                vk::MemoryPropertyFlags::DEVICE_LOCAL));
        
        let memory = device.allocate_memory(&alloc_info, None).unwrap();
        device.bind_image_memory(image, memory, 0).unwrap();
        
        // Transfer data from staging buffer to image
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        let command_buffer = device.allocate_command_buffers(&alloc_info).unwrap()[0];
        
        let begin_info = vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
        device.begin_command_buffer(command_buffer, &begin_info).unwrap();
        
        // Transition to TRANSFER_DST_OPTIMAL
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::UNDEFINED)
            .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
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
            .src_access_mask(vk::AccessFlags::empty())
            .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE);
        
        device.cmd_pipeline_barrier(command_buffer, vk::PipelineStageFlags::TOP_OF_PIPE, vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(), &[], &[], &[barrier]);
        
        // Copy buffer to image
        let region = vk::BufferImageCopy::default()
            .buffer_offset(0)
            .buffer_row_length(0)
            .buffer_image_height(0)
            .image_subresource(vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            })
            .image_offset(vk::Offset3D { x: 0, y: 0, z: 0 })
            .image_extent(vk::Extent3D { width, height, depth: 1 });
        
        device.cmd_copy_buffer_to_image(command_buffer, staging_buffer, image, 
            vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[region]);
        
        // Transition to SHADER_READ_ONLY_OPTIMAL
        let barrier = vk::ImageMemoryBarrier::default()
            .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .new_layout(vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
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
            .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
            .dst_access_mask(vk::AccessFlags::SHADER_READ);
        
        device.cmd_pipeline_barrier(command_buffer, vk::PipelineStageFlags::TRANSFER, vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(), &[], &[], &[barrier]);
        
        device.end_command_buffer(command_buffer).unwrap();
        let submit_info = vk::SubmitInfo::default().command_buffers(std::slice::from_ref(&command_buffer));
        device.queue_submit(queue, &[submit_info], vk::Fence::null()).unwrap();
        device.queue_wait_idle(queue).unwrap();
        device.free_command_buffers(command_pool, &[command_buffer]);
        
        // Cleanup staging buffer
        device.destroy_buffer(staging_buffer, None);
        device.free_memory(staging_memory, None);
        
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let image_view = device.create_image_view(&view_info, None).unwrap();
        
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::LINEAR)
            .min_filter(vk::Filter::LINEAR)
            .mipmap_mode(vk::SamplerMipmapMode::LINEAR)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE);
        let sampler = device.create_sampler(&sampler_info, None).unwrap();
        
        (image, memory, image_view, sampler)
    }
}

fn create_buffer(
    device: &ash::Device,
    memory_properties: &vk::PhysicalDeviceMemoryProperties,
    size: usize,
    usage: vk::BufferUsageFlags,
    properties: vk::MemoryPropertyFlags,
) -> (vk::Buffer, vk::DeviceMemory) {
    unsafe {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size as u64)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        let buffer = device.create_buffer(&buffer_info, None).unwrap();
        let mem_requirements = device.get_buffer_memory_requirements(buffer);
        
        let alloc_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_requirements.size)
            .memory_type_index(find_memory_type(memory_properties, mem_requirements.memory_type_bits, properties));
        let memory = device.allocate_memory(&alloc_info, None).unwrap();
        device.bind_buffer_memory(buffer, memory, 0).unwrap();
        
        (buffer, memory)
    }
}

fn find_memory_type(
    memory_properties: &vk::PhysicalDeviceMemoryProperties,
    type_filter: u32,
    properties: vk::MemoryPropertyFlags,
) -> u32 {
    for i in 0..memory_properties.memory_type_count {
        if (type_filter & (1 << i)) != 0
            && memory_properties.memory_types[i as usize].property_flags.contains(properties)
        {
            return i;
        }
    }
    panic!("Failed to find suitable memory type");
}

/// Load SPIR-V from bytes (handles alignment)
fn load_spirv_file(bytes: &[u8]) -> Vec<u32> {
    let mut cursor = std::io::Cursor::new(bytes);
    ash::util::read_spv(&mut cursor).expect("Failed to read SPIR-V")
}
