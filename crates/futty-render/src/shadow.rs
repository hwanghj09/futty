use glam::{Mat4, Vec3};
use wgpu as gpu;

use super::{DEPTH, Draw};

const RESOLUTION: u32 = 2048;

pub(super) struct ShadowMap {
    depth: gpu::TextureView,
    pipeline: gpu::RenderPipeline,
    transform: gpu::Buffer,
    transform_group: gpu::BindGroup,
    pub(super) sample_group: gpu::BindGroup,
    size: u32,
}

impl ShadowMap {
    pub(super) fn sample_layout(device: &gpu::Device) -> gpu::BindGroupLayout {
        device.create_bind_group_layout(&gpu::BindGroupLayoutDescriptor {
            label: Some("Directional shadow sampling"),
            entries: &[
                gpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: gpu::ShaderStages::FRAGMENT,
                    ty: gpu::BindingType::Texture {
                        sample_type: gpu::TextureSampleType::Depth,
                        view_dimension: gpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                gpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: gpu::ShaderStages::FRAGMENT,
                    ty: gpu::BindingType::Sampler(gpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        })
    }

    pub(super) fn new(
        device: &gpu::Device,
        sample_layout: &gpu::BindGroupLayout,
        vertex_buffers: &[Option<gpu::VertexBufferLayout<'_>>],
    ) -> Self {
        let size = RESOLUTION.min(device.limits().max_texture_dimension_2d);
        let depth = device
            .create_texture(&gpu::TextureDescriptor {
                label: Some("Directional shadow depth"),
                size: gpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                format: DEPTH,
                mip_level_count: 1,
                sample_count: 1,
                dimension: gpu::TextureDimension::D2,
                usage: gpu::TextureUsages::TEXTURE_BINDING | gpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let sampler = device.create_sampler(&gpu::SamplerDescriptor {
            compare: Some(gpu::CompareFunction::LessEqual),
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });
        let sample_group = device.create_bind_group(&gpu::BindGroupDescriptor {
            label: Some("Directional shadow map"),
            layout: sample_layout,
            entries: &[
                gpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu::BindingResource::TextureView(&depth),
                },
                gpu::BindGroupEntry {
                    binding: 1,
                    resource: gpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let transform = device.create_buffer(&gpu::BufferDescriptor {
            label: Some("Shadow view-projection"),
            size: 64,
            usage: gpu::BufferUsages::UNIFORM | gpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let transform_layout = device.create_bind_group_layout(&gpu::BindGroupLayoutDescriptor {
            label: Some("Shadow transform layout"),
            entries: &[gpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: gpu::ShaderStages::VERTEX,
                ty: gpu::BindingType::Buffer {
                    ty: gpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: gpu::BufferSize::new(64),
                },
                count: None,
            }],
        });
        let transform_group = device.create_bind_group(&gpu::BindGroupDescriptor {
            label: Some("Shadow transform"),
            layout: &transform_layout,
            entries: &[gpu::BindGroupEntry {
                binding: 0,
                resource: transform.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&gpu::PipelineLayoutDescriptor {
            label: Some("Shadow pipeline layout"),
            bind_group_layouts: &[Some(&transform_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(gpu::ShaderModuleDescriptor {
            label: Some("Directional shadow shader"),
            source: gpu::ShaderSource::Wgsl(include_str!("../../../shaders/shadow.wgsl").into()),
        });
        let pipeline = device.create_render_pipeline(&gpu::RenderPipelineDescriptor {
            label: Some("Directional shadow pipeline"),
            layout: Some(&layout),
            vertex: gpu::VertexState {
                module: &shader,
                entry_point: Some("vs_shadow"),
                compilation_options: Default::default(),
                buffers: vertex_buffers,
            },
            primitive: Default::default(),
            depth_stencil: Some(gpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(true),
                depth_compare: Some(gpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: gpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 1.5,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            fragment: None,
            multiview_mask: None,
            cache: None,
        });
        Self {
            depth,
            pipeline,
            transform,
            transform_group,
            sample_group,
            size,
        }
    }

    pub(super) fn projection(&self, eye: Vec3, toward_light: Vec3, distance: f32) -> Mat4 {
        use glam::camera::rh::{proj::directx, view};
        let forward = -toward_light;
        let reference = if forward.y.abs() > 0.99 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let right = forward.cross(reference).normalize();
        let up = right.cross(forward);
        let cell = 2.0 * distance / self.size as f32;
        let snap = |axis: Vec3| {
            let coordinate = eye.dot(axis);
            axis * ((coordinate / cell).round() * cell - coordinate)
        };
        let center = eye + snap(right) + snap(up);
        let depth = 3.0 * distance;
        let view = view::look_to_mat4(center + toward_light * depth, forward, up);
        directx::orthographic(-distance, distance, -distance, distance, 0.01, 2.0 * depth) * view
    }

    pub(super) fn render(
        &self,
        encoder: &mut gpu::CommandEncoder,
        queue: &gpu::Queue,
        instance_buffer: &gpu::Buffer,
        draws: &[Draw<'_>],
        first_instance: usize,
        projection: Mat4,
    ) {
        queue.write_buffer(
            &self.transform,
            0,
            bytemuck::cast_slice(&projection.to_cols_array()),
        );
        let mut pass = encoder.begin_render_pass(&gpu::RenderPassDescriptor {
            label: Some("Draw directional shadow depth"),
            color_attachments: &[],
            depth_stencil_attachment: Some(gpu::RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(gpu::Operations {
                    load: gpu::LoadOp::Clear(1.0),
                    store: gpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.transform_group, &[]);
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        for (index, draw) in draws.iter().enumerate() {
            draw.buffers.bind(&mut pass);
            let index = (first_instance + index) as u32;
            pass.draw_indexed(draw.indices.clone(), 0, index..index + 1);
        }
    }
}
