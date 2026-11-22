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
    let (state, animations, sprite_times, inactive, spawned) = runtime.map_or_else(
        || {
            (
                HashMap::new(),
                HashMap::new(),
                HashMap::new(),
                Default::default(),
                &[][..],
            )
        },
        |runtime| {
            let state = runtime
                .extract()
                .map(|(id, transform, color)| (id, (transform, color)))
                .collect();
            let animations = runtime.animated_objects().collect::<HashMap<_, _>>();
            let sprite_times = runtime.animated_sprites().collect::<HashMap<_, _>>();
            (
                state,
                animations,
                sprite_times,
                runtime.inactive_objects().clone(),
                runtime.spawned_objects(),
            )
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
        .filter(|&index| !inactive.contains(&scene.entities[index].uuid))
        .and_then(|index| scene.camera_view(index, hierarchy, &world, &local))
        .unwrap_or_else(|| camera.view());
    let instances = objects
        .iter()
        .zip(&world)
        .filter(|(object, _)| object.has_renderer() && !inactive.contains(&object.uuid))
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
                sprite: object.sprite.map(|sprite| {
                    sprite.sheet.map_or([0.0, 0.0, 1.0, 1.0], |sheet| {
                        sheet.uv_rect(sprite_times.get(&object.uuid).copied().unwrap_or(0.0))
                    })
                }),
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
            if inactive.contains(&object.uuid) {
                return None;
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use futty_core::{Asset, AssetKind, Light};
    use futty_runtime::Object;

    #[test]
    fn orthographic_frame_follows_script_movement_and_advances_sprite_cells() {
        let mut scene = Scene::default();
        let actor = &mut scene.entities[0];
        let texture = Uuid::new_v4();
        let script = Uuid::new_v4();
        actor.tags.push("hero".into());
        actor.script = Some(script);
        actor.sprite = Some(futty_runtime::Sprite {
            texture,
            size: glam::Vec2::ONE,
            billboard: futty_runtime::Billboard::Fixed,
            sheet: Some(futty_runtime::SpriteSheet {
                columns: 2,
                rows: 1,
                first_frame: 0,
                frames: 2,
                fps: 10.0,
                looping: true,
            }),
        });
        scene.assets.extend([
            Asset {
                id: texture,
                name: "Sheet".into(),
                kind: AssetKind::Texture,
            },
            Asset {
                id: script,
                name: "Move".into(),
                kind: AssetKind::Script,
            },
        ]);
        scene.script_sources.insert(
            script,
            "fn on_start(entity) { set_position(entity, 6.0, 7.0, 0.0); }".into(),
        );
        let camera_index = scene.entities.len();
        let mut camera = Object::cube("View".into(), Vec3::new(0.0, 2.0, 10.0));
        camera.camera = Some(CameraLens {
            projection: Projection::Orthographic,
            ..CameraLens::default()
        });
        camera.camera_follow = Some(futty_runtime::CameraFollow2d {
            tag: "hero".into(),
            offset: [1.0, 0.0],
            axes: [true, false],
            bounds: Some([[-4.0, -4.0], [5.0, 4.0]]),
        });
        scene.entities.push(camera);
        let hierarchy = scene.hierarchy().unwrap();
        let mut runtime = Runtime::new(&scene).unwrap();
        runtime.start_scripts(&scene, &HashMap::new());
        runtime.prepare_play();
        let frame = |runtime| {
            draw_objects(
                &scene,
                &hierarchy,
                runtime,
                Camera::default(),
                Some(camera_index),
                &[],
                None,
            )
        };
        assert_eq!(frame(None).instances[0].sprite, Some([0.0, 0.0, 0.5, 1.0]));
        runtime.tick(0.1);
        let played = frame(Some(&mut runtime));
        assert_eq!(played.instances[0].sprite, Some([0.5, 0.0, 0.5, 1.0]));
        assert_eq!(played.view.eye, Vec3::new(5.0, 2.0, 10.0));
    }

    #[test]
    fn runtime_frame_omits_inactive_meshes_and_lights() {
        let mut scene = Scene::default();
        let mut light = Object::cube("Lamp".into(), Vec3::Y);
        light.light = Some(Light::new(LightKind::Point));
        light.tags.push("lamp".into());
        let id = Uuid::new_v4();
        scene.entities[0].script = Some(id);
        scene.entities.push(light);
        scene.assets.push(Asset {
            id,
            name: "Hide".into(),
            kind: AssetKind::Script,
        });
        scene.script_sources.insert(
            id,
            r#"fn on_start(entity) {
            set_active(entity, false);
            set_active(find_tag("lamp"), false);
        }"#
            .into(),
        );
        let hierarchy = scene.hierarchy().unwrap();
        let mut runtime = Runtime::new(&scene).unwrap();
        runtime.start_scripts(&scene, &HashMap::new());
        runtime.prepare_play();
        let frame = draw_objects(
            &scene,
            &hierarchy,
            Some(&mut runtime),
            Camera::default(),
            None,
            &[],
            None,
        );
        assert!(frame.instances.is_empty());
        assert!(frame.lights.is_empty());
    }
}
