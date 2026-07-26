use std::sync::{Arc, atomic::{AtomicBool, Ordering, AtomicU64}};
use std::thread;

pub struct GpuStress {
    is_running: Arc<AtomicBool>,
    /// T13: Total GPU compute passes completed over the full test duration
    total_passes: Arc<AtomicU64>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl GpuStress {
    pub fn new() -> Self {
        Self {
            is_running: Arc::new(AtomicBool::new(false)),
            total_passes: Arc::new(AtomicU64::new(0)),
            thread_handle: None,
        }
    }

    pub fn start(&mut self) {
        if self.is_running.load(Ordering::SeqCst) {
            return;
        }
        self.is_running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.is_running);
        let total_passes_counter = Arc::clone(&self.total_passes);

        self.thread_handle = Some(thread::spawn(move || {
            let instance = wgpu::Instance::default();

            let adapter_opt = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            }));

            let adapter = match adapter_opt {
                Ok(a) => a,
                Err(_) => {
                    println!("Failed to find a WGPU adapter for GPU stress test.");
                    running.store(false, Ordering::SeqCst);
                    return;
                }
            };

            let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();

            let shader_src = "
                @group(0) @binding(0) var<storage, read_write> data: array<f32>;

                @compute @workgroup_size(256)
                fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
                    let id = global_id.x;
                    var v = data[id];
                    // Heavy math loop
                    for (var i = 0u; i < 5000u; i = i + 1u) {
                        v = sin(v) * cos(v) + tan(v) * 1.0001;
                        v = sqrt(abs(v)) + log(abs(v) + 1.0);
                    }
                    data[id] = v;
                }
            ";

            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Stress Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_src.into()),
            });

            let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Compute Pipeline"),
                layout: None,
                module: &shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

            let buffer_size = 1024 * 1024 * 4; // 1M floats (4MB)
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Data Buffer"),
                size: buffer_size as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let bind_group_layout = compute_pipeline.get_bind_group_layout(0);
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bind Group"),
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            });

            while running.load(Ordering::Relaxed) {
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
                {
                    let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: None,
                        timestamp_writes: None,
                    });
                    cpass.set_pipeline(&compute_pipeline);
                    cpass.set_bind_group(0, &bind_group, &[]);
                    cpass.dispatch_workgroups(1024 * 1024 / 256, 1, 1);
                }
                let submission_index = queue.submit(Some(encoder.finish()));
                let _ = device.poll(wgpu::PollType::Wait { submission_index: Some(submission_index), timeout: None });

                // T13: Count total passes over full test duration for reliable scoring
                total_passes_counter.fetch_add(1, Ordering::Relaxed);

                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }));
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// T13: Total GPU compute passes over the full test duration
    pub fn get_total_passes(&self) -> u64 {
        self.total_passes.load(Ordering::Relaxed)
    }
}

impl Drop for GpuStress {
    fn drop(&mut self) {
        self.stop();
    }
}
