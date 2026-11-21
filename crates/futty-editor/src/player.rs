use std::{
    collections::HashMap,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
};

use eframe::{egui, egui_wgpu::RenderState};
use futty_audio::Audio;
use futty_build::{FORMAT, Launch, SCENE};
pub(crate) use futty_build::{MANIFEST, export};
#[cfg(test)]
use futty_core::Asset;
use futty_core::{AssetKind, Camera};
pub(crate) use futty_render::draw_objects;
use futty_runtime::{Hierarchy, Runtime, Scene};
#[cfg(test)]
use uuid::Uuid;

use crate::{
    audio, hud_view,
    input::InputSettings,
    project::{self, Project, SceneList},
    range_view::RangeControls,
    viewport::Viewport,
};

pub(crate) struct Player {
    world: GameScene,
    project: Option<(PathBuf, SceneList)>,
    scene_error: Option<String>,
    range_controls: RangeControls,
    input_settings: InputSettings,
    controls_open: bool,
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
        let result = (|| {
            if project::is_project(path) {
                let project = Project::load(path)?;
                Self::from_project(
                    cc,
                    project.root.clone(),
                    project.list().clone(),
                    Camera::default(),
                )
            } else {
                Self::new(cc, path, Camera::default(), None)
            }
        })();
        result.map_err(|error| io::Error::other(error.to_string()).into())
    }

    fn open(cc: &eframe::CreationContext<'_>, root: &Path) -> Result<Self, Box<dyn Error>> {
        let launch: Launch = ron::from_str(&fs::read_to_string(root.join(MANIFEST))?)?;
        if !launch.camera.eye.is_finite()
            || !launch.camera.yaw.is_finite()
            || launch.camera.pitch.abs() > 1.58
            || !launch.camera.pitch.is_finite()
        {
            return Err("Invalid game manifest or camera".into());
        }
        match (launch.format, launch.project) {
            (FORMAT, None) => Self::new(cc, &root.join(SCENE), launch.camera, None),
            (2, Some(project)) => {
                Self::from_project(cc, root.canonicalize()?, project, launch.camera)
            }
            _ => Err("Unsupported game manifest".into()),
        }
    }

    fn from_project(
        cc: &eframe::CreationContext<'_>,
        root: PathBuf,
        list: SceneList,
        camera: Camera,
    ) -> Result<Self, Box<dyn Error>> {
        list.validate()?;
        let entry = list.resolve(&root, &list.start)?;
        Self::new(cc, &entry, camera, Some((root, list)))
    }

    fn new(
        cc: &eframe::CreationContext<'_>,
        entry: &Path,
        camera: Camera,
        project: Option<(PathBuf, SceneList)>,
    ) -> Result<Self, Box<dyn Error>> {
        let gpu = cc
            .wgpu_render_state
            .clone()
            .ok_or("A wgpu device is required")?;
        let world = GameScene::load(entry, &cc.egui_ctx, gpu, camera)?;
        cc.egui_ctx
            .send_viewport_cmd(egui::ViewportCommand::Title(world.scene.name.clone()));
        Ok(Self {
            world,
            project,
            scene_error: None,
            range_controls: RangeControls::default(),
            input_settings: InputSettings::load(cc.storage),
            controls_open: false,
        })
    }

    fn transition(&mut self, ctx: &egui::Context, id: &str) {
        let result = (|| {
            let (root, list) = self
                .project
                .as_ref()
                .ok_or("Open a Futty project to switch scenes")?;
            let path = list.resolve(root, id)?;
            GameScene::load(&path, ctx, self.world.viewport.gpu(), Camera::default())
        })();
        match result {
            Ok(world) => {
                self.range_controls.release(ctx);
                self.world.audio.stop();
                ctx.send_viewport_cmd(egui::ViewportCommand::Title(world.scene.name.clone()));
                self.world = world;
                self.scene_error = None;
            }
            Err(error) => {
                let message = format!("Scene {id:?}: {error}");
                self.world.runtime.log_scene_error(message.clone());
                self.scene_error = Some(message);
            }
        }
    }
}

