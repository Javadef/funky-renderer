use ash::vk;
use ash::Device;
use rayon::prelude::*;
use std::sync::Arc;

/// Thread-local command recording resources
pub struct ThreadCommandResources {
    pub command_pool: vk::CommandPool,
    pub secondary_buffers: Vec<vk::CommandBuffer>,
}

/// Multi-threaded renderer for parallel command buffer recording
pub struct MultiThreadedRenderer {
    /// Per-thread command pools and buffers
    pub thread_resources: Vec<ThreadCommandResources>,
    /// Number of worker threads
    pub thread_count: usize,
    /// Device reference for command recording
    device: Arc<Device>,
}

impl MultiThreadedRenderer {
    pub unsafe fn new(
        device: &Arc<Device>,
        queue_family_index: u32,
    ) -> Result<Self, vk::Result> {
        let thread_count = num_cpus::get().min(4);
        let mut thread_resources = Vec::with_capacity(thread_count);
        
        for i in 0..thread_count {
            // Create command pool for this thread
            let pool_info = vk::CommandPoolCreateInfo::default()
                .queue_family_index(queue_family_index)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
            
            let command_pool = device.create_command_pool(&pool_info, None)?;
            
            // Allocate secondary command buffers for this thread
            let alloc_info = vk::CommandBufferAllocateInfo::default()
                .command_pool(command_pool)
                .level(vk::CommandBufferLevel::SECONDARY)
                .command_buffer_count(4); // 4 buffers per thread for flexibility
            
            let secondary_buffers = device.allocate_command_buffers(&alloc_info)?;
            let buffer_count = secondary_buffers.len();
            
            thread_resources.push(ThreadCommandResources {
                command_pool,
                secondary_buffers,
            });
            
            println!("  Thread {}: pool + {} secondary buffers", i, buffer_count);
        }
        
        println!("🧵 Multi-threaded renderer: {} threads initialized", thread_count);
        
        Ok(Self {
            thread_resources,
            thread_count,
            device: device.clone(),
        })
    }
    
    /// Execute work in parallel across threads using Rayon
    pub fn parallel_execute<F, R>(&self, count: usize, f: F) -> Vec<R>
    where
        F: Fn(usize) -> R + Send + Sync,
        R: Send,
    {
        (0..count).into_par_iter().map(f).collect()
    }
    
    /// Record secondary command buffers in parallel for multiple objects
    /// Returns the recorded secondary command buffers ready to be executed
    pub unsafe fn record_secondary_parallel<F>(
        &self,
        render_pass: vk::RenderPass,
        framebuffer: vk::Framebuffer,
        object_count: usize,
        record_fn: F,
    ) -> Vec<vk::CommandBuffer>
    where
        F: Fn(usize, vk::CommandBuffer) + Send + Sync,
    {
        let buffers_per_thread = (object_count + self.thread_count - 1) / self.thread_count;
        let device = &self.device;
        
        // Collect all secondary buffers that will be used
        let result: Vec<vk::CommandBuffer> = (0..self.thread_count)
            .into_par_iter()
            .flat_map(|thread_idx| {
                let start = thread_idx * buffers_per_thread;
                let end = (start + buffers_per_thread).min(object_count);
                
                if start >= object_count {
                    return vec![];
                }
                
                let resources = &self.thread_resources[thread_idx];
                let mut recorded = Vec::new();
                
                for (local_idx, obj_idx) in (start..end).enumerate() {
                    if local_idx >= resources.secondary_buffers.len() {
                        break;
                    }
                    
                    let cmd = resources.secondary_buffers[local_idx];
                    
                    // Begin secondary command buffer
                    let inheritance_info = vk::CommandBufferInheritanceInfo::default()
                        .render_pass(render_pass)
                        .framebuffer(framebuffer)
                        .subpass(0);
                    
                    let begin_info = vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::RENDER_PASS_CONTINUE)
                        .inheritance_info(&inheritance_info);
                    
                    device.begin_command_buffer(cmd, &begin_info).unwrap();
                    
                    // Record commands for this object
                    record_fn(obj_idx, cmd);
                    
                    device.end_command_buffer(cmd).unwrap();
                    
                    recorded.push(cmd);
                }
                
                recorded
            })
            .collect();
        
        result
    }
    
    /// Reset all command pools (call at start of frame)
    pub unsafe fn reset_pools(&self) {
        for resources in &self.thread_resources {
            self.device.reset_command_pool(
                resources.command_pool,
                vk::CommandPoolResetFlags::empty(),
            ).unwrap();
        }
    }
    
    /// Get a secondary buffer for a specific thread
    pub fn get_secondary_buffer(&self, thread_idx: usize, buffer_idx: usize) -> Option<vk::CommandBuffer> {
        self.thread_resources
            .get(thread_idx)
            .and_then(|r| r.secondary_buffers.get(buffer_idx).copied())
    }
    
    /// Get thread count
    pub fn thread_count(&self) -> usize {
        self.thread_count
    }
    
    pub unsafe fn cleanup(&self, device: &Device) {
        for resources in &self.thread_resources {
            device.destroy_command_pool(resources.command_pool, None);
        }
    }
}

/// Utility for batch processing with Rayon
pub fn parallel_batch<T, F>(items: &[T], batch_size: usize, f: F)
where
    T: Sync,
    F: Fn(&[T]) + Send + Sync,
{
    items.par_chunks(batch_size).for_each(|chunk| f(chunk));
}

/// Thread-safe counter for work distribution
pub struct AtomicWorkQueue {
    counter: std::sync::atomic::AtomicUsize,
    total: usize,
}

impl AtomicWorkQueue {
    pub fn new(total: usize) -> Self {
        Self {
            counter: std::sync::atomic::AtomicUsize::new(0),
            total,
        }
    }
    
    /// Get next work item index, returns None if all work is done
    pub fn next(&self) -> Option<usize> {
        let idx = self.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if idx < self.total {
            Some(idx)
        } else {
            None
        }
    }
    
    pub fn reset(&self) {
        self.counter.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}
