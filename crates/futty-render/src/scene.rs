use std::collections::HashMap;

use futty_core::{Camera, CameraLens, CameraView, LightKind, Projection, Transform};
use futty_runtime::{Hierarchy, Runtime, Scene};
use glam::{Mat4, Vec3};
use uuid::Uuid;

use crate::{DrawObject, LightInstance, MAX_LIGHTS, RenderScene};

pub struct SceneFrame {
    pub world: Vec<Mat4>,
    pub local: Vec<Transform>,
    pub instances: Vec<DrawObject>,
    pub lights: Vec<LightInstance>,
    pub view: CameraView,
}

impl SceneFrame {
    pub fn render_scene(&self, aspect: f32, sky: bool) -> RenderScene<'_> {
        RenderScene {
            view_projection: self.view.view_projection(aspect),
            camera_position: self.view.eye,
            sky_clip_to_world: sky.then(|| {
                let lens = match self.view.lens.projection {
                    Projection::Perspective => self.view.lens,
                    Projection::Orthographic => CameraLens::default(),
                };
                CameraView {
                    eye: Vec3::ZERO,
                    lens,
                    ..self.view
                }
                .view_projection(aspect)
                .inverse()
            }),
            objects: &self.instances,
            lights: &self.lights,
            grid: false,
        }
    }
}

pub fn draw_objects(
    scene: &Scene,
    hierarchy: &Hierarchy,
    runtime: Option<&mut Runtime>,
    camera: Camera,
    camera_index: Option<usize>,
    selected: &[Uuid],
    preview: Option<&HashMap<Uuid, Transform>>,
) -> SceneFrame {
    let (state, animations, spawned) = runtime.map_or_else(
        || (HashMap::new(), HashMap::new(), &[][..]),
        |runtime| {
            let state = runtime
                .extract()
                .map(|(id, transform, color)| (id, (transform, color)))
                .collect();
            let animations = runtime.animated_objects().collect::<HashMap<_, _>>();
            (state, animations, runtime.spawned_objects())
        },
    );
    let mut combined = Vec::new();
    let mut combined_hierarchy = None;
    if !spawned.is_empty() {
        combined.extend_from_slice(&scene.entities);
        combined.extend_from_slice(spawned);
        combined_hierarchy = Hierarchy::new(&combined).ok();
    }
    let (objects, hierarchy) = combined_hierarchy
        .as_ref()
        .map_or((&scene.entities, hierarchy), |hierarchy| {
            (&combined, hierarchy)
        });
    let local: Vec<_> = objects
        .iter()
        .map(|object| {
            preview
                .and_then(|preview| preview.get(&object.uuid).copied())
                .or_else(|| state.get(&object.uuid).map(|&(transform, _)| transform))
                .unwrap_or(object.transform)
        })
        .collect();
    let world = hierarchy.world_matrices(|index| local[index].matrix());
    let view = camera_index
        .and_then(|index| scene.camera_view(index, hierarchy, &world, &local))
        .unwrap_or_else(|| camera.view());
    let instances = objects
        .iter()
        .zip(&world)
        .filter(|(object, _)| object.has_renderer())
        .map(|(object, &matrix)| {
            let material = object
                .material
                .filter(|_| object.sprite.is_none())
                .and_then(|id| scene.materials.get(&id));
            let tint = state
                .get(&object.uuid)
                .map_or(object.color, |&(_, color)| color);
            DrawObject {
                matrix: object
                    .sprite
                    .map_or(matrix, |sprite| sprite.matrix(matrix, view)),
                color: material.map_or(tint, |material| {
                    std::array::from_fn(|i| tint[i] * material.color[i])
                }),
                selected: selected.contains(&object.uuid),
                mesh: object.mesh,
                primitive: object.primitive,
                texture: material.map_or_else(
                    || {
                        object
                            .sprite
                            .map(|sprite| sprite.texture)
                            .or(object.texture)
                    },
                    |material| material.texture,
                ),
                surface: material.map_or_else(Default::default, |material| material.surface),
                material_override: material.is_some(),
                sprite: object.sprite.is_some(),
                animation: animations
                    .get(&object.uuid)
                    .copied()
                    .or_else(|| object.animation.map(|animation| (animation, 0.0)))
                    .map(|(animation, seconds)| futty_assets::AnimationSample {
                        clip: animation.clip,
                        seconds,
                        looping: animation.looping,
                    }),
            }
        })
        .collect();
    let lights = objects
        .iter()
        .enumerate()
        .filter_map(|(index, object)| {
            let light = object.light?;
            let position = world[index].w_axis.truncate();
            let toward_light = if light.kind == LightKind::Directional {
                hierarchy.world_rotation(index, |i| local[i].rotation) * Vec3::Z
            } else {
                Vec3::ZERO
            };
            (position.is_finite() && toward_light.is_finite()).then_some(LightInstance {
                light,
                position,
                toward_light,
            })
        })
        .take(MAX_LIGHTS)
        .collect();
    SceneFrame {
        world,
        local,
        view,
        instances,
        lights,
    }
}
