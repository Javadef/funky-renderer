use ash::vk;
use ash::Device;
use rayon::prelude::*;
use std::sync::Arc;

pub struct MultiThreadedRenderer {
    pub command_pools: Vec<vk::CommandPool>,
    pub thread_count: usize,
}

impl MultiThreadedRenderer {
    pub unsafe fn new(
        device: &Arc<Device>,
        queue_family_index: u32,
    ) -> Result<Self, vk::Result> {
        let thread_count = num_cpus::get().min(4);
        let mut command_pools = Vec::with_capacity(thread_count);
        
        for _ in 0..thread_count {
            let pool_info = vk::CommandPoolCreateInfo::default()
                .queue_family_index(queue_family_index)
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);
            
            let pool = device.create_command_pool(&pool_info, None)?;
            command_pools.push(pool);
        }
        
        println!("🧵 Multi-threaded renderer: {} threads", thread_count);
        
        Ok(Self {
            command_pools,
            thread_count,
        })
    }
    
    pub fn parallel_execute<F, R>(&self, count: usize, f: F) -> Vec<R>
    where
        F: Fn(usize) -> R + Send + Sync,
        R: Send,
    {
        (0..count).into_par_iter().map(f).collect()
    }
    
    pub unsafe fn cleanup(&self, device: &Device) {
        for &pool in &self.command_pools {
            device.destroy_command_pool(pool, None);
        }
    }
}

// Utility for batch processing
pub fn parallel_batch<T, F>(items: &[T], batch_size: usize, f: F)
where
    T: Sync,
    F: Fn(&[T]) + Send + Sync,
{
    items.par_chunks(batch_size).for_each(|chunk| f(chunk));
}
