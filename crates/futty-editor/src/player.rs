use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs, io,
    path::Path,
};

use eframe::egui;
use futty_audio::Audio;
use futty_core::{
    Asset, AssetKind, Camera, CameraLens, CameraView, LightKind, Projection, Transform,
};
use futty_render::{DrawObject, LightInstance, MAX_LIGHTS, RenderScene};
use futty_runtime::{GameInput, Hierarchy, Runtime, Scene};
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{audio, range_view::RangeControls, viewport::Viewport};

pub(crate) const MANIFEST: &str = "game.futty.build.ron";
const SCENE: &str = "game.futty.scene.ron";
const FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
struct Launch {
    format: u32,
    camera: Camera,
}

pub(crate) fn export(
    source: &Path,
    destination: &Path,
    camera: Camera,
    executable: &Path,
) -> Result<(), Box<dyn Error>> {
    if destination.try_exists()? {
        return Err("Choose a new output folder. The destination already exists.".into());
    }
    let mut scene = Scene::load(source)?;
    scene.prefabs.clear();
    let referenced_materials: HashSet<_> = scene
        .entities
        .iter()
        .filter_map(|object| object.material)
        .collect();
    scene
        .materials
        .retain(|id, _| referenced_materials.contains(id));
    let mut used: HashSet<_> = scene
        .entities
        .iter()
        .flat_map(|object| {
            [
                object.mesh,
                object.texture,
                object.sprite.map(|sprite| sprite.texture),
                object.script,
                object.audio_source.map(|audio| audio.clip),
            ]
            .into_iter()
            .flatten()
        })
        .collect();
    used.extend(
        scene
            .materials
            .values()
            .filter_map(|material| material.texture),
    );
    scene.assets.retain(|asset| used.contains(&asset.id));
    scene.script_sources.retain(|id, _| used.contains(id));
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stage = tempfile::Builder::new()
        .prefix(".futty-build-")
        .tempdir_in(parent)?;
    let entry = stage.path().join(SCENE);
    let mut audio = Audio::default();
    for asset in &scene.assets {
        if let Some(source_code) = scene.script_sources.get(&asset.id) {
            check_script(asset, source_code)?;
            continue;
        }
        let bytes = futty_assets::read_saved(source, asset)
            .map_err(|error| format!("{}: {error}", asset.name))?;
        validate_asset(asset, &bytes, &mut audio)
            .map_err(|error| format!("{}: {error}", asset.name))?;
        futty_assets::save(&entry, asset, &bytes)?;
    }
    scene.save(&entry)?;
    let launch = Launch {
        format: FORMAT,
        camera,
    };
    fs::write(stage.path().join(MANIFEST), ron::to_string(&launch)?)?;
    fs::copy(executable, stage.path().join("Game.exe"))?;
    fs::rename(stage.path(), destination)?;
    Ok(())
}

fn validate_asset(asset: &Asset, bytes: &[u8], audio: &mut Audio) -> Result<(), Box<dyn Error>> {
    match asset.kind {
        AssetKind::Model => {
            futty_assets::model(bytes)?;
        }
        AssetKind::Texture => {
            futty_assets::png(bytes)?;
        }
        AssetKind::Script => check_script(asset, futty_assets::script_source(bytes)?)?,
        AssetKind::Wav | AssetKind::Ogg => audio.load(asset.id, asset.kind.extension(), bytes)?,
    }
    Ok(())
}

fn check_script(asset: &Asset, source: &str) -> Result<(), Box<dyn Error>> {
    futty_runtime::inspect_script(source)
        .map(|_| ())
        .map_err(|error| format!("{}: {error}", asset.name).into())
}

pub(crate) fn game_input(ctx: &egui::Context, enabled: bool) -> (f32, GameInput) {
    ctx.input(|input| {
        let down = |key| {
            enabled
                && input.focused
                && !input.pointer.secondary_down()
                && !input.modifiers.command
                && input.key_down(key)
        };
        (
            input.stable_dt,
            GameInput {
                forward: down(egui::Key::W),
                backward: down(egui::Key::S),
                left: down(egui::Key::A),
                right: down(egui::Key::D),
                held: egui::Key::ALL
                    .iter()
                    .copied()
                    .filter(|&key| down(key))
                    .map(egui::Key::name)
                    .collect(),
                range: Default::default(),
            },
        )
    })
}

