use std::{mem, rc::Rc};
use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::Window};

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CellId {
    #[default]
    Void = 0x00,
    Rock = 0x01,
    Sand = 0x02,
    Water = 0x03,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    id: CellId,
    state: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Zeroable, bytemuck::Pod)]
struct Cursor {
    enabled: u32,
    radius: u32,
    position: [u32; 2],
    cell_id: u32,
    _p0: u32,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            enabled: 0,
            radius: 1,
            position: [0, 0],
            cell_id: 0,
            _p0: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Zeroable, bytemuck::Pod)]
struct World {
    size: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Zeroable, bytemuck::Pod)]
struct Push {
    local_offset: [u32; 2],
    state: u32,
    _p0: u32,
}

pub struct Simulation {
    window: Rc<Window>,
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    surface_present_mode: wgpu::PresentMode,
    device: wgpu::Device,
    queue: wgpu::Queue,
    world_buffer: wgpu::Buffer,
    cursor_buffer: wgpu::Buffer,
    cells_buffer_size: u64,
    cells_input_buffer: wgpu::Buffer,
    cells_output_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    cursor_pipeline: wgpu::ComputePipeline,
    step_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
    state: u32,
}

impl Simulation {
    pub const SIZE: [u32; 2] = [128, 128];

    pub fn new(window: Rc<Window>) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let surface = {
            let target = unsafe { wgpu::SurfaceTargetUnsafe::from_window(&window) }
                .expect("valid surface target");
            unsafe { instance.create_surface_unsafe(target) }
        }
        .expect("new surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .expect("request adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::PUSH_CONSTANTS,
                required_limits: wgpu::Limits {
                    max_push_constant_size: adapter.limits().max_push_constant_size,
                    ..Default::default()
                },
            },
            None,
        ))
        .expect("valid device, queue");
        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = *capabilities.formats.get(0).expect("texture format");
        let surface_present_mode = wgpu::PresentMode::AutoNoVsync;
        let surface_config =
            Self::create_surface_config(surface_format, window.inner_size(), surface_present_mode);
        surface.configure(&device, &surface_config);
        let world = World { size: Self::SIZE };
        let world_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("World"),
            contents: bytemuck::bytes_of(&world),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });
        let cursor = Cursor {
            enabled: 0,
            radius: 1,
            position: [0, 0],
            cell_id: 0,
            _p0: 0,
        };
        let cursor_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cursor"),
            contents: bytemuck::bytes_of(&cursor),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });
        let cells_buffer_size = wgpu::util::align_to(
            mem::size_of::<Cell>() as u64 * (Self::SIZE[0] * Self::SIZE[1]) as u64,
            wgpu::COPY_BUFFER_ALIGNMENT,
        );
        let cells_input_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cells Input"),
            size: cells_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let cells_output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cells Output"),
            size: cells_buffer_size,
            usage: wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(
                        world_buffer.as_entire_buffer_binding(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(
                        cursor_buffer.as_entire_buffer_binding(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Buffer(
                        cells_input_buffer.as_entire_buffer_binding(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Buffer(
                        cells_output_buffer.as_entire_buffer_binding(),
                    ),
                },
            ],
        });
        let range = 0..mem::size_of::<Push>() as u32;
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::COMPUTE,
                range,
            }],
        });
        let module = device.create_shader_module(wgpu::include_wgsl!("simulation.wgsl"));
        let cursor_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: "compute_cursor",
        });
        let step_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: "compute_step",
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "vertex_main",
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: "fragment_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });
        let state = 0;
        Self {
            window,
            instance,
            surface,
            surface_format,
            surface_present_mode,
            device,
            queue,
            world_buffer,
            cursor_buffer,
            cells_buffer_size,
            cells_input_buffer,
            cells_output_buffer,
            bind_group,
            cursor_pipeline,
            step_pipeline,
            render_pipeline,
            state,
        }
    }

    pub fn reconfigure(&self) {
        let size = self.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        let surface_config =
            Self::create_surface_config(self.surface_format, size, self.surface_present_mode);
        self.surface.configure(&self.device, &surface_config);
    }

    pub fn set_cursor(&self, enabled: bool, radius: u32, position: [u32; 2], cell_id: CellId) {
        let cursor = Cursor {
            enabled: enabled.into(),
            radius,
            position,
            cell_id: cell_id as u32,
            _p0: 0,
        };
        self.queue
            .write_buffer(&self.cursor_buffer, 0, bytemuck::bytes_of(&cursor));
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_pipeline(&self.cursor_pipeline);
            pass.dispatch_workgroups(Self::SIZE[0], Self::SIZE[1], 1);
        }
        self.queue.submit(Some(encoder.finish()));
    }

    pub fn step(&mut self) {
        let workgroups = [
            wgpu::util::align_to(Self::SIZE[0], 3) / 3,
            wgpu::util::align_to(Self::SIZE[1], 3) / 3,
        ];
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_buffer_to_buffer(
            &self.cells_output_buffer,
            0,
            &self.cells_input_buffer,
            0,
            self.cells_buffer_size,
        );
        encoder.clear_buffer(&self.cells_output_buffer, 0, None);
        {
            let mut push = Push {
                local_offset: [0, 0],
                state: self.state,
                _p0: 0,
            };
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_pipeline(&self.step_pipeline);
            for i in 0..9 {
                push.local_offset = [i % 3, i / 3];
                pass.set_push_constants(0, bytemuck::bytes_of(&push));
                pass.dispatch_workgroups(workgroups[0], workgroups[1], 1);
                push.state = self.state.wrapping_add(1);
            }
            self.state = push.state;
        }
        self.queue.submit(Some(encoder.finish()));
    }

    pub fn redraw(&self) {
        let surface_texture = match self.surface.get_current_texture() {
            Ok(surface_texture) => surface_texture,
            Err(err) => {
                log::warn!("failed to get surface texture! {:?}", &err);
                return;
            }
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_pipeline(&self.render_pipeline);
            pass.draw(0..6, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();
    }

    fn create_surface_config(
        format: wgpu::TextureFormat,
        size: PhysicalSize<u32>,
        present_mode: wgpu::PresentMode,
    ) -> wgpu::SurfaceConfiguration {
        wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        }
    }
}
