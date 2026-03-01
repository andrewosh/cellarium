use wgpu::*;

pub struct TextureState {
    pub textures_a: Vec<Texture>,
    pub textures_b: Vec<Texture>,
    pub views_a: Vec<TextureView>,
    pub views_b: Vec<TextureView>,
    pub phase: bool, // false = read A write B, true = read B write A
    pub width: u32,
    pub height: u32,
    pub texture_count: u32,
}

impl TextureState {
    pub fn new(device: &Device, width: u32, height: u32, texture_count: u32) -> Self {
        let mut textures_a = Vec::new();
        let mut textures_b = Vec::new();
        let mut views_a = Vec::new();
        let mut views_b = Vec::new();

        for i in 0..texture_count {
            let desc = TextureDescriptor {
                label: Some(&format!("state_tex_{}", i)),
                size: Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba32Float,
                usage: TextureUsages::TEXTURE_BINDING
                    | TextureUsages::RENDER_ATTACHMENT
                    | TextureUsages::STORAGE_BINDING
                    | TextureUsages::COPY_DST,
                view_formats: &[],
            };

            let tex_a = device.create_texture(&desc);
            let tex_b = device.create_texture(&desc);
            views_a.push(tex_a.create_view(&TextureViewDescriptor::default()));
            views_b.push(tex_b.create_view(&TextureViewDescriptor::default()));
            textures_a.push(tex_a);
            textures_b.push(tex_b);
        }

        Self {
            textures_a,
            textures_b,
            views_a,
            views_b,
            phase: false,
            width,
            height,
            texture_count,
        }
    }

    pub fn read_views(&self) -> &[TextureView] {
        if self.phase { &self.views_b } else { &self.views_a }
    }

    pub fn write_views(&self) -> &[TextureView] {
        if self.phase { &self.views_a } else { &self.views_b }
    }

    pub fn read_textures(&self) -> &[Texture] {
        if self.phase { &self.textures_b } else { &self.textures_a }
    }

    pub fn write_textures(&self) -> &[Texture] {
        if self.phase { &self.textures_a } else { &self.textures_b }
    }

    pub fn swap(&mut self) {
        self.phase = !self.phase;
    }

    pub fn write_defaults(&self, queue: &Queue, defaults: &[[f32; 4]]) {
        // Write default values to both A and B textures
        let row_bytes = self.width as usize * 16; // 4 floats * 4 bytes each
        for (i, default_val) in defaults.iter().enumerate() {
            let mut data = vec![0u8; row_bytes * self.height as usize];
            for pixel in 0..((self.width * self.height) as usize) {
                let offset = pixel * 16;
                for (ch, val) in default_val.iter().enumerate() {
                    let bytes = val.to_le_bytes();
                    data[offset + ch * 4..offset + ch * 4 + 4].copy_from_slice(&bytes);
                }
            }

            let layout = TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes as u32),
                rows_per_image: Some(self.height),
            };
            let size = Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 };

            queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &self.textures_a[i],
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &data,
                layout,
                size,
            );
            queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &self.textures_b[i],
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                &data,
                layout,
                size,
            );
        }
    }
}