pub(crate) struct SceneFrame {
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

pub(crate) fn draw_objects(
    scene: &Scene,
    hierarchy: &Hierarchy,
    runtime: Option<&mut Runtime>,
    camera: Camera,
    camera_index: Option<usize>,
    selected: &[Uuid],
    preview: Option<&HashMap<Uuid, Transform>>,
) -> SceneFrame {
    let state: HashMap<_, _> = runtime.map_or_else(HashMap::new, |runtime| {
        runtime
            .extract()
            .map(|(id, transform, color)| (id, (transform, color)))
            .collect()
    });
    let local: Vec<_> = scene
        .entities
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
    let instances = scene
        .entities
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
            }
        })
        .collect();
    let lights = scene
        .entities
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

pub(crate) struct Player {
    scene: Scene,
    hierarchy: Hierarchy,
    runtime: Runtime,
    audio: Audio,
    viewport: Viewport,
    range_controls: RangeControls,
}

impl Player {
    pub(crate) fn load(
        cc: &eframe::CreationContext<'_>,
        root: &Path,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Self::open(cc, root).map_err(|error| io::Error::other(error.to_string()).into())
    }

    pub(crate) fn load_scene(
        cc: &eframe::CreationContext<'_>,
        path: &Path,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Self::open_scene(cc, path, Camera::default())
            .map_err(|error| io::Error::other(error.to_string()).into())
    }

    fn open(cc: &eframe::CreationContext<'_>, root: &Path) -> Result<Self, Box<dyn Error>> {
        let launch: Launch = ron::from_str(&fs::read_to_string(root.join(MANIFEST))?)?;
        if launch.format != FORMAT
            || !launch.camera.eye.is_finite()
            || !launch.camera.yaw.is_finite()
            || launch.camera.pitch.abs() > 1.58
            || !launch.camera.pitch.is_finite()
        {
            return Err("Invalid game manifest or camera".into());
        }
        Self::open_scene(cc, &root.join(SCENE), launch.camera)
    }

    fn open_scene(
        cc: &eframe::CreationContext<'_>,
        entry: &Path,
        camera: Camera,
    ) -> Result<Self, Box<dyn Error>> {
        let scene = Scene::load(entry)?;
        let hierarchy = scene.hierarchy()?;
        let gpu = cc
            .wgpu_render_state
            .clone()
            .ok_or("A wgpu device is required")?;
        let mut viewport = Viewport::new(gpu);
        viewport.camera = camera;
        let mut audio = Audio::default();
        let mut scripts = HashMap::new();
        for asset in &scene.assets {
            if scene.script_sources.contains_key(&asset.id) {
                continue;
            }
            let bytes = futty_assets::read_saved(entry, asset)
                .map_err(|error| format!("{}: {error}", asset.name))?;
            match asset.kind {
                AssetKind::Model | AssetKind::Texture => viewport.upload(asset, &bytes)?,
                AssetKind::Wav | AssetKind::Ogg => {
                    audio.load(asset.id, asset.kind.extension(), &bytes)?
                }
                AssetKind::Script => {
                    scripts.insert(asset.id, futty_assets::script_source(&bytes)?.to_owned());
                }
            }
        }
        let mut runtime = Runtime::new(&scene)?;
        runtime.start_scripts(&scene, &scripts);
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::Title(scene.name.clone()));
        Ok(Self {
            scene,
            hierarchy,
            runtime,
            audio,
            viewport,
            range_controls: RangeControls::default(),
        })
    }
}

