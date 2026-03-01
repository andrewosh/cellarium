use wgpu::*;

pub struct Pipelines {
    pub sim_pipeline: ComputePipeline,
    pub sim_bind_group_layout: BindGroupLayout,
    pub view_pipeline: RenderPipeline,
    pub view_bind_group_layout: BindGroupLayout,
    pub init_pipeline: Option<RenderPipeline>,
    pub init_bind_group_layout: Option<BindGroupLayout>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub tick: u32,
    pub zoom: f32,
    pub resolution: [f32; 2],
    pub camera: [f32; 2],
    pub viewport: [f32; 2],
}

pub fn uniform_buffer_size(param_count: usize) -> u64 {
    let param_vec4s = (param_count + 3) / 4;
    (std::mem::size_of::<Uniforms>() + param_vec4s * 16) as u64
}

pub fn create_pipelines(
    device: &Device,
    texture_count: u32,
    update_shader_src: &str,
    view_shader_src: &str,
    init_shader_src: Option<&str>,
    surface_format: TextureFormat,
) -> Pipelines {
    // Simulation compute pipeline
    let sim_bind_group_layout = create_compute_bind_group_layout(device, texture_count);
    let sim_shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("sim_compute_shader"),
        source: ShaderSource::Wgsl(update_shader_src.into()),
    });
    let sim_pipeline = create_compute_pipeline(device, &sim_shader, &sim_bind_group_layout);

    // View pipeline (stays as render pipeline)
    let view_bind_group_layout = create_bind_group_layout(device, texture_count, "view");
    let view_shader = device.create_shader_module(ShaderModuleDescriptor {
        label: Some("view_shader"),
        source: ShaderSource::Wgsl(view_shader_src.into()),
    });
    let view_pipeline = create_view_pipeline(device, &view_shader, &view_bind_group_layout, surface_format);

    // Init pipeline (optional, stays as render pipeline)
    let (init_pipeline, init_bind_group_layout) = if let Some(init_src) = init_shader_src {
        let init_bgl = create_init_bind_group_layout(device);
        let init_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("init_shader"),
            source: ShaderSource::Wgsl(init_src.into()),
        });
        let init_pl = create_init_pipeline(device, &init_shader, &init_bgl, texture_count);
        (Some(init_pl), Some(init_bgl))
    } else {
        (None, None)
    };

    Pipelines {
        sim_pipeline,
        sim_bind_group_layout,
        view_pipeline,
        view_bind_group_layout,
        init_pipeline,
        init_bind_group_layout,
    }
}

fn create_compute_bind_group_layout(device: &Device, texture_count: u32) -> BindGroupLayout {
    let mut entries: Vec<BindGroupLayoutEntry> = Vec::new();

    // Read textures (bindings 0..texture_count-1)
    for i in 0..texture_count {
        entries.push(BindGroupLayoutEntry {
            binding: i,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: false },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
    }

    // Uniform buffer
    entries.push(BindGroupLayoutEntry {
        binding: texture_count,
        visibility: ShaderStages::COMPUTE,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    });

    // Write textures (storage, bindings texture_count+1..texture_count*2)
    for i in 0..texture_count {
        entries.push(BindGroupLayoutEntry {
            binding: texture_count + 1 + i,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::StorageTexture {
                access: StorageTextureAccess::WriteOnly,
                format: TextureFormat::Rgba32Float,
                view_dimension: TextureViewDimension::D2,
            },
            count: None,
        });
    }

    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("sim_compute"),
        entries: &entries,
    })
}

fn create_compute_pipeline(
    device: &Device,
    shader: &ShaderModule,
    bind_group_layout: &BindGroupLayout,
) -> ComputePipeline {
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("sim_compute_pipeline_layout"),
        bind_group_layouts: &[bind_group_layout],
        immediate_size: 0,
    });

    device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: Some("sim_compute_pipeline"),
        layout: Some(&pipeline_layout),
        module: shader,
        entry_point: Some("cs_main"),
        compilation_options: Default::default(),
        cache: None,
    })
}

fn create_bind_group_layout(device: &Device, texture_count: u32, label: &str) -> BindGroupLayout {
    let mut entries: Vec<BindGroupLayoutEntry> = Vec::new();

    // State textures (read)
    for i in 0..texture_count {
        entries.push(BindGroupLayoutEntry {
            binding: i,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: false },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
    }

    // Uniforms
    entries.push(BindGroupLayoutEntry {
        binding: texture_count,
        visibility: ShaderStages::FRAGMENT,
        ty: BindingType::Buffer {
            ty: BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    });

    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &entries,
    })
}

fn create_init_bind_group_layout(device: &Device) -> BindGroupLayout {
    // Init shader only needs uniforms (no state textures to read)
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("init"),
        entries: &[BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    })
}

fn create_view_pipeline(
    device: &Device,
    shader: &ShaderModule,
    bind_group_layout: &BindGroupLayout,
    surface_format: TextureFormat,
) -> RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("view_pipeline_layout"),
        bind_group_layouts: &[bind_group_layout],
        immediate_size: 0,
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("view_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &[Some(ColorTargetState {
                format: surface_format,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn create_init_pipeline(
    device: &Device,
    shader: &ShaderModule,
    bind_group_layout: &BindGroupLayout,
    texture_count: u32,
) -> RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("init_pipeline_layout"),
        bind_group_layouts: &[bind_group_layout],
        immediate_size: 0,
    });

    let targets: Vec<Option<ColorTargetState>> = (0..texture_count)
        .map(|_| Some(ColorTargetState {
            format: TextureFormat::Rgba32Float,
            blend: None,
            write_mask: ColorWrites::ALL,
        }))
        .collect();

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("init_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: shader,
            entry_point: Some("fs_main"),
            targets: &targets,
            compilation_options: Default::default(),
        }),
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

pub fn create_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    texture_views: &[&TextureView],
    uniform_buffer: &wgpu::Buffer,
    texture_count: u32,
) -> BindGroup {
    let mut entries: Vec<BindGroupEntry> = Vec::new();

    for i in 0..texture_count as usize {
        entries.push(BindGroupEntry {
            binding: i as u32,
            resource: BindingResource::TextureView(texture_views[i]),
        });
    }

    entries.push(BindGroupEntry {
        binding: texture_count,
        resource: uniform_buffer.as_entire_binding(),
    });

    device.create_bind_group(&BindGroupDescriptor {
        label: Some("bind_group"),
        layout,
        entries: &entries,
    })
}

pub fn create_compute_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    read_views: &[&TextureView],
    write_views: &[&TextureView],
    uniform_buffer: &wgpu::Buffer,
    texture_count: u32,
) -> BindGroup {
    let mut entries: Vec<BindGroupEntry> = Vec::new();

    // Read textures
    for i in 0..texture_count as usize {
        entries.push(BindGroupEntry {
            binding: i as u32,
            resource: BindingResource::TextureView(read_views[i]),
        });
    }

    // Uniform buffer
    entries.push(BindGroupEntry {
        binding: texture_count,
        resource: uniform_buffer.as_entire_binding(),
    });

    // Write textures (storage)
    for i in 0..texture_count as usize {
        entries.push(BindGroupEntry {
            binding: texture_count + 1 + i as u32,
            resource: BindingResource::TextureView(write_views[i]),
        });
    }

    device.create_bind_group(&BindGroupDescriptor {
        label: Some("compute_bind_group"),
        layout,
        entries: &entries,
    })
}