pub(crate) struct GameScene {
    pub scene: Scene,
    hierarchy: Hierarchy,
    pub runtime: Runtime,
    pub audio: Audio,
    pub viewport: Viewport,
}

impl GameScene {
    pub fn load(
        entry: &Path,
        ctx: &egui::Context,
        gpu: RenderState,
        camera: Camera,
    ) -> Result<Self, Box<dyn Error>> {
        let scene = Scene::load(entry)?;
        let hierarchy = scene.hierarchy()?;
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
            if scene
                .hud
                .iter()
                .any(|widget| widget.image == Some(asset.id))
            {
                viewport.hud_image(ctx, asset.id, Some(&bytes));
            }
        }
        let mut runtime = Runtime::new(&scene)?;
        runtime.start_scripts(&scene, &scripts);
        Ok(Self {
            scene,
            hierarchy,
            runtime,
            audio,
            viewport,
        })
    }

    pub fn paint(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        controls: &mut RangeControls,
        settings: &InputSettings,
        interactive: bool,
    ) {
        let rect = response.rect;
        let index = self.scene.camera_index(None);
        if index.is_none() && interactive {
            self.viewport.camera = self.viewport.navigate(response, self.viewport.camera);
        }
        let frame = draw_objects(
            &self.scene,
            &self.hierarchy,
            Some(&mut self.runtime),
            self.viewport.camera,
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
        hud_view::paint(
            ui,
            rect,
            &self.scene,
            &mut self.viewport,
            None,
            Some(&mut self.runtime),
            interactive && controls.cursor_free(),
        );
        if let Some(status) = self.runtime.range_status() {
            let capture = interactive && !hud_view::over_button(ui.ctx(), rect, &self.scene.hud);
            controls.paint(
                ui,
                response,
                status,
                settings,
                self.scene.hud.is_empty(),
                capture,
            );
        }
    }
}

