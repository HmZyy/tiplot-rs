use eframe::egui;
use eframe::egui_wgpu::{CallbackResources, CallbackTrait};
use std::collections::{HashMap, VecDeque};
use std::mem::size_of;
use std::sync::Mutex;
use wgpu::util::DeviceExt;

pub struct TraceGpuResource {
    pub buffer: wgpu::Buffer,
    /// Number of data points currently written (what the shader draws).
    pub count: u32,
    /// Number of data points the buffer can hold before reallocation.
    pub capacity: u32,
}

pub struct PlotRenderer {
    pub point_pipeline: wgpu::RenderPipeline,
    pub miter_pipeline: wgpu::RenderPipeline,

    pub bind_group_layout: wgpu::BindGroupLayout,

    pub buffers: HashMap<String, TraceGpuResource>,

    pub paint_jobs: Mutex<VecDeque<wgpu::BindGroup>>,
}

impl PlotRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Plot Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shader.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Plot Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Plot Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let point_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Plot Point Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_scatter",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_scatter",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let miter_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Plot Miter Line Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_miter",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            point_pipeline,
            miter_pipeline,
            bind_group_layout,
            buffers: HashMap::new(),
            paint_jobs: Mutex::new(VecDeque::new()),
        }
    }

    pub fn upload_trace(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        topic: &str,
        col: &str,
        times: &[f32],
        values: &[f32],
    ) {
        let key = format!("{}/{}", topic, col);
        let new_count = times.len().min(values.len());

        if new_count == 0 {
            return;
        }

        // Fast path: existing buffer has enough capacity — only write the new tail.
        if let Some(res) = self.buffers.get_mut(&key) {
            let old_count = res.count as usize;

            if new_count == old_count {
                return; // nothing new since last upload
            }

            if (new_count as u32) <= res.capacity {
                // Interleave only the newly appended points.
                let new_data: Vec<f32> = times[old_count..new_count]
                    .iter()
                    .zip(values[old_count..new_count].iter())
                    .flat_map(|(t, v)| [*t, *v])
                    .collect();
                let byte_offset = (old_count * 2 * size_of::<f32>()) as u64;
                queue.write_buffer(&res.buffer, byte_offset, bytemuck::cast_slice(&new_data));
                res.count = new_count as u32;
                return;
            }
            // Capacity exceeded — fall through to full reallocation.
        }

        // Slow path: allocate a new buffer with 2× headroom to amortise future appends.
        let capacity = ((new_count * 2) as u32).max(64);
        let buffer_size = (capacity as usize * 2 * size_of::<f32>()) as u64;

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("Trace Buffer: {}", key)),
            size: buffer_size,
            // COPY_DST is required for queue.write_buffer on subsequent appends.
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Interleave all current points: [T0, V0, T1, V1, …]
        let data: Vec<f32> = times[..new_count]
            .iter()
            .zip(values[..new_count].iter())
            .flat_map(|(t, v)| [*t, *v])
            .collect();

        queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&data));

        self.buffers.insert(
            key,
            TraceGpuResource {
                buffer,
                count: new_count as u32,
                capacity,
            },
        );
    }

    pub fn _get_trace(&self, topic: &str, col: &str) -> Option<&TraceGpuResource> {
        let key = format!("{}/{}", topic, col);
        self.buffers.get(&key)
    }
}

pub struct RealPlotCallback {
    pub topic: String,
    pub col: String,
    pub bounds: [f32; 4], // [min_time, max_time, min_val, max_val]
    pub color: [f32; 4],  // RGBA
    pub scatter_mode: bool,
    pub point_size: f32,
    pub line_thickness: f32,
    /// The tile rect in logical points — used to compute the correct
    /// pixel dimensions for the shader (egui-wgpu sets the wgpu viewport
    /// to this rect, so clip space maps to the tile, not the full screen).
    pub rect: egui::Rect,
}

impl CallbackTrait for RealPlotCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        screen: &eframe::egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(renderer) = resources.get::<PlotRenderer>() else {
            return Vec::new();
        };
        let key = format!("{}/{}", self.topic, self.col);

        if let Some(trace_res) = renderer.buffers.get(&key) {
            let ppp = screen.pixels_per_point;
            let line_thickness = self.line_thickness * ppp;
            // egui-wgpu sets the wgpu viewport to the callback rect, so clip space
            // [-1,1] maps to the tile — use tile pixel dimensions for the shader's
            // pixel-to-clip-space conversion, not the full screen dimensions.
            let tile_w = (self.rect.width() * ppp).max(1.0);
            let tile_h = (self.rect.height() * ppp).max(1.0);
            // params.x serves dual purpose:
            //   scatter mode → point_size_px (circle radius input)
            //   line mode    → total point count (for miter bounds-checking)
            let params_x = if self.scatter_mode {
                self.point_size * ppp
            } else {
                trace_res.count as f32
            };
            let uniforms_data: Vec<f32> = self
                .bounds
                .iter()
                .chain(self.color.iter())
                .cloned()
                .chain([params_x, tile_w, tile_h, line_thickness].iter().cloned())
                .collect();

            let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Plot Uniform Buffer"),
                contents: bytemuck::cast_slice(&uniforms_data),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Plot Bind Group"),
                layout: &renderer.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: trace_res.buffer.as_entire_binding(),
                    },
                ],
            });

            match renderer.paint_jobs.lock() {
                Ok(mut jobs) => jobs.push_back(bind_group),
                Err(e) => {
                    eprintln!("[error] paint_jobs mutex poisoned in prepare(): {}", e);
                }
            }
        }

        Vec::new()
    }

    fn paint<'a>(
        &'a self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &'a CallbackResources,
    ) {
        let Some(renderer) = resources.get::<PlotRenderer>() else {
            return;
        };
        let key = format!("{}/{}", self.topic, self.col);

        if let Some(trace_res) = renderer.buffers.get(&key) {
            let mut jobs = match renderer.paint_jobs.lock() {
                Ok(j) => j,
                Err(e) => {
                    eprintln!("[error] paint_jobs mutex poisoned in paint(): {}", e);
                    return;
                }
            };

            if let Some(bg) = jobs.pop_front() {
                if self.scatter_mode {
                    render_pass.set_pipeline(&renderer.point_pipeline);
                    render_pass.set_bind_group(0, &bg, &[]);
                    render_pass.draw(0..4, 0..trace_res.count);
                } else {
                    // Miter-join thick polyline: each instance draws the quad for
                    // segment [i, i+1] and reads neighbours for gapless joins.
                    render_pass.set_pipeline(&renderer.miter_pipeline);
                    render_pass.set_bind_group(0, &bg, &[]);
                    render_pass.draw(0..4, 0..trace_res.count.saturating_sub(1));
                }
            }
        }
    }
}
