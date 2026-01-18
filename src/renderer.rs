use ash::vk;
use ash::{Device, Entry, Instance};
use gpu_allocator::vulkan::{Allocator, AllocatorCreateDesc};
use gpu_allocator::AllocationSizes;
use parking_lot::Mutex;
use std::ffi::CString;
use std::sync::Arc;
use winit::raw_window_handle::{HasDisplayHandle, HasWindowHandle};

pub struct VulkanRenderer {
    pub entry: Entry,
    pub instance: Instance,
    pub physical_device: vk::PhysicalDevice,
    pub device: Arc<Device>,
    pub graphics_queue: vk::Queue,
    pub present_queue: vk::Queue,
    pub surface_fn: ash::khr::surface::Instance,
    pub surface: vk::SurfaceKHR,
    pub swapchain_fn: ash::khr::swapchain::Device,
    pub swapchain: vk::SwapchainKHR,
    pub swapchain_images: Vec<vk::Image>,
    pub swapchain_image_views: Vec<vk::ImageView>,
    pub swapchain_format: vk::Format,
    pub swapchain_extent: vk::Extent2D,
    pub render_pass: vk::RenderPass,
    pub framebuffers: Vec<vk::Framebuffer>,
    pub pipeline_layout: vk::PipelineLayout,
    pub graphics_pipeline: vk::Pipeline,
    pub command_pool: vk::CommandPool,
    pub command_buffers: Vec<vk::CommandBuffer>,
    pub image_available_semaphores: Vec<vk::Semaphore>,
    pub render_finished_semaphores: Vec<vk::Semaphore>,
    pub in_flight_fences: Vec<vk::Fence>,
    pub current_frame: usize,
    pub allocator: Arc<Mutex<Allocator>>,
    pub descriptor_set_layout: vk::DescriptorSetLayout,
    pub descriptor_pool: vk::DescriptorPool,
    pub descriptor_sets: Vec<vk::DescriptorSet>,
    pub graphics_queue_family_index: u32,
    pub framebuffer_resized: bool,
}

pub const MAX_FRAMES_IN_FLIGHT: usize = 2;

impl VulkanRenderer {
    pub unsafe fn new(window: &winit::window::Window) -> Result<Self, Box<dyn std::error::Error>> {
        let entry = Entry::linked();
        
        // Create instance
        let app_name = CString::new("Funky Renderer")?;
        let engine_name = CString::new("No Engine")?;
        
        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(vk::make_api_version(0, 1, 0, 0))
            .engine_name(&engine_name)
            .engine_version(vk::make_api_version(0, 1, 0, 0))
            .api_version(vk::API_VERSION_1_2);
        
        let extension_names = ash_window::enumerate_required_extensions(
            window.display_handle()?.as_raw()
        )?.to_vec();
        
        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&app_info)
            .enabled_extension_names(&extension_names);
        
        let instance = entry.create_instance(&create_info, None)?;
        
        // Create surface
        let surface = ash_window::create_surface(
            &entry,
            &instance,
            window.display_handle()?.as_raw(),
            window.window_handle()?.as_raw(),
            None,
        )?;
        let surface_fn = ash::khr::surface::Instance::new(&entry, &instance);
        
        // Pick physical device
        let physical_devices = instance.enumerate_physical_devices()?;
        let physical_device = physical_devices[0];
        
        let props = instance.get_physical_device_properties(physical_device);
        let device_name = std::ffi::CStr::from_ptr(props.device_name.as_ptr())
            .to_string_lossy();
        println!("🎮 GPU: {}", device_name);
        
        // Find queue families
        let queue_families = instance.get_physical_device_queue_family_properties(physical_device);
        let graphics_queue_family_index = queue_families
            .iter()
            .enumerate()
            .find(|(i, queue_family)| {
                queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                    && surface_fn
                        .get_physical_device_surface_support(physical_device, *i as u32, surface)
                        .unwrap_or(false)
            })
            .map(|(i, _)| i as u32)
            .ok_or("No suitable queue family found")?;
        
        // Create logical device
        let queue_priorities = [1.0];
        let queue_create_info = vk::DeviceQueueCreateInfo::default()
            .queue_family_index(graphics_queue_family_index)
            .queue_priorities(&queue_priorities);
        
        let device_extension_names = [ash::khr::swapchain::NAME.as_ptr()];
        
        let physical_device_features = vk::PhysicalDeviceFeatures::default();
        
        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(std::slice::from_ref(&queue_create_info))
            .enabled_extension_names(&device_extension_names)
            .enabled_features(&physical_device_features);
        
        let device = Arc::new(instance.create_device(physical_device, &device_create_info, None)?);
        
