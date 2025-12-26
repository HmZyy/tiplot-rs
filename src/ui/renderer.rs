use eframe::egui;
use eframe::egui_wgpu::{CallbackResources, CallbackTrait};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use wgpu::util::DeviceExt;

pub struct TraceGpuResource {
    pub buffer: wgpu::Buffer,
    pub count: u32,
}

pub struct PlotRenderer {
    pub pipeline: wgpu::RenderPipeline,
    pub point_pipeline: wgpu::RenderPipeline,

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

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Plot Line Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
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
                topology: wgpu::PrimitiveTopology::LineStrip,
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

        let point_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Plot Point Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
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
                topology: wgpu::PrimitiveTopology::PointList,
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
            pipeline,
            point_pipeline,
            bind_group_layout,
            buffers: HashMap::new(),
            paint_jobs: Mutex::new(VecDeque::new()),
        }
    }

    pub fn upload_trace(
        &mut self,
        device: &wgpu::Device,
        topic: &str,
        col: &str,
        times: &[f32],
        values: &[f32],
    ) {
        let key = format!("{}/{}", topic, col);

        if times.is_empty() || values.is_empty() {
            return;
        }

        // Interleave times and values: [T0, V0, T1, V1, T2, V2, ...]
        let data: Vec<f32> = times
            .iter()
            .zip(values.iter())
            .flat_map(|(t, v)| [*t, *v])
            .collect();

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("Trace Buffer: {}", key)),
            contents: bytemuck::cast_slice(&data),
            usage: wgpu::BufferUsages::STORAGE,
        });

        self.buffers.insert(
            key,
            TraceGpuResource {
                buffer,
                count: times.len() as u32,
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
}

impl CallbackTrait for RealPlotCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _screen: &eframe::egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let renderer = resources.get::<PlotRenderer>().unwrap();
        let key = format!("{}/{}", self.topic, self.col);

        if let Some(trace_res) = renderer.buffers.get(&key) {
            let point_size = 3.0f32;
            let uniforms_data: Vec<f32> = self
                .bounds
                .iter()
                .chain(self.color.iter())
                .cloned()
                .chain([point_size, 0.0, 0.0, 0.0].iter().cloned()) // params vec4
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

            renderer.paint_jobs.lock().unwrap().push_back(bind_group);
        }

        Vec::new()
    }

    fn paint<'a>(
        &'a self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &'a CallbackResources,
    ) {
        let renderer = resources.get::<PlotRenderer>().unwrap();
        let key = format!("{}/{}", self.topic, self.col);

        if let Some(trace_res) = renderer.buffers.get(&key) {
            let mut jobs = renderer.paint_jobs.lock().unwrap();

            if let Some(bg) = jobs.pop_front() {
                if self.scatter_mode {
                    render_pass.set_pipeline(&renderer.point_pipeline);
                } else {
                    render_pass.set_pipeline(&renderer.pipeline);
                }

                render_pass.set_bind_group(0, &bg, &[]);
                render_pass.draw(0..trace_res.count, 0..1);
            }
        }
    }
}