impl eframe::App for Player {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let size = ui.available_size();
                if size.x < 2.0 || size.y < 2.0 {
                    return;
                }
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
                let index = self.scene.camera_index(None);
                if index.is_none() {
                    self.viewport.camera = self.viewport.navigate(&response, self.viewport.camera);
                }
                if ctx.current_pass_index() == 0 {
                    let is_range = self.runtime.range_status().is_some();
                    let range = self.range_controls.input(&ctx, is_range);
                    let (dt, mut keys) = game_input(&ctx, !is_range || range.active);
                    keys.range = range;
                    self.runtime.game_input(keys);
                    self.runtime.tick(dt);
                    self.audio.update();
                    audio::pump(&mut self.audio, &mut self.runtime);
                }
                let camera = self.viewport.camera;
                let frame = draw_objects(
                    &self.scene,
                    &self.hierarchy,
                    Some(&mut self.runtime),
                    camera,
                    index,
                    &[],
                    None,
                );
                self.viewport.render(
                    ui,
                    rect,
                    frame.render_scene(rect.aspect_ratio(), self.scene.sky),
                    true,
                );
                if let Some(status) = self.runtime.range_status() {
                    self.range_controls.paint(ui, &response, status);
                }
            });
        ctx.request_repaint();
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futty_runtime::{Billboard, Object, Sprite};
    use glam::{Vec2, Vec3};

    #[test]
    fn shared_material_edits_reach_every_mesh_in_edit_and_play() {
        let mut scene = Scene::default();
        scene.entities.push(Object::cube("Sphere".into(), Vec3::X));
        let id = Uuid::new_v4();
        scene
            .materials
            .insert(id, futty_runtime::Material::default());
        for object in &mut scene.entities {
            crate::material::assign(object, Some(id));
        }
        scene.entities[1].color = [0.5; 3];
        scene.materials.get_mut(&id).unwrap().color = [0.8, 0.4, 0.2];
        scene.materials.get_mut(&id).unwrap().surface.roughness = 0.1;
        let hierarchy = scene.hierarchy().unwrap();
        let mut runtime = Runtime::new(&scene).unwrap();
        for state in [None, Some(&mut runtime)] {
            let frame = draw_objects(
                &scene,
                &hierarchy,
                state,
                Camera::default(),
                None,
                &[],
                None,
            );
            assert_eq!(frame.instances[0].color, [0.8, 0.4, 0.2]);
            assert_eq!(frame.instances[1].color, [0.4, 0.2, 0.1]);
            assert!(
                frame
                    .instances
                    .iter()
                    .all(|object| object.material_override && object.surface.roughness == 0.1)
            );
        }
    }

    #[test]
    fn scene_lights_follow_their_parent_without_becoming_visible_meshes() {
        use futty_core::Light;
        use glam::Quat;

        let mut scene = Scene::default();
        scene.entities[0].transform.translation = Vec3::Y;
        scene.entities[0].transform.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let mut sun = Object::cube("Sun".into(), Vec3::Z);
        sun.parent = Some(scene.entities[0].uuid);
        sun.light = Some(Light::new(LightKind::Directional));
        scene.entities.push(sun);
        let frame = draw_objects(
            &scene,
            &scene.hierarchy().unwrap(),
            None,
            Camera::default(),
            None,
            &[],
            None,
        );
        assert_eq!(frame.instances.len(), 1);
        assert_eq!(frame.lights.len(), 1);
        assert!(
            frame.lights[0]
                .position
                .abs_diff_eq(Vec3::X + Vec3::Y, 1e-5)
        );
        assert!(frame.lights[0].toward_light.abs_diff_eq(Vec3::X, 1e-5));
    }

    #[test]
    fn game_view_uses_the_scripted_camera_and_excludes_its_editor_representation() {
        let mut scene = Scene::default();
        let script = Uuid::new_v4();
        let mut camera = Object::cube("Main Camera".into(), Vec3::Z * 8.0);
        camera.spin = false;
        camera.camera = Some(Default::default());
        camera.script = Some(script);
        scene.main_camera = Some(camera.uuid);
        scene.entities.push(camera);
        scene.assets.push(Asset {
            id: script,
            name: "Camera Move".into(),
            kind: AssetKind::Script,
        });
        scene.script_sources.insert(
            script,
            "fn on_start(entity) { translate(entity, 3.0, 0.0, 0.0); }".into(),
        );
        let mut runtime = Runtime::new(&scene).unwrap();
        runtime.start_scripts(&scene, &HashMap::new());
        runtime.tick(0.016);
        let frame = draw_objects(
            &scene,
            &scene.hierarchy().unwrap(),
            Some(&mut runtime),
            Camera::default(),
            scene.camera_index(None),
            &[],
            None,
        );
        assert_eq!(frame.view.eye, Vec3::new(3.0, 0.0, 8.0));
        assert_eq!(frame.instances.len(), 1);
    }

    #[test]
    fn export_relocates_the_scene_and_only_its_referenced_assets() -> Result<(), Box<dyn Error>> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source.ron");
        let output = root.path().join("Windows");
        let executable = root.path().join("template.exe");
        fs::write(&executable, b"test template")?;
        let mut scene = Scene::default();
        let asset = Asset {
            id: Uuid::new_v4(),
            name: "Signal".into(),
            kind: AssetKind::Texture,
        };
        let bytes = include_bytes!("../../../examples/sprite-playground/Signal.png");
        futty_assets::save(&source, &asset, bytes)?;
        scene.entities[0].sprite = Some(Sprite {
            texture: asset.id,
            size: Vec2::ONE,
            billboard: Billboard::Fixed,
        });
        scene.assets.push(asset.clone());
        let texture = Asset {
            id: Uuid::new_v4(),
            name: "Material Texture".into(),
            kind: AssetKind::Texture,
        };
        futty_assets::save(&source, &texture, bytes)?;
        scene.assets.push(texture.clone());
        let material = Uuid::new_v4();
        scene.materials.insert(
            material,
            futty_runtime::Material {
                texture: Some(texture.id),
                ..Default::default()
            },
        );
        scene
            .materials
            .insert(Uuid::new_v4(), futty_runtime::Material::default());
        let mut cube = Object::cube("Shared".into(), Vec3::X);
        cube.material = Some(material);
        scene.entities.push(cube);
        scene.assets.push(Asset {
            id: Uuid::new_v4(),
            name: "Unused".into(),
            kind: AssetKind::Model,
        });
        let placed = scene.create_prefab(scene.entities[1].uuid)?;
        scene.instantiate_prefab(placed, Vec3::X * 4.0)?;
        let mut template_only = Object::cube("Template only".into(), Vec3::ZERO);
        template_only.mesh = Some(scene.assets.last().unwrap().id);
        template_only.material = scene.materials.keys().copied().find(|id| *id != material);
        let template_root = template_only.uuid;
        scene.entities.push(template_only);
        scene.create_prefab(template_root)?;
        scene.entities.pop();
        scene.save(&source)?;
        export(&source, &output, Camera::default(), &executable)?;
        let portable = output.join(SCENE);
        let exported = Scene::load(&portable)?;
        assert_eq!(exported.sky, scene.sky);
        assert_eq!(exported.assets, vec![asset.clone(), texture.clone()]);
        assert_eq!(exported.materials.len(), 1);
        assert!(exported.prefabs.is_empty());
        assert_eq!(exported.entities.len(), scene.entities.len());
        assert_eq!(exported.materials[&material], scene.materials[&material]);
        assert_eq!(futty_assets::read_saved(&portable, &texture)?, bytes);
        assert_eq!(futty_assets::read_saved(&portable, &asset)?, bytes);
        assert_eq!(fs::read(output.join("Game.exe"))?, b"test template");
        assert!(output.join(MANIFEST).is_file());
        assert!(export(&source, &output, Camera::default(), &executable).is_err());
        Ok(())
    }

    #[test]
    fn invalid_script_does_not_publish_a_partial_build() -> Result<(), Box<dyn Error>> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source.ron");
        let output = root.path().join("Windows");
        let mut scene = Scene::default();
        let script = Uuid::new_v4();
        scene.assets.push(Asset {
            id: script,
            name: "Broken".into(),
            kind: AssetKind::Script,
        });
        scene.entities[0].script = Some(script);
        scene.script_sources.insert(script, "fn on_update(".into());
        scene.save(&source)?;
        let failure =
            export(&source, &output, Camera::default(), Path::new("unused.exe")).unwrap_err();
        assert!(failure.to_string().contains("Broken"));
        assert!(!output.exists());
        Ok(())
    }
}