        let graphics_queue = device.get_device_queue(graphics_queue_family_index, 0);
        let present_queue = graphics_queue;
        
        // Create allocator
        let allocator = Allocator::new(&AllocatorCreateDesc {
            instance: instance.clone(),
            device: (*device).clone(),
            physical_device,
            debug_settings: Default::default(),
            buffer_device_address: false,
            allocation_sizes: AllocationSizes::default(),
        })?;
        let allocator = Arc::new(Mutex::new(allocator));
        
        // Create swapchain
        let surface_capabilities = surface_fn
            .get_physical_device_surface_capabilities(physical_device, surface)?;
        let surface_formats = surface_fn
            .get_physical_device_surface_formats(physical_device, surface)?;
        let surface_format = surface_formats[0];
        
        let swapchain_extent = surface_capabilities.current_extent;
        let max_images = if surface_capabilities.max_image_count == 0 {
            u32::MAX
        } else {
            surface_capabilities.max_image_count
        };
        let image_count = (surface_capabilities.min_image_count + 1).min(max_images);
        
        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(surface)
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(swapchain_extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(surface_capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::MAILBOX);  // No VSync
        
        let swapchain_fn = ash::khr::swapchain::Device::new(&instance, &device);
        let swapchain = swapchain_fn.create_swapchain(&swapchain_create_info, None)?;
        
        let swapchain_images = swapchain_fn.get_swapchain_images(swapchain)?;
        
        // Create image views
        let swapchain_image_views: Vec<vk::ImageView> = swapchain_images
            .iter()
            .map(|&image| {
                let create_info = vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(surface_format.format)
                    .components(vk::ComponentMapping {
                        r: vk::ComponentSwizzle::IDENTITY,
                        g: vk::ComponentSwizzle::IDENTITY,
                        b: vk::ComponentSwizzle::IDENTITY,
                        a: vk::ComponentSwizzle::IDENTITY,
                    })
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });
                
                device.create_image_view(&create_info, None)
            })
            .collect::<Result<Vec<_>, _>>()?;
        
        // Create render pass
        let color_attachment = vk::AttachmentDescription::default()
            .format(surface_format.format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);
        
        let color_attachment_ref = vk::AttachmentReference {
            attachment: 0,
            layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        };
        
        let subpass = vk::SubpassDescription::default()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(std::slice::from_ref(&color_attachment_ref));
        
        let dependency = vk::SubpassDependency::default()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE);
        
        let render_pass_info = vk::RenderPassCreateInfo::default()
            .attachments(std::slice::from_ref(&color_attachment))
            .subpasses(std::slice::from_ref(&subpass))
            .dependencies(std::slice::from_ref(&dependency));
        
        let render_pass = device.create_render_pass(&render_pass_info, None)?;
        
        // Create descriptor set layout
        let ubo_layout_binding = vk::DescriptorSetLayoutBinding::default()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX);
        
        let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
            .bindings(std::slice::from_ref(&ubo_layout_binding));
        
        let descriptor_set_layout = device.create_descriptor_set_layout(&layout_info, None)?;
        
        // Create pipeline layout
        let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(std::slice::from_ref(&descriptor_set_layout));
        
        let pipeline_layout = device.create_pipeline_layout(&pipeline_layout_info, None)?;
        
        // Load shaders (embedded SPIR-V)
        let vert_shader_code = include_bytes!("../shaders/cube.vert.spv");
        let frag_shader_code = include_bytes!("../shaders/cube.frag.spv");
        
        let vert_shader_module = Self::create_shader_module(&device, vert_shader_code)?;
        let frag_shader_module = Self::create_shader_module(&device, frag_shader_code)?;
        
        let main_name = CString::new("main")?;
        
        let vert_stage_info = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::VERTEX)
            .module(vert_shader_module)
            .name(&main_name);
        
        let frag_stage_info = vk::PipelineShaderStageCreateInfo::default()
            .stage(vk::ShaderStageFlags::FRAGMENT)
            .module(frag_shader_module)
            .name(&main_name);
        
        let shader_stages = [vert_stage_info, frag_stage_info];
        
        // Vertex input
        let binding_description = vk::VertexInputBindingDescription::default()
            .binding(0)
            .stride(std::mem::size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX);
        
        let attribute_descriptions = [
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 0,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 0,
            },
            vk::VertexInputAttributeDescription {
                binding: 0,
                location: 1,
                format: vk::Format::R32G32B32_SFLOAT,
                offset: 12,
            },
        ];
        
        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::default()
            .vertex_binding_descriptions(std::slice::from_ref(&binding_description))
            .vertex_attribute_descriptions(&attribute_descriptions);
        
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);
        
        // Use dynamic viewport and scissor for resizing support
        let viewport_state = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1);
        
        let dynamic_states = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_state_info = vk::PipelineDynamicStateCreateInfo::default()
            .dynamic_states(&dynamic_states);
        
        let rasterizer = vk::PipelineRasterizationStateCreateInfo::default()
            .depth_clamp_enable(false)
            .rasterizer_discard_enable(false)
            .polygon_mode(vk::PolygonMode::FILL)
            .line_width(1.0)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .depth_bias_enable(false);
        
        let multisampling = vk::PipelineMultisampleStateCreateInfo::default()
            .sample_shading_enable(false)
            .rasterization_samples(vk::SampleCountFlags::TYPE_1);
        
        let color_blend_attachment = vk::PipelineColorBlendAttachmentState::default()
            .color_write_mask(vk::ColorComponentFlags::RGBA)
            .blend_enable(false);
        
        let color_blending = vk::PipelineColorBlendStateCreateInfo::default()
            .logic_op_enable(false)
            .attachments(std::slice::from_ref(&color_blend_attachment));
        
        let pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input_info)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .color_blend_state(&color_blending)
            .dynamic_state(&dynamic_state_info)
            .layout(pipeline_layout)
            .render_pass(render_pass)
            .subpass(0);
        
        let graphics_pipeline = device
            .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
            .map_err(|(_, e)| e)?[0];
        
        device.destroy_shader_module(vert_shader_module, None);
        device.destroy_shader_module(frag_shader_module, None);
        
        // Create framebuffers
        let framebuffers: Vec<vk::Framebuffer> = swapchain_image_views
            .iter()
            .map(|&image_view| {
                let attachments = [image_view];
                let framebuffer_info = vk::FramebufferCreateInfo::default()
                    .render_pass(render_pass)
                    .attachments(&attachments)
                    .width(swapchain_extent.width)
                    .height(swapchain_extent.height)
                    .layers(1);
                
                device.create_framebuffer(&framebuffer_info, None)
            })
            .collect::<Result<Vec<_>, _>>()?;
        
        // Create command pool
        let pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(graphics_queue_family_index)
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
        
        let command_pool = device.create_command_pool(&pool_info, None)?;
        
        // Allocate command buffers
        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(MAX_FRAMES_IN_FLIGHT as u32);
        
        let command_buffers = device.allocate_command_buffers(&alloc_info)?;
        
        // Create descriptor pool
        let pool_size = vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: MAX_FRAMES_IN_FLIGHT as u32,
        };
        
        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .pool_sizes(std::slice::from_ref(&pool_size))
            .max_sets(MAX_FRAMES_IN_FLIGHT as u32);
        
        let descriptor_pool = device.create_descriptor_pool(&pool_info, None)?;
        
        // Allocate descriptor sets
        let layouts = vec![descriptor_set_layout; MAX_FRAMES_IN_FLIGHT];
        let alloc_info = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&layouts);
        
        let descriptor_sets = device.allocate_descriptor_sets(&alloc_info)?;
        
        // Create sync objects
        let semaphore_info = vk::SemaphoreCreateInfo::default();
        let fence_info = vk::FenceCreateInfo::default()
            .flags(vk::FenceCreateFlags::SIGNALED);
        
        let mut image_available_semaphores = Vec::new();
        let mut render_finished_semaphores = Vec::new();
        let mut in_flight_fences = Vec::new();
        
        for _ in 0..MAX_FRAMES_IN_FLIGHT {
            image_available_semaphores.push(device.create_semaphore(&semaphore_info, None)?);
            render_finished_semaphores.push(device.create_semaphore(&semaphore_info, None)?);
            in_flight_fences.push(device.create_fence(&fence_info, None)?);
        }
        
        Ok(Self {
            entry,
            instance,
            physical_device,
            device,
            graphics_queue,
            present_queue,
            surface_fn,
            surface,
            swapchain_fn,
            swapchain,
            swapchain_images,
            swapchain_image_views,
            swapchain_format: surface_format.format,
            swapchain_extent,
            render_pass,
            framebuffers,
            pipeline_layout,
            graphics_pipeline,
            command_pool,
            command_buffers,
            image_available_semaphores,
            render_finished_semaphores,
            in_flight_fences,
            current_frame: 0,
            allocator,
            descriptor_set_layout,
            descriptor_pool,
            descriptor_sets,
            graphics_queue_family_index,
            framebuffer_resized: false,
        })
    }
    
    pub unsafe fn recreate_swapchain(&mut self, width: u32, height: u32) -> Result<(), vk::Result> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        
        self.device.device_wait_idle()?;
        
        // Cleanup old swapchain resources
        for &framebuffer in &self.framebuffers {
            self.device.destroy_framebuffer(framebuffer, None);
        }
        for &image_view in &self.swapchain_image_views {
            self.device.destroy_image_view(image_view, None);
        }
        
        let old_swapchain = self.swapchain;
        
        // Get new surface capabilities
        let surface_capabilities = self.surface_fn
            .get_physical_device_surface_capabilities(self.physical_device, self.surface)?;
        
        // Determine new extent
        let new_extent = if surface_capabilities.current_extent.width != u32::MAX {
            surface_capabilities.current_extent
        } else {
            vk::Extent2D {
                width: width.clamp(
                    surface_capabilities.min_image_extent.width,
                    surface_capabilities.max_image_extent.width,
                ),
                height: height.clamp(
                    surface_capabilities.min_image_extent.height,
                    surface_capabilities.max_image_extent.height,
                ),
            }
        };
        
        let max_images = if surface_capabilities.max_image_count == 0 {
            u32::MAX
        } else {
            surface_capabilities.max_image_count
        };
        let image_count = (surface_capabilities.min_image_count + 1).min(max_images);
        
        // Create new swapchain
        let swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(self.surface)
            .min_image_count(image_count)
            .image_format(self.swapchain_format)
            .image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .image_extent(new_extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(surface_capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::MAILBOX)  // No VSync
            .old_swapchain(old_swapchain);
        
        self.swapchain = self.swapchain_fn.create_swapchain(&swapchain_create_info, None)?;
        
        // Destroy old swapchain
        self.swapchain_fn.destroy_swapchain(old_swapchain, None);
        
        // Get new images
        self.swapchain_images = self.swapchain_fn.get_swapchain_images(self.swapchain)?;
        self.swapchain_extent = new_extent;
        
        // Create new image views
        self.swapchain_image_views = self.swapchain_images
            .iter()
            .map(|&image| {
                let create_info = vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(self.swapchain_format)
                    .components(vk::ComponentMapping {
                        r: vk::ComponentSwizzle::IDENTITY,
                        g: vk::ComponentSwizzle::IDENTITY,
                        b: vk::ComponentSwizzle::IDENTITY,
                        a: vk::ComponentSwizzle::IDENTITY,
                    })
                    .subresource_range(vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    });
                
                self.device.create_image_view(&create_info, None)
            })
            .collect::<Result<Vec<_>, _>>()?;
        
        // Create new framebuffers
        self.framebuffers = self.swapchain_image_views
            .iter()
            .map(|&image_view| {
                let attachments = [image_view];
                let framebuffer_info = vk::FramebufferCreateInfo::default()
                    .render_pass(self.render_pass)
                    .attachments(&attachments)
                    .width(new_extent.width)
                    .height(new_extent.height)
                    .layers(1);
                
                self.device.create_framebuffer(&framebuffer_info, None)
            })
            .collect::<Result<Vec<_>, _>>()?;
        
        self.framebuffer_resized = false;
        
        Ok(())
    }
    
    unsafe fn create_shader_module(
        device: &Device,
        code: &[u8],
    ) -> Result<vk::ShaderModule, vk::Result> {
        let code_u32: Vec<u32> = code
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        
        let create_info = vk::ShaderModuleCreateInfo::default().code(&code_u32);
        
        device.create_shader_module(&create_info, None)
    }
}

