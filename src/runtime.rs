use std::sync::Arc;

use wgpu::*;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes},
};

use crate::pipeline::{self, Pipelines, Uniforms};
use crate::texture::TextureState;
use crate::types::Cell;

pub struct Simulation<T: Cell> {
    width: u32,
    height: u32,
    window_width: Option<u32>,
    window_height: Option<u32>,
    title: String,
    ticks_per_frame: u32,
    paused: bool,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Cell> Simulation<T> {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            window_width: None,
            window_height: None,
            title: "Cellarium".to_string(),
            ticks_per_frame: 1,
            paused: false,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    pub fn ticks_per_frame(mut self, n: u32) -> Self {
        self.ticks_per_frame = n;
        self
    }

    pub fn paused(mut self, paused: bool) -> Self {
        self.paused = paused;
        self
    }

    pub fn window_size(mut self, width: u32, height: u32) -> Self {
        self.window_width = Some(width);
        self.window_height = Some(height);
        self
    }

    pub fn run(self) {
        env_logger::init();
        let event_loop = EventLoop::new().expect("Failed to create event loop");
        let mut app = App::<T>::new(self);
        event_loop.run_app(&mut app).expect("Event loop failed");
    }
}

struct GpuState {
    device: Device,
    queue: Queue,
    surface: Surface<'static>,
    surface_config: SurfaceConfiguration,
    pipelines: Pipelines,
    textures: TextureState,
    uniform_buffer: Buffer,
    tick: u32,
    ticks_per_frame: u32,
    paused: bool,
    texture_count: u32,
    zoom: f32,
    default_zoom: f32,
    camera: [f32; 2],
    viewport: [f32; 2],
    dragging: bool,
    last_mouse: [f32; 2],
    params: Vec<f32>,
    param_names: Vec<String>,
    selected_param: usize,
}

impl GpuState {
    fn write_uniforms(&self, tick: u32) {
        let header = Uniforms {
            tick,
            zoom: self.zoom,
            resolution: [self.textures.width as f32, self.textures.height as f32],
            camera: self.camera,
            viewport: self.viewport,
        };
        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&header));
        if !self.params.is_empty() {
            let vec4_count = (self.params.len() + 3) / 4;
            let mut padded = vec![0.0f32; vec4_count * 4];
            for (i, &v) in self.params.iter().enumerate() {
                padded[i] = v;
            }
            let offset = std::mem::size_of::<Uniforms>() as u64;
            self.queue.write_buffer(&self.uniform_buffer, offset, bytemuck::cast_slice(&padded));
        }
    }

    fn print_param(&self) {
        if self.params.is_empty() { return; }
        let i = self.selected_param;
        eprintln!("[{}] {} = {:.6}", i, self.param_names[i], self.params[i]);
    }

    fn run_tick(&mut self) {
        self.write_uniforms(self.tick);

        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("sim_encoder"),
        });

        // Simulation pass — read from current, write to other
        {
            let read_views = self.textures.read_views();
            let write_views = self.textures.write_views();

            let bind_group = {
                let view_refs: Vec<&TextureView> = read_views.iter().collect();
                pipeline::create_bind_group(
                    &self.device,
                    &self.pipelines.sim_bind_group_layout,
                    &view_refs,
                    &self.uniform_buffer,
                    self.texture_count,
                )
            };

            let color_attachments: Vec<Option<RenderPassColorAttachment>> = write_views.iter()
                .map(|view| Some(RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                }))
                .collect();

            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("sim_pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines.sim_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.textures.swap();
        self.tick += 1;
    }

    fn render(&mut self, window: &Window) {
        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.surface_config);
                return;
            }
            Err(SurfaceError::OutOfMemory) => panic!("Out of GPU memory"),
            Err(e) => {
                log::warn!("Surface error: {:?}", e);
                return;
            }
        };

        let surface_view = output.texture.create_view(&TextureViewDescriptor::default());

        // Update uniforms for view pass
        self.write_uniforms(self.tick);

        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("view_encoder"),
        });

        // View pass — read current state, write to surface
        {
            let read_views = self.textures.read_views();
            let bind_group = {
                let view_refs: Vec<&TextureView> = read_views.iter().collect();
                pipeline::create_bind_group(
                    &self.device,
                    &self.pipelines.view_bind_group_layout,
                    &view_refs,
                    &self.uniform_buffer,
                    self.texture_count,
                )
            };

            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("view_pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &surface_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines.view_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        window.request_redraw();
    }

    fn run_init_shader(&mut self) {
        let Some(ref init_pipeline) = self.pipelines.init_pipeline else { return };
        let Some(ref init_bgl) = self.pipelines.init_bind_group_layout else { return };

        self.write_uniforms(0);

        // Init shader writes to textures_a
        let mut encoder = self.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("init_encoder"),
        });

        let bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("init_bind_group"),
            layout: init_bgl,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: self.uniform_buffer.as_entire_binding(),
            }],
        });

        // Write to both A and B textures
        for textures in [&self.textures.views_a, &self.textures.views_b] {
            let color_attachments: Vec<Option<RenderPassColorAttachment>> = textures.iter()
                .map(|view| Some(RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                }))
                .collect();

            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("init_pass"),
                color_attachments: &color_attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(init_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    fn reset<T: Cell>(&mut self) {
        self.tick = 0;
        self.zoom = self.default_zoom;
        self.camera = [self.textures.width as f32 / 2.0, self.textures.height as f32 / 2.0];
        self.textures.phase = false;
        if T::HAS_INIT {
            self.run_init_shader();
        } else {
            let defaults = T::defaults();
            self.textures.write_defaults(&self.queue, &defaults);
        }
    }
}

