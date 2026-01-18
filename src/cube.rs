use ash::vk;
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc, AllocationScheme};
use gpu_allocator::MemoryLocation;
use crate::renderer::{VulkanRenderer, Vertex, UniformBufferObject, MAX_FRAMES_IN_FLIGHT};

pub struct CubeRenderer {
    pub vertex_buffer: vk::Buffer,
    pub vertex_allocation: Option<Allocation>,
    pub index_buffer: vk::Buffer,
    pub index_allocation: Option<Allocation>,
    pub uniform_buffers: Vec<vk::Buffer>,
    pub uniform_allocations: Vec<Option<Allocation>>,
    pub index_count: u32,
}

impl CubeRenderer {
    pub unsafe fn new(renderer: &VulkanRenderer) -> Result<Self, Box<dyn std::error::Error>> {
        // Create cube vertices - 24 vertices for proper per-face colors
        // Pastel colors matching reference scene
        let vertices = [
            // Front face (pale green)
            Vertex { pos: [-0.5, -0.5,  0.5], color: [0.5, 0.7, 0.5], normal: [0.0, 0.0, 1.0] },
            Vertex { pos: [ 0.5, -0.5,  0.5], color: [0.5, 0.7, 0.5], normal: [0.0, 0.0, 1.0] },
            Vertex { pos: [ 0.5,  0.5,  0.5], color: [0.5, 0.7, 0.5], normal: [0.0, 0.0, 1.0] },
            Vertex { pos: [-0.5,  0.5,  0.5], color: [0.5, 0.7, 0.5], normal: [0.0, 0.0, 1.0] },
            // Back face (olive/khaki)
            Vertex { pos: [-0.5, -0.5, -0.5], color: [0.6, 0.6, 0.3], normal: [0.0, 0.0, -1.0] },
            Vertex { pos: [-0.5,  0.5, -0.5], color: [0.6, 0.6, 0.3], normal: [0.0, 0.0, -1.0] },
            Vertex { pos: [ 0.5,  0.5, -0.5], color: [0.6, 0.6, 0.3], normal: [0.0, 0.0, -1.0] },
            Vertex { pos: [ 0.5, -0.5, -0.5], color: [0.6, 0.6, 0.3], normal: [0.0, 0.0, -1.0] },
            // Top face (light cyan/aqua)
            Vertex { pos: [-0.5,  0.5, -0.5], color: [0.7, 0.9, 0.9], normal: [0.0, 1.0, 0.0] },
            Vertex { pos: [-0.5,  0.5,  0.5], color: [0.7, 0.9, 0.9], normal: [0.0, 1.0, 0.0] },
            Vertex { pos: [ 0.5,  0.5,  0.5], color: [0.7, 0.9, 0.9], normal: [0.0, 1.0, 0.0] },
            Vertex { pos: [ 0.5,  0.5, -0.5], color: [0.7, 0.9, 0.9], normal: [0.0, 1.0, 0.0] },
            // Bottom face (muted green)
            Vertex { pos: [-0.5, -0.5, -0.5], color: [0.4, 0.55, 0.4], normal: [0.0, -1.0, 0.0] },
            Vertex { pos: [ 0.5, -0.5, -0.5], color: [0.4, 0.55, 0.4], normal: [0.0, -1.0, 0.0] },
            Vertex { pos: [ 0.5, -0.5,  0.5], color: [0.4, 0.55, 0.4], normal: [0.0, -1.0, 0.0] },
            Vertex { pos: [-0.5, -0.5,  0.5], color: [0.4, 0.55, 0.4], normal: [0.0, -1.0, 0.0] },
            // Right face (pale yellow)
            Vertex { pos: [ 0.5, -0.5, -0.5], color: [0.9, 0.9, 0.7], normal: [1.0, 0.0, 0.0] },
            Vertex { pos: [ 0.5,  0.5, -0.5], color: [0.9, 0.9, 0.7], normal: [1.0, 0.0, 0.0] },
            Vertex { pos: [ 0.5,  0.5,  0.5], color: [0.9, 0.9, 0.7], normal: [1.0, 0.0, 0.0] },
            Vertex { pos: [ 0.5, -0.5,  0.5], color: [0.9, 0.9, 0.7], normal: [1.0, 0.0, 0.0] },
            // Left face (forest green)
            Vertex { pos: [-0.5, -0.5, -0.5], color: [0.35, 0.55, 0.35], normal: [-1.0, 0.0, 0.0] },
            Vertex { pos: [-0.5, -0.5,  0.5], color: [0.35, 0.55, 0.35], normal: [-1.0, 0.0, 0.0] },
            Vertex { pos: [-0.5,  0.5,  0.5], color: [0.35, 0.55, 0.35], normal: [-1.0, 0.0, 0.0] },
            Vertex { pos: [-0.5,  0.5, -0.5], color: [0.35, 0.55, 0.35], normal: [-1.0, 0.0, 0.0] },
        ];
        
        // Indices for 12 triangles (6 faces * 2 triangles)
        let indices: [u16; 36] = [
            0,  1,  2,  2,  3,  0,   // Front
            4,  5,  6,  6,  7,  4,   // Back
            8,  9,  10, 10, 11, 8,   // Top
            12, 13, 14, 14, 15, 12,  // Bottom
            16, 17, 18, 18, 19, 16,  // Right
            20, 21, 22, 22, 23, 20,  // Left
        ];
        
        // Create vertex buffer
        let (vertex_buffer, vertex_allocation) = Self::create_buffer(
            renderer,
            std::mem::size_of_val(&vertices) as u64,
            vk::BufferUsageFlags::VERTEX_BUFFER,
            &vertices,
        )?;
        
        // Create index buffer
        let (index_buffer, index_allocation) = Self::create_buffer(
            renderer,
            std::mem::size_of_val(&indices) as u64,
            vk::BufferUsageFlags::INDEX_BUFFER,
            &indices,
        )?;
        
        // Create uniform buffers (one per frame in flight)
        let mut uniform_buffers = Vec::new();
        let mut uniform_allocations = Vec::new();
        
        let ubo_size = std::mem::size_of::<UniformBufferObject>() as u64;
        for i in 0..MAX_FRAMES_IN_FLIGHT {
            let buffer_info = vk::BufferCreateInfo::default()
                .size(ubo_size)
                .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
                .sharing_mode(vk::SharingMode::EXCLUSIVE);
            
            let buffer = renderer.device.create_buffer(&buffer_info, None)?;
            let requirements = renderer.device.get_buffer_memory_requirements(buffer);
            
            let allocation = renderer.allocator.lock().allocate(&AllocationCreateDesc {
                name: &format!("Uniform Buffer {}", i),
                requirements,
                location: MemoryLocation::CpuToGpu,
                linear: true,
                allocation_scheme: AllocationScheme::GpuAllocatorManaged,
            })?;
            
            renderer.device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())?;
            
            // Update descriptor sets
            let buffer_info_desc = vk::DescriptorBufferInfo {
                buffer,
                offset: 0,
                range: ubo_size,
            };
            
            let descriptor_write = vk::WriteDescriptorSet::default()
                .dst_set(renderer.descriptor_sets[i])
                .dst_binding(0)
                .dst_array_element(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(std::slice::from_ref(&buffer_info_desc));
            
            renderer.device.update_descriptor_sets(&[descriptor_write], &[]);
            
            uniform_buffers.push(buffer);
            uniform_allocations.push(Some(allocation));
        }
        
        Ok(Self {
            vertex_buffer,
            vertex_allocation: Some(vertex_allocation),
            index_buffer,
            index_allocation: Some(index_allocation),
            uniform_buffers,
            uniform_allocations,
            index_count: indices.len() as u32,
        })
    }
    