impl Drop for VulkanRenderer {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            
            for &semaphore in &self.image_available_semaphores {
                self.device.destroy_semaphore(semaphore, None);
            }
            for &semaphore in &self.render_finished_semaphores {
                self.device.destroy_semaphore(semaphore, None);
            }
            for &fence in &self.in_flight_fences {
                self.device.destroy_fence(fence, None);
            }
            
            self.device.destroy_command_pool(self.command_pool, None);
            
            for &framebuffer in &self.framebuffers {
                self.device.destroy_framebuffer(framebuffer, None);
            }
            
            self.device.destroy_pipeline(self.graphics_pipeline, None);
            self.device.destroy_pipeline_layout(self.pipeline_layout, None);
            self.device.destroy_render_pass(self.render_pass, None);
            
            for &image_view in &self.swapchain_image_views {
                self.device.destroy_image_view(image_view, None);
            }
            
            self.swapchain_fn.destroy_swapchain(self.swapchain, None);
            
            self.device.destroy_descriptor_pool(self.descriptor_pool, None);
            self.device.destroy_descriptor_set_layout(self.descriptor_set_layout, None);
            
            self.surface_fn.destroy_surface(self.surface, None);
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub color: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct UniformBufferObject {
    pub model: glam::Mat4,
    pub view: glam::Mat4,
    pub proj: glam::Mat4,
    pub camera_pos: glam::Vec4,  // xyz = camera position, w = time
    pub light_dir: glam::Vec4,   // xyz = light direction, w = unused
}
