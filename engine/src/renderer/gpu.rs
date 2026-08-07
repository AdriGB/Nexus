use super::terrain::encode_world_texture;
use crate::world::Grid;
use bytemuck::{Pod, Zeroable};
use wasm_bindgen::{JsCast, JsValue};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CameraUniform {
    viewport: [f32; 4],
    world: [f32; 4],
    hover_selected: [f32; 4],
    options: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RouteVertex {
    position: [f32; 2],
}

pub struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    route_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    route_vertex_buffer: wgpu::Buffer,
    route_vertex_count: u32,
    _world_texture: wgpu::Texture,
    world_width: u32,
    world_height: u32,
    dpr: f32,
}

impl GpuState {
    pub async fn new(canvas_id: &str) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or_else(|| js_error("window is unavailable"))?;
        let document = window
            .document()
            .ok_or_else(|| js_error("document is unavailable"))?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| js_error("GPU canvas was not found"))?
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .map_err(|_| js_error("GPU target is not a canvas"))?;

        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|error| js_error(&format!("could not create WebGPU surface: {error}")))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|error| js_error(&format!("could not request WebGPU adapter: {error}")))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("NEXUS WebGPU device"),
                ..Default::default()
            })
            .await
            .map_err(|error| js_error(&format!("could not create WebGPU device: {error}")))?;
        let config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| js_error("WebGPU surface is not supported by this adapter"))?;
        surface.configure(&device, &config);

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("NEXUS camera uniform"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("NEXUS terrain bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let world_texture = create_world_texture(&device, 1, 1);
        let world_view = world_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group =
            create_bind_group(&device, &bind_group_layout, &camera_buffer, &world_view);
        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/terrain.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("NEXUS terrain pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("NEXUS terrain pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let route_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/route.wgsl"));
        let route_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("NEXUS route pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &route_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<RouteVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &route_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let route_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("NEXUS empty route vertex buffer"),
            size: std::mem::size_of::<RouteVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            route_pipeline,
            bind_group_layout,
            bind_group,
            camera_buffer,
            route_vertex_buffer,
            route_vertex_count: 0,
            _world_texture: world_texture,
            world_width: 0,
            world_height: 0,
            dpr: 1.0,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32, dpr: f32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.dpr = dpr.max(1.0);
        self.surface.configure(&self.device, &self.config);
    }

    pub fn upload_world(&mut self, grid: &Grid) {
        if grid.width == 0 || grid.height == 0 {
            return;
        }

        let texture = create_world_texture(&self.device, grid.width, grid.height);
        let data = encode_world_texture(grid);
        self.queue.write_texture(
            texture.as_image_copy(),
            &data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(grid.width * 4),
                rows_per_image: Some(grid.height),
            },
            wgpu::Extent3d {
                width: grid.width,
                height: grid.height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.bind_group = create_bind_group(
            &self.device,
            &self.bind_group_layout,
            &self.camera_buffer,
            &view,
        );
        self._world_texture = texture;
        self.world_width = grid.width;
        self.world_height = grid.height;
        self.route_vertex_count = 0;
    }

    pub fn upload_route(&mut self, coordinates: &[u32]) {
        let vertices: Vec<_> = coordinates
            .chunks_exact(2)
            .filter_map(|coordinate| {
                let x = coordinate[0];
                let y = coordinate[1];
                (x < self.world_width && y < self.world_height).then_some(RouteVertex {
                    position: [x as f32 + 0.5, y as f32 + 0.5],
                })
            })
            .collect();

        self.route_vertex_count = vertices.len() as u32;
        if vertices.is_empty() {
            return;
        }

        self.route_vertex_buffer =
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("NEXUS route vertex buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        pan_x: f32,
        pan_y: f32,
        zoom: f32,
        hover_x: i32,
        hover_y: i32,
        selected_x: i32,
        selected_y: i32,
        show_grid: bool,
    ) -> Result<(), JsValue> {
        if self.world_width == 0 || self.world_height == 0 {
            return Ok(());
        }

        let camera = CameraUniform {
            viewport: [
                self.config.width as f32,
                self.config.height as f32,
                self.dpr,
                16.0 * zoom,
            ],
            world: [
                pan_x,
                pan_y,
                self.world_width as f32,
                self.world_height as f32,
            ],
            hover_selected: [
                hover_x as f32,
                hover_y as f32,
                selected_x as f32,
                selected_y as f32,
            ],
            options: [show_grid as u8 as f32, zoom, 0.0, 0.0],
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera));

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(())
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => return Err(js_error("WebGPU surface was lost")),
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(js_error("WebGPU surface validation failed"))
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("NEXUS terrain encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("NEXUS terrain render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 7.0 / 255.0,
                            g: 8.0 / 255.0,
                            b: 12.0 / 255.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        if self.route_vertex_count >= 2 {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("NEXUS route render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.route_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.route_vertex_buffer.slice(..));
            pass.draw(0..self.route_vertex_count, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        self.queue.present(frame);
        Ok(())
    }
}

fn create_world_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("NEXUS world texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Uint,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}

fn create_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    camera_buffer: &wgpu::Buffer,
    world_view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("NEXUS terrain bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(world_view),
            },
        ],
    })
}

fn js_error(message: &str) -> JsValue {
    JsValue::from_str(message)
}