struct App<T: Cell> {
    config: Simulation<T>,
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
}

impl<T: Cell> App<T> {
    fn new(config: Simulation<T>) -> Self {
        Self {
            config,
            window: None,
            gpu: None,
        }
    }
}

impl<T: Cell> ApplicationHandler for App<T> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = if let (Some(w), Some(h)) = (self.config.window_width, self.config.window_height) {
            WindowAttributes::default()
                .with_title(&self.config.title)
                .with_inner_size(winit::dpi::PhysicalSize::new(w, h))
        } else {
            WindowAttributes::default()
                .with_title(&self.config.title)
                .with_maximized(true)
        };

        let window = Arc::new(event_loop.create_window(attrs).expect("Failed to create window"));
        self.window = Some(window.clone());

        // Initialize wgpu
        let gpu = pollster::block_on(async {
            let instance = Instance::new(&InstanceDescriptor {
                backends: Backends::PRIMARY,
                ..Default::default()
            });

            let surface = instance.create_surface(window.clone()).expect("Failed to create surface");

            let adapter = instance.request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }).await.expect("Failed to find GPU adapter");

            let (device, queue) = adapter.request_device(&DeviceDescriptor {
                label: Some("cellarium_device"),
                required_features: Features::empty(),
                required_limits: Limits::default(),
                memory_hints: MemoryHints::Performance,
                trace: Default::default(),
                experimental_features: Default::default(),
            }).await.expect("Failed to create device");

            let size = window.inner_size();
            let caps = surface.get_capabilities(&adapter);
            let surface_format = caps.formats.iter()
                .find(|f| !f.is_srgb())
                .copied()
                .unwrap_or(caps.formats[0]);

            let surface_config = SurfaceConfiguration {
                usage: TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width,
                height: size.height,
                present_mode: PresentMode::AutoVsync,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &surface_config);

            let texture_count = T::TEXTURE_COUNT;
            let textures = TextureState::new(&device, self.config.width, self.config.height, texture_count);

            let init_src = if T::HAS_INIT { Some(T::INIT_SHADER) } else { None };
            let pipelines = pipeline::create_pipelines(
                &device,
                texture_count,
                T::UPDATE_SHADER,
                T::VIEW_SHADER,
                init_src,
                surface_format,
            );

            let uniform_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("uniforms"),
                size: pipeline::uniform_buffer_size(T::PARAM_DEFAULTS.len()),
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            let viewport = [size.width as f32, size.height as f32];
            let default_zoom = (size.width as f32 / self.config.width as f32)
                .min(size.height as f32 / self.config.height as f32);

            let params: Vec<f32> = T::PARAM_DEFAULTS.to_vec();
            let param_names: Vec<String> = T::PARAM_NAMES.iter().map(|s| s.to_string()).collect();

            let mut gpu = GpuState {
                device,
                queue,
                surface,
                surface_config,
                pipelines,
                textures,
                uniform_buffer,
                tick: 0,
                ticks_per_frame: self.config.ticks_per_frame,
                paused: self.config.paused,
                texture_count,
                zoom: default_zoom,
                default_zoom,
                camera: [self.config.width as f32 / 2.0, self.config.height as f32 / 2.0],
                viewport,
                dragging: false,
                last_mouse: [0.0, 0.0],
                params,
                param_names,
                selected_param: 0,
            };

            // Initialize state
            if T::HAS_INIT {
                gpu.run_init_shader();
            } else {
                let defaults = T::defaults();
                gpu.textures.write_defaults(&gpu.queue, &defaults);
            }

            gpu
        });

        self.gpu = Some(gpu);
        self.window.as_ref().unwrap().request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: winit::window::WindowId, event: WindowEvent) {
        let Some(gpu) = self.gpu.as_mut() else { return };
        let Some(window) = self.window.as_ref() else { return };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput {
                event: KeyEvent {
                    physical_key: PhysicalKey::Code(key),
                    state: ElementState::Pressed,
                    ..
                },
                ..
            } => {
                match key {
                    KeyCode::Escape => event_loop.exit(),
                    KeyCode::Space => {
                        gpu.paused = !gpu.paused;
                        window.request_redraw();
                    }
                    KeyCode::ArrowRight => {
                        if gpu.paused {
                            gpu.run_tick();
                            window.request_redraw();
                        }
                    }
                    KeyCode::Equal => {
                        gpu.ticks_per_frame = (gpu.ticks_per_frame + 1).min(64);
                    }
                    KeyCode::Minus => {
                        gpu.ticks_per_frame = gpu.ticks_per_frame.saturating_sub(1).max(1);
                    }
                    KeyCode::KeyR => {
                        gpu.reset::<T>();
                        window.request_redraw();
                    }
                    KeyCode::Tab => {
                        if !gpu.params.is_empty() {
                            gpu.selected_param = (gpu.selected_param + 1) % gpu.params.len();
                            gpu.print_param();
                        }
                    }
                    KeyCode::BracketLeft => {
                        if !gpu.params.is_empty() {
                            gpu.params[gpu.selected_param] /= 1.05;
                            gpu.print_param();
                        }
                    }
                    KeyCode::BracketRight => {
                        if !gpu.params.is_empty() {
                            gpu.params[gpu.selected_param] *= 1.05;
                            gpu.print_param();
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::PinchGesture { delta, .. } => {
                gpu.zoom *= 1.0 + delta as f32;
                gpu.zoom = gpu.zoom.max(0.1);
                window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                match delta {
                    MouseScrollDelta::LineDelta(_, y) => {
                        gpu.zoom *= 1.1_f32.powf(y);
                        gpu.zoom = gpu.zoom.max(0.1);
                        window.request_redraw();
                    }
                    MouseScrollDelta::PixelDelta(d) => {
                        gpu.camera[0] -= d.x as f32 / gpu.zoom;
                        gpu.camera[1] -= d.y as f32 / gpu.zoom;
                        window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput { button: MouseButton::Left, state, .. } => {
                gpu.dragging = state == ElementState::Pressed;
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pos = [position.x as f32, position.y as f32];
                if gpu.dragging {
                    let dx = pos[0] - gpu.last_mouse[0];
                    let dy = pos[1] - gpu.last_mouse[1];
                    gpu.camera[0] -= dx / gpu.zoom;
                    gpu.camera[1] -= dy / gpu.zoom;
                    window.request_redraw();
                }
                gpu.last_mouse = pos;
            }
            WindowEvent::RedrawRequested => {
                if !gpu.paused {
                    for _ in 0..gpu.ticks_per_frame {
                        gpu.run_tick();
                    }
                }
                gpu.render(window);
            }
            _ => {}
        }
    }
}