    unsafe fn create_buffer<T: Copy>(
        renderer: &VulkanRenderer,
        size: u64,
        usage: vk::BufferUsageFlags,
        data: &[T],
    ) -> Result<(vk::Buffer, Allocation), Box<dyn std::error::Error>> {
        let buffer_info = vk::BufferCreateInfo::default()
            .size(size)
            .usage(usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);
        
        let buffer = renderer.device.create_buffer(&buffer_info, None)?;
        let requirements = renderer.device.get_buffer_memory_requirements(buffer);
        
        let allocation = renderer.allocator.lock().allocate(&AllocationCreateDesc {
            name: "Buffer",
            requirements,
            location: MemoryLocation::CpuToGpu,
            linear: true,
            allocation_scheme: AllocationScheme::GpuAllocatorManaged,
        })?;
        
        renderer.device.bind_buffer_memory(buffer, allocation.memory(), allocation.offset())?;
        
        // Copy data
        let mapped = allocation.mapped_ptr().unwrap().as_ptr() as *mut T;
        std::ptr::copy_nonoverlapping(data.as_ptr(), mapped, data.len());
        
        Ok((buffer, allocation))
    }
    
    pub unsafe fn update_uniform_buffer(
        &mut self,
        renderer: &VulkanRenderer,
        frame_index: usize,
        rotation: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let aspect = renderer.swapchain_extent.width as f32 / renderer.swapchain_extent.height as f32;
        
        let model = glam::Mat4::from_rotation_y(rotation) * glam::Mat4::from_rotation_x(rotation * 0.5);
        
        // Camera position for lighting calculations
        let camera_pos = glam::Vec3::new(2.0, 2.0, 2.0);
        
        let view = glam::Mat4::look_at_rh(
            camera_pos,
            glam::Vec3::ZERO,
            glam::Vec3::Y,
        );
        let mut proj = glam::Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 0.1, 10.0);
        // Vulkan clip space has inverted Y
        proj.y_axis.y *= -1.0;
        
        // Light coming from top-right-front
        let light_dir = glam::Vec3::new(1.0, 1.0, 1.0).normalize();
        