impl eframe::App for Player {
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.current_pass_index() == 0 {
            InputSettings::toggle_shortcut(&ctx, &mut self.controls_open);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let size = ui.available_size();
                if size.x < 2.0 || size.y < 2.0 {
                    return;
                }
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
                if ctx.current_pass_index() == 0 {
                    let is_range = self.world.runtime.range_status().is_some();
                    let game = (!self.controls_open).then_some(rect);
                    let hud_blocks_mouse = self.range_controls.cursor_free()
                        && hud_view::over_button(&ctx, rect, &self.world.scene.hud);
                    let (dt, keys) = self.range_controls.input(
                        &ctx,
                        game,
                        is_range,
                        &self.input_settings.with_profile(&self.world.scene.input),
                        hud_blocks_mouse,
                    );
                    self.world.runtime.game_input(keys);
                    self.world.runtime.tick(dt);
                    if let Some(id) = self.world.runtime.take_scene_request() {
                        self.transition(&ctx, &id);
                    }
                    self.world.audio.update();
                    audio::pump(&mut self.world.audio, &mut self.world.runtime);
                }
                let settings = self.input_settings.with_profile(&self.world.scene.input);
                self.world.paint(
                    ui,
                    &response,
                    &mut self.range_controls,
                    &settings,
                    !self.controls_open,
                );
                if let Some(error) = &self.scene_error {
                    ui.painter().text(
                        rect.left_bottom() + egui::vec2(12.0, -12.0),
                        egui::Align2::LEFT_BOTTOM,
                        error,
                        egui::FontId::proportional(14.0),
                        egui::Color32::LIGHT_RED,
                    );
                }
            });
        self.input_settings
            .window(&ctx, &mut self.controls_open, &self.world.scene.input);
        ctx.request_repaint();
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.input_settings.save(storage);
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futty_runtime::{Billboard, Object, Sprite};

    #[test]
    fn beacon_bot_export_keeps_its_rig_and_hud_can_switch_to_a_one_shot_clip()
    -> Result<(), Box<dyn Error>> {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/animation-playground/Beacon Bot.futty.scene.ron");
        let root = tempfile::tempdir()?;
        let executable = root.path().join("template.exe");
        fs::write(&executable, b"test template")?;
        let output = root.path().join("Beacon-Windows");
        export(&source, &output, Camera::default(), &executable)?;
        let entry = output.join(SCENE);
        let scene = Scene::load(&entry)?;
        let bot = &scene.entities[0];
        let asset = scene
            .assets
            .iter()
            .find(|asset| Some(asset.id) == bot.mesh)
            .unwrap();
        let model = futty_assets::model(&futty_assets::read_saved(&entry, asset)?)?;
        assert_eq!(model.rig.unwrap().clips().len(), 3);

        let mut runtime = Runtime::new(&scene)?;
        runtime.start_scripts(&scene, &HashMap::new());
        runtime.tick(1.0 / 60.0);
        let (_, (idle, time)) = runtime.animated_objects().next().unwrap();
        assert_eq!(idle, bot.animation.unwrap());
        assert!(time > 0.0);
        runtime.click_hud("signal");
        runtime.tick(1.0 / 60.0);
        let (_, (signal, time)) = runtime.animated_objects().next().unwrap();
        assert_eq!(signal.clip, 2);
        assert!(!signal.looping);
        assert_eq!(time, 0.0);
        runtime.tick(1.0 / 60.0);
        assert!(runtime.animated_objects().next().unwrap().1.1 > 0.0);
        assert_eq!(scene.entities[0].animation, Some(idle));
        Ok(())
    }
    use glam::{Vec2, Vec3};

    #[test]
    fn spawned_prefab_children_render_and_despawn_together() {
        use futty_runtime::{Body, GameInput, InputAction, Motion, Prefab};
        let mut scene = Scene::default();
        let asset = Uuid::new_v4();
        scene.assets.push(Asset {
            id: asset,
            name: "Spawner".into(),
            kind: AssetKind::Script,
        });
        scene.script_sources.insert(
            asset,
            r#"
            fn on_start(entity) { this.branch = spawn_prefab("Pair", 2, 0, 0); }
            fn on_update(entity, dt) { if input_pressed("Interact") { despawn(this.branch); } }
        "#
            .into(),
        );
        scene.entities[0].script = Some(asset);
        let mut root = Object::cube("Root".into(), Vec3::ZERO);
        root.spin = false;
        root.body = Some(Body {
            motion: Motion::Fixed,
            ..Default::default()
        });
        let mut child = Object::cube("Child".into(), Vec3::Y);
        child.parent = Some(root.uuid);
        scene.prefabs.insert(
            Uuid::new_v4(),
            Prefab {
                name: "Pair".into(),
                entities: vec![root, child],
                source: None,
            },
        );
        let mut runtime = Runtime::new(&scene).unwrap();
        runtime.start_scripts(&scene, &HashMap::new());
        runtime.tick(1.0 / 60.0);
        let hierarchy = scene.hierarchy().unwrap();
        let render = |runtime: &mut Runtime| {
            draw_objects(
                &scene,
                &hierarchy,
                Some(runtime),
                Camera::default(),
                None,
                &[],
                None,
            )
        };
        let spawned = render(&mut runtime);
        assert_eq!(spawned.instances.len(), 3);
        assert_eq!(spawned.world[2].w_axis.truncate(), Vec3::new(2.0, 1.0, 0.0));
        assert_eq!(runtime.physics_body_count(), 1);
        let mut input = GameInput::default();
        input.actions[InputAction::Interact as usize].pressed = true;
        runtime.game_input(input);
        runtime.tick(1.0 / 60.0);
        runtime.game_input(GameInput::default());
        runtime.tick(1.0 / 60.0);
        assert_eq!(render(&mut runtime).instances.len(), 1);
        assert_eq!(runtime.physics_body_count(), 0);
        assert_eq!(scene.entities.len(), 1);
    }

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
        sun.light = Some(Light::new(futty_core::LightKind::Directional));
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
        let hud_texture = Asset {
            id: Uuid::new_v4(),
            name: "HUD texture".into(),
            kind: AssetKind::Texture,
        };
        futty_assets::save(&source, &hud_texture, bytes)?;
        scene.assets.push(hud_texture.clone());
        let mut image = futty_runtime::HudWidget::new(futty_runtime::HudKind::Image);
        image.image = Some(hud_texture.id);
        scene.hud.push(image);
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
        assert_eq!(
            exported.assets,
            vec![asset.clone(), texture.clone(), hud_texture.clone()]
        );
        assert_eq!(exported.hud, scene.hud);
        assert_eq!(exported.materials.len(), 1);
        assert!(exported.prefabs.is_empty());
        assert!(
            exported
                .entities
                .iter()
                .all(|object| object.prefab.is_none())
        );
        assert_eq!(exported.entities.len(), scene.entities.len());
        assert_eq!(exported.materials[&material], scene.materials[&material]);
        assert_eq!(futty_assets::read_saved(&portable, &texture)?, bytes);
        assert_eq!(futty_assets::read_saved(&portable, &asset)?, bytes);
        assert_eq!(futty_assets::read_saved(&portable, &hud_texture)?, bytes);
        assert_eq!(fs::read(output.join("Game.exe"))?, b"test template");
        assert!(output.join(MANIFEST).is_file());
        assert!(export(&source, &output, Camera::default(), &executable).is_err());

        let prefab_model = scene.assets.last().unwrap().clone();
        let model_bytes = include_bytes!("../../../examples/asset-preview/Workshop Crate.glb");
        futty_assets::save(&source, &prefab_model, model_bytes)?;
        let script = Uuid::new_v4();
        scene.assets.push(Asset {
            id: script,
            name: "Spawner".into(),
            kind: AssetKind::Script,
        });
        scene.entities[0].script = Some(script);
        scene.script_sources.insert(
            script,
            r#"fn on_start(entity) { spawn_prefab("Template only", 2, 0, 0); }"#.into(),
        );
        scene.save(&source)?;
        let scripted_output = root.path().join("Scripted");
        export(&source, &scripted_output, Camera::default(), &executable)?;
        let portable = scripted_output.join(SCENE);
        let exported = Scene::load(&portable)?;
        assert_eq!(
            futty_assets::read_saved(&portable, &prefab_model)?,
            model_bytes
        );
        let mut runtime = Runtime::new(&exported).unwrap();
        runtime.start_scripts(&exported, &HashMap::new());
        runtime.tick(1.0 / 60.0);
        assert_eq!(runtime.spawned_objects()[0].mesh, Some(prefab_model.id));
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
#[test]
fn exported_project_launches_its_lobby_and_resolves_its_scripted_destination()
-> Result<(), Box<dyn Error>> {
    let example = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/pulse-range/Pulse Range.futty.project.ron");
    let root = tempfile::tempdir()?;
    let executable = root.path().join("template.exe");
    fs::write(&executable, b"test template")?;
    let output = root.path().join("Range-Windows");
    export(&example, &output, Camera::default(), &executable)?;
    let relocated = root.path().join("Moved");
    fs::rename(output, &relocated)?;
    let relocated = relocated.canonicalize()?;

    let launch: Launch = ron::from_str(&fs::read_to_string(relocated.join(MANIFEST))?)?;
    assert_eq!(launch.format, 2);
    let list = launch.project.unwrap();
    list.validate()?;
    let entry = list.resolve(&relocated, &list.start)?;
    let lobby = Scene::load(&entry)?;
    assert_eq!(lobby.name, "Range Lobby");
    let mut runtime = Runtime::new(&lobby)?;
    runtime.start_scripts(&lobby, &HashMap::new());
    runtime.tick(1.0 / 60.0);
    assert!(runtime.take_scene_request().is_none());
    runtime.click_hud("enter_range");
    runtime.tick(1.0 / 60.0);
    runtime.tick(1.0 / 60.0);
    let requested = runtime.take_scene_request().unwrap();
    assert!(runtime.take_scene_request().is_none());
    let path = list.resolve(&relocated, &requested)?;
    let range = Scene::load(&path)?;
    assert_eq!(range.name, "Pulse Range");
    assert!(range.training_range.is_some());
    for asset in &range.assets {
        assert!(!futty_assets::read_saved(&path, asset)?.is_empty());
    }
    Ok(())
}
