use std::{collections::HashMap, ops::Range};

use bytemuck::{Pod, Zeroable};
use futty_assets::{AnimationSample, Clip, Model, Part, Rig, Vertex};
use futty_core::{Light, LightKind, Primitive, Surface};
use glam::{Mat4, Vec3};
use uuid::Uuid;
use wgpu::{self as gpu, util::DeviceExt};

const COLOR: gpu::TextureFormat = gpu::TextureFormat::Rgba8Unorm;
const DEPTH: gpu::TextureFormat = gpu::TextureFormat::Depth24Plus;

mod primitive;
mod shadow;

pub struct Targets {
    pub size: [u32; 2],
    pub color: gpu::TextureView,
    depth: gpu::TextureView,
}

impl Targets {
    pub fn new(device: &gpu::Device, size: [u32; 2]) -> Self {
        let texture = |label, format, usage| {
            device
                .create_texture(&gpu::TextureDescriptor {
                    label: Some(label),
                    size: gpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: gpu::TextureDimension::D2,
                    format,
                    usage: usage | gpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        Self {
            size,
            color: texture(
                "Scene color",
                COLOR,
                gpu::TextureUsages::TEXTURE_BINDING | gpu::TextureUsages::COPY_SRC,
            ),
            depth: texture("Scene depth", DEPTH, gpu::TextureUsages::empty()),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instance {
    model: [[f32; 4]; 4],
    color: [f32; 4],
    surface: [f32; 4],
}

impl Instance {
    fn new(model: Mat4, [r, g, b]: [f32; 3], mode: f32, surface: Surface) -> Self {
        Self {
            model: model.to_cols_array_2d(),
            color: [r, g, b, mode],
            surface: (Vec3::from(surface.emission) * surface.emission_strength)
                .extend(surface.roughness)
                .to_array(),
        }
    }
}

pub struct DrawObject {
    pub matrix: Mat4,
    pub color: [f32; 3],
    pub selected: bool,
    pub mesh: Option<Uuid>,
    pub primitive: Primitive,
    pub texture: Option<Uuid>,
    pub sprite: bool,
    pub surface: Surface,
    pub material_override: bool,
    pub animation: Option<AnimationSample>,
}

pub const MAX_LIGHTS: usize = 8;

pub struct LightInstance {
    pub light: Light,
    pub position: Vec3,
    pub toward_light: Vec3,
}

pub struct RenderScene<'a> {
    pub view_projection: Mat4,
    pub camera_position: Vec3,
    pub sky_clip_to_world: Option<Mat4>,
    pub objects: &'a [DrawObject],
    pub lights: &'a [LightInstance],
    pub grid: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LightUniform {
    // A zero range identifies a directional light; xyz is its direction toward the light.
    position_range: [f32; 4],
    color_intensity: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SceneUniform {
    view_projection: [[f32; 4]; 4],
    shadow_view_projection: [[f32; 4]; 4],
    sky_clip_to_world: [[f32; 4]; 4],
    sky_viewport: [f32; 4],
    shadow_region: [f32; 4],
    lighting: [f32; 4],
    lights: [LightUniform; MAX_LIGHTS],
}

impl SceneUniform {
    fn new(
        scene: &RenderScene<'_>,
        shadow: &shadow::ShadowMap,
        [width, height]: [u32; 2],
    ) -> (Self, Option<Mat4>) {
        let mut lights = [LightUniform::zeroed(); MAX_LIGHTS];
        let mut count = 0;
        let mut shadow_light = 0.0;
        let mut shadow_distance = 0.0;
        let mut shadow_projection = None;
        for (slot, instance) in lights
            .iter_mut()
            .zip(scene.lights.iter().filter(|instance| {
                instance.light.valid()
                    && instance.position.is_finite()
                    && instance.toward_light.is_finite()
            }))
        {
            let light = instance.light;
            let position_range = match light.kind {
                LightKind::Directional => {
                    let direction = instance.toward_light.normalize_or_zero();
                    // shortcut: use the first shadow-casting sun, add more maps for multiple shadowed suns.
                    if shadow_projection.is_none()
                        && light.cast_shadows
                        && light.intensity > 0.0
                        && direction != Vec3::ZERO
                        && scene.camera_position.is_finite()
                    {
                        shadow_light = (count + 1) as f32;
                        shadow_distance = light.shadow_distance;
                        shadow_projection = Some(shadow.projection(
                            scene.camera_position,
                            direction,
                            shadow_distance,
                        ));
                    }
                    direction.extend(0.0)
                }
                LightKind::Point => instance.position.extend(light.range),
            };
            *slot = LightUniform {
                position_range: position_range.to_array(),
                color_intensity: Vec3::from(light.color).extend(light.intensity).to_array(),
            };
            count += 1;
        }
        let ambient = if count == 0 {
            lights[0] = LightUniform {
                position_range: Vec3::new(0.35, 1.0, 0.65)
                    .normalize()
                    .extend(0.0)
                    .to_array(),
                color_intensity: [1.0, 1.0, 1.0, 0.57],
            };
            count = 1;
            0.43
        } else {
            0.22
        };
        (
            Self {
                view_projection: scene.view_projection.to_cols_array_2d(),
                shadow_view_projection: shadow_projection
                    .unwrap_or(Mat4::IDENTITY)
                    .to_cols_array_2d(),
                sky_clip_to_world: scene
                    .sky_clip_to_world
                    .unwrap_or(Mat4::ZERO)
                    .to_cols_array_2d(),
                sky_viewport: [
                    2.0 / width as f32,
                    -2.0 / height as f32,
                    u8::from(scene.sky_clip_to_world.is_some()) as f32,
                    0.0,
                ],
                shadow_region: scene.camera_position.extend(shadow_distance).to_array(),
                lighting: [ambient, count as f32, shadow_light, 0.0],
                lights,
            },
            shadow_projection,
        )
    }
}

struct MeshBuffers {
    vertices: gpu::Buffer,
    indices: gpu::Buffer,
}

impl MeshBuffers {
    fn new(device: &gpu::Device, vertices: &[Vertex], indices: &[u32]) -> Self {
        let vertices = device.create_buffer_init(&gpu::util::BufferInitDescriptor {
            label: Some("Mesh vertices"),
            contents: bytemuck::cast_slice(vertices),
            usage: gpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&gpu::util::BufferInitDescriptor {
            label: Some("Mesh indices"),
            contents: bytemuck::cast_slice(indices),
            usage: gpu::BufferUsages::INDEX,
        });
        Self { vertices, indices }
    }

    fn bind(&self, pass: &mut gpu::RenderPass<'_>) {
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), gpu::IndexFormat::Uint32);
    }
}

struct GpuModel {
    buffers: MeshBuffers,
    parts: Vec<Part>,
    textures: Vec<gpu::BindGroup>,
    bounds: [Vec3; 2],
    rig: Option<Rig>,
}

struct Draw<'a> {
    buffers: &'a MeshBuffers,
    indices: Range<u32>,
    texture: &'a gpu::BindGroup,
    deformed: Option<gpu::BufferSlice<'a>>,
}

impl Draw<'_> {
    fn bind(&self, pass: &mut gpu::RenderPass<'_>) {
        pass.set_vertex_buffer(
            0,
            self.deformed
                .unwrap_or_else(|| self.buffers.vertices.slice(..)),
        );
        pass.set_index_buffer(self.buffers.indices.slice(..), gpu::IndexFormat::Uint32);
    }
}

pub struct Renderer {
    pipeline: gpu::RenderPipeline,
    sprite_pipeline: gpu::RenderPipeline,
    sky_pipeline: gpu::RenderPipeline,
    shadow: shadow::ShadowMap,
    camera: gpu::Buffer,
    camera_group: gpu::BindGroup,
    primitives: [(MeshBuffers, u32); Primitive::ALL.len()],
    quad: MeshBuffers,
    texture_layout: gpu::BindGroupLayout,
    sampler: gpu::Sampler,
    white: gpu::BindGroup,
    models: HashMap<Uuid, GpuModel>,
    textures: HashMap<Uuid, (gpu::BindGroup, [u32; 2])>,
    instances: gpu::Buffer,
    animation_vertices: gpu::Buffer,
    grid: Vec<Instance>,
}

impl Renderer {
    pub fn new(device: &gpu::Device, queue: &gpu::Queue) -> Self {
        let shader = device.create_shader_module(gpu::ShaderModuleDescriptor {
            label: Some("Futty viewport shader"),
            source: gpu::ShaderSource::Wgsl(include_str!("../../../shaders/standard.wgsl").into()),
        });
        let camera_layout = device.create_bind_group_layout(&gpu::BindGroupLayoutDescriptor {
            label: Some("Camera layout"),
            entries: &[gpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: gpu::ShaderStages::VERTEX_FRAGMENT,
                ty: gpu::BindingType::Buffer {
                    ty: gpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: gpu::BufferSize::new(size_of::<SceneUniform>() as u64),
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&gpu::BindGroupLayoutDescriptor {
            label: Some("Texture layout"),
            entries: &[
                gpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: gpu::ShaderStages::FRAGMENT,
                    ty: gpu::BindingType::Texture {
                        sample_type: gpu::TextureSampleType::Float { filterable: true },
                        view_dimension: gpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                gpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: gpu::ShaderStages::FRAGMENT,
                    ty: gpu::BindingType::Sampler(gpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shadow_layout = shadow::ShadowMap::sample_layout(device);
        let layout = device.create_pipeline_layout(&gpu::PipelineLayoutDescriptor {
            label: Some("Scene pipeline layout"),
            bind_group_layouts: &[
                Some(&camera_layout),
                Some(&texture_layout),
                Some(&shadow_layout),
            ],
            immediate_size: 0,
        });
        let solid_targets = [Some(COLOR.into())];
        let sprite_targets = [Some(gpu::ColorTargetState {
            format: COLOR,
            blend: Some(gpu::BlendState::ALPHA_BLENDING),
            write_mask: gpu::ColorWrites::ALL,
        })];
        let mut descriptor = gpu::RenderPipelineDescriptor {
            label: Some("Futty viewport pipeline"),
            layout: Some(&layout),
            vertex: gpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[
                    Some(gpu::VertexBufferLayout {
                        array_stride: size_of::<Vertex>() as u64,
                        step_mode: gpu::VertexStepMode::Vertex,
                        attributes: &gpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
                    }),
                    Some(gpu::VertexBufferLayout {
                        array_stride: size_of::<Instance>() as u64,
                        step_mode: gpu::VertexStepMode::Instance,
                        attributes: &gpu::vertex_attr_array![3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4, 8 => Float32x4],
                    }),
                ],
            },
            primitive: gpu::PrimitiveState {
                cull_mode: Some(gpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(gpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(true),
                depth_compare: Some(gpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(gpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &solid_targets,
            }),
            multiview_mask: None,
            cache: None,
        };
        let pipeline = device.create_render_pipeline(&descriptor);
        descriptor.label = Some("Sprite3D pipeline");
        descriptor.primitive.cull_mode = None;
        descriptor
            .depth_stencil
            .as_mut()
            .unwrap()
            .depth_write_enabled = Some(false);
        descriptor.fragment.as_mut().unwrap().targets = &sprite_targets;
        let sprite_pipeline = device.create_render_pipeline(&descriptor);
        let shadow = shadow::ShadowMap::new(device, &shadow_layout, descriptor.vertex.buffers);
        let sky_layout = device.create_pipeline_layout(&gpu::PipelineLayoutDescriptor {
            label: Some("Sky pipeline layout"),
            bind_group_layouts: &[Some(&camera_layout)],
            immediate_size: 0,
        });
        descriptor.label = Some("Sky pipeline");
        descriptor.layout = Some(&sky_layout);
        descriptor.vertex.entry_point = Some("vs_sky");
        descriptor.vertex.buffers = &[];
        descriptor.depth_stencil.as_mut().unwrap().depth_compare =
            Some(gpu::CompareFunction::Always);
        let fragment = descriptor.fragment.as_mut().unwrap();
        fragment.entry_point = Some("fs_sky");
        fragment.targets = &solid_targets;
        let sky_pipeline = device.create_render_pipeline(&descriptor);
        let camera = device.create_buffer(&gpu::BufferDescriptor {
            label: Some("View projection"),
            size: size_of::<SceneUniform>() as u64,
            usage: gpu::BufferUsages::UNIFORM | gpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let camera_group = device.create_bind_group(&gpu::BindGroupDescriptor {
            label: Some("Viewport camera"),
            layout: &camera_layout,
            entries: &[gpu::BindGroupEntry {
                binding: 0,
                resource: camera.as_entire_binding(),
            }],
        });
        let primitives = Primitive::ALL.map(|shape| {
            let (vertices, indices) = primitive::mesh(shape);
            (
                MeshBuffers::new(device, &vertices, &indices),
                indices.len() as u32,
            )
        });
        let quad = MeshBuffers::new(device, &sprite_vertices(), &[0, 1, 2, 0, 2, 3]);
        let sampler = device.create_sampler(&gpu::SamplerDescriptor {
            address_mode_u: gpu::AddressMode::Repeat,
            address_mode_v: gpu::AddressMode::Repeat,
            mag_filter: gpu::FilterMode::Linear,
            min_filter: gpu::FilterMode::Linear,
            ..Default::default()
        });
        let white = upload_texture(device, queue, &texture_layout, &sampler, [1, 1], &[255; 4]);
        let grid = grid();
        let instances = Self::instance_buffer(device, 128);
        Self {
            pipeline,
            sprite_pipeline,
            sky_pipeline,
            shadow,
            camera,
            camera_group,
            primitives,
            quad,
            texture_layout,
            sampler,
            white,
            models: HashMap::new(),
            textures: HashMap::new(),
            instances,
            animation_vertices: Self::animation_buffer(device, size_of::<Vertex>() as u64),
            grid,
        }
    }

    pub fn clear_assets(&mut self) {
        self.models.clear();
        self.textures.clear();
    }

    pub fn upload_model(
        &mut self,
        device: &gpu::Device,
        queue: &gpu::Queue,
        id: Uuid,
        model: Model,
    ) {
        let buffers = MeshBuffers::new(device, &model.vertices, &model.indices);
        let textures = model
            .textures
            .iter()
            .map(|image| {
                upload_texture(
                    device,
                    queue,
                    &self.texture_layout,
                    &self.sampler,
                    [image.width(), image.height()],
                    image.as_raw(),
                )
            })
            .collect();
        self.models.insert(
            id,
            GpuModel {
                buffers,
                textures,
                parts: model.parts,
                bounds: model.bounds,
                rig: model.rig,
            },
        );
    }

    pub fn upload_image(
        &mut self,
        device: &gpu::Device,
        queue: &gpu::Queue,
        id: Uuid,
        size: [u32; 2],
        pixels: &[u8],
    ) {
        self.textures.insert(
            id,
            (
                upload_texture(
                    device,
                    queue,
                    &self.texture_layout,
                    &self.sampler,
                    size,
                    pixels,
                ),
                size,
            ),
        );
    }

    pub fn texture_size(&self, id: Uuid) -> Option<[u32; 2]> {
        self.textures.get(&id).map(|(_, size)| *size)
    }

    pub fn clips(&self, model: Uuid) -> &[Clip] {
        self.models
            .get(&model)
            .and_then(|model| model.rig.as_ref())
            .map_or(&[], Rig::clips)
    }

    pub fn bounds(&self, mesh: Option<Uuid>, primitive: Primitive) -> [Vec3; 2] {
        mesh.and_then(|id| self.models.get(&id)).map_or_else(
            || {
                if mesh.is_some() {
                    Primitive::Cube.bounds()
                } else {
                    primitive.bounds()
                }
            },
            |model| model.bounds,
        )
    }

    fn instance_buffer(device: &gpu::Device, capacity: usize) -> gpu::Buffer {
        device.create_buffer(&gpu::BufferDescriptor {
            label: Some("Viewport instances"),
            size: (capacity * size_of::<Instance>()) as u64,
            usage: gpu::BufferUsages::VERTEX | gpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn animation_buffer(device: &gpu::Device, size: u64) -> gpu::Buffer {
        device.create_buffer(&gpu::BufferDescriptor {
            label: Some("Animated mesh vertices"),
            size,
            usage: gpu::BufferUsages::VERTEX | gpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn animate_objects(
        &mut self,
        device: &gpu::Device,
        queue: &gpu::Queue,
        objects: &[DrawObject],
    ) -> HashMap<usize, Range<u64>> {
        // shortcut: deform up to one million vertices per view on the CPU, move skinning to the GPU for larger scenes.
        let stride = size_of::<Vertex>() as u64;
        let capacity = device.limits().max_buffer_size.min(1_000_000 * stride);
        let mut vertices = Vec::new();
        let mut ranges = HashMap::new();
        for (index, object) in objects.iter().enumerate() {
            let Some(sample) = object.animation else {
                continue;
            };
            let Some(rig) = object
                .mesh
                .and_then(|id| self.models.get(&id))
                .and_then(|model| model.rig.as_ref())
            else {
                continue;
            };
            let start = vertices.len();
            if (start + rig.vertex_count()) as u64 * stride <= capacity
                && rig.sample(sample, &mut vertices)
            {
                ranges.insert(index, start as u64 * stride..vertices.len() as u64 * stride);
            }
        }
        let bytes = bytemuck::cast_slice(&vertices);
        if !bytes.is_empty() {
            let size = bytes.len() as u64;
            if self.animation_vertices.size() < size {
                self.animation_vertices =
                    Self::animation_buffer(device, size.next_power_of_two().min(capacity));
            }
            queue.write_buffer(&self.animation_vertices, 0, bytes);
        }
        ranges
    }

    pub fn draw(
        &mut self,
        device: &gpu::Device,
        queue: &gpu::Queue,
        targets: &Targets,
        scene: RenderScene<'_>,
    ) {
        let (uniform, shadow_projection) = SceneUniform::new(&scene, &self.shadow, targets.size);
        let RenderScene {
            view_projection,
            objects,
            grid: show_grid,
            ..
        } = scene;
        let animated = self.animate_objects(device, queue, objects);
        let grid = if show_grid { self.grid.as_slice() } else { &[] };
        let mut instances = grid.to_vec();
        let mut draws = Vec::new();
        for (index, object) in objects
            .iter()
            .enumerate()
            .filter(|(_, object)| !object.sprite && object.matrix.is_finite())
        {
            let replacement = object
                .texture
                .and_then(|id| self.textures.get(&id))
                .map(|(group, _)| group);
            if let Some(model) = object.mesh.and_then(|id| self.models.get(&id)) {
                for part in &model.parts {
                    let texture = replacement
                        .or_else(|| {
                            if object.material_override {
                                None
                            } else {
                                part.texture.map(|index| &model.textures[index])
                            }
                        })
                        .unwrap_or(&self.white);
                    let color = if object.material_override {
                        object.color
                    } else {
                        (Vec3::from(object.color) * Vec3::from(part.color)).to_array()
                    };
                    instances.push(Instance::new(
                        object.matrix,
                        color,
                        if object.selected { 3.0 } else { 1.0 },
                        object.surface,
                    ));
                    draws.push(Draw {
                        buffers: &model.buffers,
                        indices: part.indices.clone(),
                        texture,
                        deformed: animated
                            .get(&index)
                            .map(|range| self.animation_vertices.slice(range.clone())),
                    });
                }
            } else {
                let (primitive, color) = if object.mesh.is_some() {
                    (Primitive::Cube, [0.18; 3])
                } else {
                    (object.primitive, object.color)
                };
                let (buffers, count) = &self.primitives[primitive as usize];
                instances.push(Instance::new(
                    object.matrix,
                    color,
                    if !object.selected {
                        1.0
                    } else if primitive.is_cube() {
                        2.0
                    } else {
                        3.0
                    },
                    object.surface,
                ));
                draws.push(Draw {
                    buffers,
                    indices: 0..*count,
                    texture: replacement.unwrap_or(&self.white),
                    deformed: None,
                });
            }
        }
        let first_sprite = draws.len();
        let mut sprites: Vec<_> = objects
            .iter()
            .filter(|object| object.sprite && object.matrix.is_finite())
            .map(|object| {
                let center = view_projection * object.matrix.w_axis;
                (center.z / center.w, object)
            })
            .collect();
        // shortcut: intersecting sprites sort by center depth, revisit if scenes need per-pixel ordering.
        sprites.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, object) in sprites {
            let texture = object
                .texture
                .and_then(|id| self.textures.get(&id))
                .map(|(group, _)| group);
            let color = if texture.is_some() {
                object.color
            } else {
                [0.25, 0.18, 0.12]
            };
            instances.push(Instance::new(
                object.matrix,
                color,
                if object.selected { 5.0 } else { 4.0 },
                object.surface,
            ));
            draws.push(Draw {
                buffers: &self.quad,
                indices: 0..6,
                texture: texture.unwrap_or(&self.white),
                deformed: None,
            });
        }
        if (instances.len() * size_of::<Instance>()) as u64 > self.instances.size() {
            self.instances = Self::instance_buffer(device, instances.len().next_power_of_two());
        }
        queue.write_buffer(&self.camera, 0, bytemuck::bytes_of(&uniform));
        if !instances.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&instances));
        }
        let mut encoder = device.create_command_encoder(&gpu::CommandEncoderDescriptor {
            label: Some("Render scene viewport"),
        });
        if let Some(projection) = shadow_projection {
            self.shadow.render(
                &mut encoder,
                queue,
                &self.instances,
                &draws[..first_sprite],
                grid.len(),
                projection,
            );
        }
        {
            let mut pass = encoder.begin_render_pass(&gpu::RenderPassDescriptor {
                label: Some("Draw scene"),
                color_attachments: &[Some(gpu::RenderPassColorAttachment {
                    view: &targets.color,
                    depth_slice: None,
                    resolve_target: None,
                    ops: gpu::Operations {
                        load: gpu::LoadOp::Clear(gpu::Color {
                            r: 0.065,
                            g: 0.065,
                            b: 0.065,
                            a: 1.0,
                        }),
                        store: gpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(gpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(gpu::Operations {
                        load: gpu::LoadOp::Clear(1.0),
                        store: gpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_bind_group(0, &self.camera_group, &[]);
            if scene.sky_clip_to_world.is_some() {
                pass.set_pipeline(&self.sky_pipeline);
                pass.draw(0..3, 0..1);
            }
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(2, &self.shadow.sample_group, &[]);
            pass.set_vertex_buffer(1, self.instances.slice(..));
            if !grid.is_empty() {
                self.primitives[Primitive::Cube as usize].0.bind(&mut pass);
                pass.set_bind_group(1, &self.white, &[]);
                pass.draw_indexed(0..36, 0, 0..grid.len() as u32);
            }
            for (index, draw) in draws.into_iter().enumerate() {
                if index == first_sprite {
                    pass.set_pipeline(&self.sprite_pipeline);
                }
                draw.bind(&mut pass);
                pass.set_bind_group(1, draw.texture, &[]);
                let index = (grid.len() + index) as u32;
                pass.draw_indexed(draw.indices, 0, index..index + 1);
            }
        }
        queue.submit([encoder.finish()]);
    }
}

fn sprite_vertices() -> [Vertex; 4] {
    [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]].map(|uv| Vertex {
        position: [uv[0] - 0.5, 0.5 - uv[1], 0.0],
        normal: [0.0, 0.0, 1.0],
        uv,
    })
}

fn upload_texture(
    device: &gpu::Device,
    queue: &gpu::Queue,
    layout: &gpu::BindGroupLayout,
    sampler: &gpu::Sampler,
    [width, height]: [u32; 2],
    pixels: &[u8],
) -> gpu::BindGroup {
    let size = gpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&gpu::TextureDescriptor {
        label: Some("Base color texture"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: gpu::TextureDimension::D2,
        format: gpu::TextureFormat::Rgba8UnormSrgb,
        usage: gpu::TextureUsages::TEXTURE_BINDING | gpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        pixels,
        gpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        size,
    );
    let view = texture.create_view(&Default::default());
    device.create_bind_group(&gpu::BindGroupDescriptor {
        label: Some("Base color"),
        layout,
        entries: &[
            gpu::BindGroupEntry {
                binding: 0,
                resource: gpu::BindingResource::TextureView(&view),
            },
            gpu::BindGroupEntry {
                binding: 1,
                resource: gpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

fn cube_mesh() -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for (normal, tangent) in [
        (Vec3::X, Vec3::Y),
        (Vec3::NEG_X, Vec3::Y),
        (Vec3::Y, Vec3::Z),
        (Vec3::NEG_Y, Vec3::Z),
        (Vec3::Z, Vec3::X),
        (Vec3::NEG_Z, Vec3::X),
    ] {
        let bitangent = normal.cross(tangent);
        let base = vertices.len() as u32;
        for uv in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
            let position = normal * 0.5 + tangent * (uv[0] - 0.5) + bitangent * (uv[1] - 0.5);
            vertices.push(Vertex {
                position: position.to_array(),
                normal: normal.to_array(),
                uv,
            });
        }
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (vertices, indices)
}

fn grid() -> Vec<Instance> {
    let mut grid = Vec::with_capacity(82);
    for offset in -20..=20 {
        let (x, z) = if offset == 0 {
            ([0.28, 0.22, 0.23], [0.20, 0.25, 0.32])
        } else {
            let gray = if offset % 5 == 0 { 0.21 } else { 0.12 };
            ([gray; 3], [gray; 3])
        };
        let offset = offset as f32;
        for (scale, position, color) in [
            (
                Vec3::new(40.0, 0.007, 0.014),
                Vec3::new(0.0, -0.01, offset),
                x,
            ),
            (
                Vec3::new(0.014, 0.007, 40.0),
                Vec3::new(offset, -0.013, 0.0),
                z,
            ),
        ] {
            grid.push(Instance::new(
                Mat4::from_translation(position) * Mat4::from_scale(scale),
                color,
                0.0,
                Surface::default(),
            ));
        }
    }
    grid
}

#[cfg(test)]
mod tests {
    use super::*;
    use futty_core::Camera;

    fn default_scene(view_projection: Mat4, objects: &[DrawObject]) -> RenderScene<'_> {
        RenderScene {
            view_projection,
            camera_position: Vec3::ZERO,
            sky_clip_to_world: None,
            objects,
            lights: &[],
            grid: false,
        }
    }

    #[test]
    #[ignore = "requires a graphics adapter"]
    fn sky_gradient_follows_camera_rotation_and_stays_behind_objects() {
        let instance = gpu::Instance::new(gpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut renderer = Renderer::new(&device, &queue);
        let targets = Targets::new(&device, [64, 64]);
        let mut camera = Camera {
            eye: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
        };
        let mut draw = |camera: Camera, objects: &[DrawObject]| {
            let mut scene = default_scene(camera.view_projection(1.0), objects);
            scene.camera_position = camera.eye;
            scene.sky_clip_to_world = Some(
                Camera {
                    eye: Vec3::ZERO,
                    ..camera
                }
                .view_projection(1.0)
                .inverse(),
            );
            renderer.draw(&device, &queue, &targets, scene);
            pixels(&device, &queue, &targets)
        };
        let level = draw(camera, &[]);
        let top = (4 * 64 + 32) * 4;
        let center = (32 * 64 + 32) * 4;
        let bottom = (59 * 64 + 32) * 4;
        assert!(level[top + 2] > level[bottom + 2]);
        camera.eye = Vec3::Z * 3.0;
        assert_eq!(draw(camera, &[]), level);
        let cube = DrawObject {
            matrix: Mat4::IDENTITY,
            color: [1.0, 0.0, 0.0],
            selected: false,
            mesh: None,
            texture: None,
            sprite: false,
            primitive: Primitive::Cube,
            surface: Surface::default(),
            material_override: true,
            animation: None,
        };
        let with_cube = draw(camera, &[cube]);
        assert_eq!(&with_cube[top..top + 4], &level[top..top + 4]);
        assert!(with_cube[center] > with_cube[center + 2]);
        camera.pitch = 0.8;
        let tilted = draw(camera, &[]);
        assert!(tilted[center] < level[center]);
    }

    #[test]
    #[ignore = "requires a graphics adapter"]
    fn roughness_changes_highlights_and_emission_survives_without_light() {
        use futty_core::{Light, LightKind};
        let instance = gpu::Instance::new(gpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut renderer = Renderer::new(&device, &queue);
        let targets = Targets::new(&device, [128, 128]);
        let camera = Camera {
            eye: Vec3::Z * 3.0,
            yaw: 0.0,
            pitch: 0.0,
        };
        let mut cube = DrawObject {
            matrix: Mat4::IDENTITY,
            color: [0.01; 3],
            selected: false,
            mesh: None,
            texture: None,
            sprite: false,
            primitive: Primitive::Cube,
            surface: Surface::default(),
            material_override: true,
            animation: None,
        };
        let sun = LightInstance {
            light: Light::new(LightKind::Directional),
            position: Vec3::ZERO,
            toward_light: Vec3::Z,
        };
        let mut sample = |object: &DrawObject, lights: &[LightInstance]| {
            renderer.draw(
                &device,
                &queue,
                &targets,
                RenderScene {
                    view_projection: camera.view_projection(1.0),
                    camera_position: camera.eye,
                    sky_clip_to_world: None,
                    objects: std::slice::from_ref(object),
                    lights,
                    grid: false,
                },
            );
            let rgba = pixels(&device, &queue, &targets);
            let center = (64 * 128 + 64) * 4;
            [rgba[center], rgba[center + 1], rgba[center + 2]]
        };
        cube.surface.roughness = 1.0;
        let matte = sample(&cube, std::slice::from_ref(&sun));
        cube.surface.roughness = 0.0;
        let smooth = sample(&cube, std::slice::from_ref(&sun));
        assert!(
            smooth[0] > matte[0].saturating_add(40),
            "{smooth:?} {matte:?}"
        );
        let mut off = sun;
        off.light.intensity = 0.0;
        let dark = sample(&cube, std::slice::from_ref(&off));
        cube.surface.emission = [0.8, 0.0, 0.0];
        cube.surface.emission_strength = 1.0;
        let glowing = sample(&cube, std::slice::from_ref(&off));
        assert!(
            glowing[0] > dark[0].saturating_add(100),
            "{glowing:?} {dark:?}"
        );
        assert_eq!(glowing[1..], dark[1..]);
    }

    #[test]
    #[ignore = "requires a local graphics adapter"]
    fn authored_lights_color_meshes_and_point_lights_respect_range() {
        let instance = gpu::Instance::new(gpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut renderer = Renderer::new(&device, &queue);
        let targets = Targets::new(&device, [128, 128]);
        let view = Camera {
            eye: Vec3::Z * 3.0,
            yaw: 0.0,
            pitch: 0.0,
        }
        .view_projection(1.0);
        let cube = DrawObject {
            surface: Surface::default(),
            material_override: false,
            animation: None,
            matrix: Mat4::IDENTITY,
            color: [0.24; 3],
            selected: false,
            mesh: None,
            texture: None,
            sprite: false,
            primitive: Primitive::Cube,
        };
        let mut sampled = |lights: &[LightInstance]| {
            renderer.draw(
                &device,
                &queue,
                &targets,
                RenderScene {
                    view_projection: view,
                    camera_position: Vec3::Z * 3.0,
                    sky_clip_to_world: None,
                    objects: std::slice::from_ref(&cube),
                    lights,
                    grid: false,
                },
            );
            let rgba = pixels(&device, &queue, &targets);
            let center = (64 * 128 + 64) * 4;
            [rgba[center], rgba[center + 1], rgba[center + 2]]
        };
        let sun = LightInstance {
            light: Light {
                color: [1.0, 0.0, 0.0],
                ..Light::new(LightKind::Directional)
            },
            position: Vec3::ZERO,
            toward_light: Vec3::Z,
        };
        let [red, green, blue] = sampled(&[sun]);
        assert!(red > green + 40 && green.abs_diff(blue) <= 1);
        let mut bulb = LightInstance {
            light: Light {
                color: [0.0, 1.0, 0.0],
                range: 6.0,
                ..Light::new(LightKind::Point)
            },
            position: Vec3::Z * 1.5,
            toward_light: Vec3::ZERO,
        };
        let [red, green, blue] = sampled(std::slice::from_ref(&bulb));
        assert!(green > red + 40 && red.abs_diff(blue) <= 1);
        bulb.light.range = 0.25;
        let [red, green, blue] = sampled(std::slice::from_ref(&bulb));
        assert!(red.abs_diff(green) <= 1 && green.abs_diff(blue) <= 1);
    }

    #[test]
    #[ignore = "requires a local graphics adapter"]
    fn directional_shadow_darkens_a_receiver_and_respects_shadow_distance() {
        let instance = gpu::Instance::new(gpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut renderer = Renderer::new(&device, &queue);
        let targets = Targets::new(&device, [128, 128]);
        let mut camera = Camera {
            eye: Vec3::new(4.0, 4.0, 6.0),
            yaw: 0.0,
            pitch: 0.0,
        };
        camera.look_at(Vec3::new(0.0, 0.6, 0.0));
        let view_projection = camera.view_projection(1.0);
        let object = |matrix, primitive| DrawObject {
            surface: Surface::default(),
            material_override: false,
            animation: None,
            matrix,
            color: [0.5; 3],
            selected: false,
            mesh: None,
            texture: None,
            sprite: false,
            primitive,
        };
        let objects = [
            object(
                Mat4::from_scale(Vec3::new(12.0, 1.0, 12.0)),
                Primitive::Plane,
            ),
            object(Mat4::from_translation(Vec3::Y * 1.3), Primitive::Cube),
        ];
        let mut sun = Light::new(LightKind::Directional);
        let mut frame = |light| {
            let source = LightInstance {
                light,
                position: Vec3::ZERO,
                toward_light: Vec3::new(-1.8, 2.0, 0.0),
            };
            renderer.draw(
                &device,
                &queue,
                &targets,
                RenderScene {
                    view_projection,
                    camera_position: camera.eye,
                    sky_clip_to_world: None,
                    objects: &objects,
                    lights: std::slice::from_ref(&source),
                    grid: false,
                },
            );
            let rgba = pixels(&device, &queue, &targets);
            let sample = |world| {
                let ndc = view_projection.project_point3(world);
                let x = ((ndc.x * 0.5 + 0.5) * 128.0) as usize;
                let y = ((0.5 - ndc.y * 0.5) * 128.0) as usize;
                rgba[(y * 128 + x) * 4]
            };
            [sample(Vec3::X * 1.17), sample(Vec3::new(-2.0, 0.0, -1.0))]
        };
        let [shadow, lit] = frame(sun);
        sun.cast_shadows = false;
        let [clear, unrelated] = frame(sun);
        assert!(clear > shadow + 25, "Shadow {shadow}, unshadowed {clear}");
        assert!(lit.abs_diff(unrelated) <= 2);
        sun.cast_shadows = true;
        sun.shadow_distance = 2.0;
        let [outside, _] = frame(sun);
        assert!(outside.abs_diff(clear) <= 2);
    }

    #[test]
    #[ignore = "requires a local graphics adapter"]
    fn renders_sprite_alpha_and_respects_opaque_depth() {
        let instance = gpu::Instance::new(gpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut renderer = Renderer::new(&device, &queue);
        let texture = Uuid::new_v4();
        renderer.upload_image(
            &device,
            &queue,
            texture,
            [2, 1],
            &[255, 0, 0, 0, 255, 0, 0, 128],
        );
        let targets = Targets::new(&device, [128, 128]);
        let camera = Camera {
            eye: Vec3::Z * 3.0,
            yaw: 0.0,
            pitch: 0.0,
        }
        .view_projection(1.0);
        let sprite = DrawObject {
            surface: Surface::default(),
            material_override: false,
            animation: None,
            matrix: Mat4::IDENTITY,
            color: [1.0; 3],
            selected: false,
            mesh: None,
            texture: Some(texture),
            sprite: true,
            primitive: Primitive::Cube,
        };
        renderer.draw(&device, &queue, &targets, default_scene(camera, &[]));
        let background = pixels(&device, &queue, &targets);
        renderer.draw(
            &device,
            &queue,
            &targets,
            default_scene(camera, std::slice::from_ref(&sprite)),
        );
        let image = pixels(&device, &queue, &targets);
        let left = (64 * 128 + 49) * 4;
        let right = (64 * 128 + 79) * 4;
        assert_eq!(&image[left..left + 4], &background[left..left + 4]);
        let blended = u16::from(image[right]);
        let expected = (u16::from(background[right]) + 255) / 2;
        assert!(
            blended.abs_diff(expected) <= 2,
            "Expected half red, got {blended}"
        );
        let solid = DrawObject {
            surface: Surface::default(),
            material_override: false,
            animation: None,
            matrix: Mat4::from_translation(Vec3::Z * 0.75),
            color: [0.0, 0.0, 1.0],
            selected: false,
            mesh: None,
            texture: None,
            sprite: false,
            primitive: Primitive::Cube,
        };
        renderer.draw(
            &device,
            &queue,
            &targets,
            default_scene(camera, &[sprite, solid]),
        );
        let occluded = pixels(&device, &queue, &targets);
        assert!(occluded[right + 2] > 100 && occluded[right] < 5);
    }

    #[test]
    #[ignore = "requires a local graphics adapter"]
    fn renders_model_and_texture_override() {
        let instance = gpu::Instance::new(gpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let model_id = Uuid::new_v4();
        let texture_id = Uuid::new_v4();
        let model = futty_assets::model(include_bytes!(
            "../../../examples/asset-preview/Workshop Crate.glb"
        ))
        .unwrap();
        let grid = futty_assets::png(include_bytes!(
            "../../../examples/asset-preview/Workshop Grid.png"
        ))
        .unwrap();
        let mut renderer = Renderer::new(&device, &queue);
        renderer.upload_model(&device, &queue, model_id, model);
        renderer.upload_image(
            &device,
            &queue,
            texture_id,
            [grid.width(), grid.height()],
            grid.as_raw(),
        );
        let targets = Targets::new(&device, [128, 128]);
        let mut object = DrawObject {
            surface: Surface::default(),
            material_override: false,
            animation: None,
            matrix: Mat4::IDENTITY,
            color: [1.0; 3],
            selected: false,
            mesh: Some(model_id),
            texture: None,
            sprite: false,
            primitive: Primitive::Cube,
        };
        let camera = Camera::default().view_projection(1.0);
        renderer.draw(
            &device,
            &queue,
            &targets,
            default_scene(camera, std::slice::from_ref(&object)),
        );
        let embedded = pixels(&device, &queue, &targets);
        assert!(
            embedded
                .as_chunks::<4>()
                .0
                .iter()
                .any(|rgba| rgba[0] > rgba[2].saturating_add(45))
        );
        object.texture = Some(texture_id);
        renderer.draw(
            &device,
            &queue,
            &targets,
            default_scene(camera, std::slice::from_ref(&object)),
        );
        let updated = pixels(&device, &queue, &targets);
        assert!(
            updated
                .as_chunks::<4>()
                .0
                .iter()
                .any(|rgba| rgba[2] > rgba[0].saturating_add(30))
        );
        assert_ne!(embedded, updated);
        object.material_override = true;
        object.texture = None;
        object.color = [0.0, 1.0, 0.0];
        renderer.draw(&device, &queue, &targets, default_scene(camera, &[object]));
        let overridden = pixels(&device, &queue, &targets);
        assert!(overridden.as_chunks::<4>().0.iter().any(|rgba| rgba[1]
            > rgba[0].saturating_add(60)
            && rgba[1] > rgba[2].saturating_add(60)));
    }

    fn pixels(device: &gpu::Device, queue: &gpu::Queue, targets: &Targets) -> Vec<u8> {
        let [width, height] = targets.size;
        let buffer = device.create_buffer(&gpu::BufferDescriptor {
            label: Some("Viewport test readback"),
            size: u64::from(width * height * 4),
            usage: gpu::BufferUsages::COPY_DST | gpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            targets.color.texture().as_image_copy(),
            gpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: gpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
            },
            gpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.map_async(gpu::MapMode::Read, .., move |result| {
            tx.send(result).unwrap()
        });
        device.poll(gpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        buffer.get_mapped_range(..).unwrap().to_vec()
    }
}