        let ubo = UniformBufferObject { 
            model, 
            view, 
            proj,
            camera_pos: glam::Vec4::new(camera_pos.x, camera_pos.y, camera_pos.z, rotation), // w = time
            light_dir: glam::Vec4::new(light_dir.x, light_dir.y, light_dir.z, 0.0),
        };
        
        if let Some(ref allocation) = self.uniform_allocations[frame_index] {
            let mapped = allocation.mapped_ptr().unwrap().as_ptr() as *mut UniformBufferObject;
            std::ptr::copy_nonoverlapping(&ubo, mapped, 1);
        }
        
        Ok(())
    }
    
    pub unsafe fn draw(
        &self,
        renderer: &VulkanRenderer,
        command_buffer: vk::CommandBuffer,
        frame_index: usize,
    ) -> Result<(), vk::Result> {
        renderer.device.cmd_bind_pipeline(
            command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            renderer.graphics_pipeline,
        );
        
        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: renderer.swapchain_extent.width as f32,
            height: renderer.swapchain_extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        renderer.device.cmd_set_viewport(command_buffer, 0, &[viewport]);
        
        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: renderer.swapchain_extent,
        };
        renderer.device.cmd_set_scissor(command_buffer, 0, &[scissor]);
        
        renderer.device.cmd_bind_vertex_buffers(command_buffer, 0, &[self.vertex_buffer], &[0]);
        renderer.device.cmd_bind_index_buffer(command_buffer, self.index_buffer, 0, vk::IndexType::UINT16);
        
        renderer.device.cmd_bind_descriptor_sets(
            command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            renderer.pipeline_layout,
            0,
            &[renderer.descriptor_sets[frame_index]],
            &[],
        );
        
        renderer.device.cmd_draw_indexed(command_buffer, self.index_count, 1, 0, 0, 0);
        
        Ok(())
    }
    
    pub unsafe fn record_commands(
        &self,
        renderer: &VulkanRenderer,
        command_buffer: vk::CommandBuffer,
        framebuffer: vk::Framebuffer,
        frame_index: usize,
    ) -> Result<(), vk::Result> {
        let begin_info = vk::CommandBufferBeginInfo::default();
        renderer.device.begin_command_buffer(command_buffer, &begin_info)?;
        
        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.39, 0.58, 0.93, 1.0],  // Cornflower blue background
            },
        }];
        
        let render_pass_info = vk::RenderPassBeginInfo::default()
            .render_pass(renderer.render_pass)
            .framebuffer(framebuffer)
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: renderer.swapchain_extent,
            })
            .clear_values(&clear_values);
        
        renderer.device.cmd_begin_render_pass(
            command_buffer,
            &render_pass_info,
            vk::SubpassContents::INLINE,
        );
        
        renderer.device.cmd_bind_pipeline(
            command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            renderer.graphics_pipeline,
        );
        
        // Set dynamic viewport
        let viewport = vk::Viewport {
            x: 0.0,
            y: 0.0,
            width: renderer.swapchain_extent.width as f32,
            height: renderer.swapchain_extent.height as f32,
            min_depth: 0.0,
            max_depth: 1.0,
        };
        renderer.device.cmd_set_viewport(command_buffer, 0, &[viewport]);
        
        // Set dynamic scissor
        let scissor = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: renderer.swapchain_extent,
        };
        renderer.device.cmd_set_scissor(command_buffer, 0, &[scissor]);
        
        renderer.device.cmd_bind_vertex_buffers(command_buffer, 0, &[self.vertex_buffer], &[0]);
        renderer.device.cmd_bind_index_buffer(command_buffer, self.index_buffer, 0, vk::IndexType::UINT16);
        
        renderer.device.cmd_bind_descriptor_sets(
            command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            renderer.pipeline_layout,
            0,
            &[renderer.descriptor_sets[frame_index]],
            &[],
        );
        
        renderer.device.cmd_draw_indexed(command_buffer, self.index_count, 1, 0, 0, 0);
        
        renderer.device.cmd_end_render_pass(command_buffer);
        renderer.device.end_command_buffer(command_buffer)?;
        
        Ok(())
    }
    
    pub unsafe fn cleanup(&mut self, renderer: &VulkanRenderer) {
        for buffer in &self.uniform_buffers {
            renderer.device.destroy_buffer(*buffer, None);
        }
        for allocation in self.uniform_allocations.drain(..) {
            if let Some(alloc) = allocation {
                let _ = renderer.allocator.lock().free(alloc);
            }
        }
        
        renderer.device.destroy_buffer(self.index_buffer, None);
        if let Some(alloc) = self.index_allocation.take() {
            let _ = renderer.allocator.lock().free(alloc);
        }
        
        renderer.device.destroy_buffer(self.vertex_buffer, None);
        if let Some(alloc) = self.vertex_allocation.take() {
            let _ = renderer.allocator.lock().free(alloc);
        }
    }
}
