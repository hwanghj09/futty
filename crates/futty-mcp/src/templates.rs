use std::{fs, path::Path};

use futty_core::{Asset, AssetKind, Light, LightKind, Primitive};
use futty_runtime::{
    Billboard, Body, CameraFollow2d, CharacterController, CharacterMode, HudAnchor, HudKind,
    HudWidget, Motion, Object, Scene, SceneDimension, ScriptValue, Sprite, Sprite2d, SpriteSheet,
    project::Project,
};
use glam::{Quat, Vec2, Vec3};
use rmcp::{ErrorData, schemars};
use serde::Deserialize;
use uuid::Uuid;

use crate::{authoring::invalid, camera_object, io_error, scripts};

pub(crate) const COIN_COLLECTION: &str = "coin_collection";
pub(crate) const COIN_RUNNER: &str = "coin_runner_2d";
pub(crate) const POWER_MAZE: &str = "power_maze_2d";

pub(crate) struct Template {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    scene_name: &'static str,
    pub steps: &'static [(&'static [&'static str], u32)],
    pub hud_expectation: (&'static str, u32),
    build_scene: fn(&Path) -> Result<Scene, ErrorData>,
}

pub(crate) const TEMPLATES: &[Template] = &[
    Template {
        id: COIN_COLLECTION,
        name: "Coin Collection",
        description: "First-person collection game with three coins, a score HUD, and a success condition.",
        scene_name: "Coin Field",
        steps: &[(&["forward"], 120)],
        hud_expectation: ("coins", 3),
        build_scene: coin_scene,
    },
    Template {
        id: COIN_RUNNER,
        name: "2D Coin Runner",
        description: "A side-scroller with animated sprites, colliding tiles, a jump over a gap, and three coins.",
        scene_name: "Coin Runner",
        steps: &[
            (&[], 10),
            (&["right"], 52),
            (&["right", "jump"], 54),
            (&["right"], 49),
        ],
        hud_expectation: ("coins", 3),
        build_scene: runner_scene,
    },
    Template {
        id: POWER_MAZE,
        name: "2D Power Maze",
        description: "A top-down maze with an animated explorer, a power cell, and a locked exit.",
        scene_name: "Power Maze",
        steps: &[
            (&["forward"], 82),
            (&["right"], 82),
            (&["backward"], 82),
            (&["forward"], 82),
        ],
        hud_expectation: ("power", 1),
        build_scene: power_maze_scene,
    },
];

impl Template {
    pub fn project_file(&self) -> String {
        format!("{}.futty.project.ron", self.name)
    }

    pub fn scene_file(&self) -> String {
        format!("{}.futty.scene.ron", self.scene_name)
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateGame {
    /// Template ID from project_template_list.
    pub template: String,
    /// New workspace-relative project directory. Its parent must exist.
    pub output_path: String,
}

pub(crate) fn create(id: &str, output: &Path) -> Result<&'static Template, ErrorData> {
    let template = TEMPLATES
        .iter()
        .find(|template| template.id == id)
        .ok_or_else(|| invalid(format!("Unknown project template {id:?}")))?;
    if output.symlink_metadata().is_ok() {
        return Err(invalid("Choose a new project folder"));
    }
    let parent = output
        .parent()
        .ok_or_else(|| invalid("Project output requires a parent folder"))?;
    let stage = tempfile::Builder::new()
        .prefix(".futty-template-")
        .tempdir_in(parent)
        .map_err(io_error)?;
    let scene_path = stage.path().join(template.scene_file());
    let mut scene = (template.build_scene)(&scene_path)?;
    scene.name = template.scene_name.into();
    scene.save_new(&scene_path).map_err(io_error)?;
    Project::create(&stage.path().join(template.project_file()), &scene_path).map_err(io_error)?;
    if output.symlink_metadata().is_ok() {
        return Err(invalid("Choose a new project folder"));
    }
    fs::rename(stage.path(), output).map_err(io_error)?;
    Ok(template)
}

fn coin_scene(path: &Path) -> Result<Scene, ErrorData> {
    let mut scene = Scene::default();
    scene.entities.clear();
    let mut floor = Object::cube("Platform".into(), Vec3::new(0.0, -0.25, 0.0));
    floor.spin = false;
    floor.transform.scale = Vec3::new(9.0, 0.5, 14.0);
    floor.color = [0.16, 0.28, 0.36];
    floor.body = Some(Body {
        motion: Motion::Fixed,
        ..Body::default()
    });
    scene.entities.push(floor);
    let mut sun = Object::cube("Sun".into(), Vec3::new(0.0, 4.0, 0.0));
    sun.spin = false;
    sun.transform.rotation = Quat::from_rotation_x(-0.85);
    sun.light = Some(Light::new(LightKind::Directional));
    scene.entities.push(sun);

    let [score, pickup] = coin_rules(&mut scene, path)?;
    let mut camera = camera_object("Collector".into(), [0.0, 1.65, 4.5], [0.0, 1.35, 0.0])?;
    camera.character = Some(Default::default());
    scripts::assign(&mut camera, Some(score));
    scene.main_camera = Some(camera.uuid);
    scene.entities.push(camera);
    for (index, z) in [2.0, 0.0, -2.0].into_iter().enumerate() {
        let mut coin = Object::cube(format!("Coin {}", index + 1), Vec3::new(0.0, 1.05, z));
        coin.primitive = Primitive::Cylinder;
        coin.tags.push("coin".into());
        coin.color = [1.0, 0.74, 0.09];
        coin.spin = false;
        coin.transform.scale = Vec3::new(0.52, 0.11, 0.52);
        coin.transform.rotation = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        scripts::assign(&mut coin, Some(pickup));
        scene.entities.push(coin);
    }

    add_objective_hud(&mut scene, "Coins / 3", "coins");
    Ok(scene)
}

fn coin_rules(scene: &mut Scene, path: &Path) -> Result<[Uuid; 2], ErrorData> {
    let score = include_str!("../templates/coin-score.rhai");
    let pickup = include_str!("../templates/coin-pickup.rhai");
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Coin template");
    scripts::check(score, name, None)?;
    scripts::check(pickup, name, None)?;
    Ok([
        scripts::register(scene, "Coin score".into(), score.into()),
        scripts::register(scene, "Coin pickup".into(), pickup.into()),
    ])
}

fn add_objective_hud(scene: &mut Scene, label: &str, binding: &str) {
    let mut objective = HudWidget::new(HudKind::Text);
    objective.label.clear();
    objective.binding = "objective".into();
    objective.size = Vec2::new(305.0, 48.0);
    let mut progress = HudWidget::new(HudKind::Text);
    progress.label = label.into();
    progress.binding = binding.into();
    progress.anchor = HudAnchor::TopRight;
    progress.color = [255, 210, 85];
    scene.hud.extend([objective, progress]);
}

fn runner_scene(path: &Path) -> Result<Scene, ErrorData> {
    let mut scene = Scene::new(SceneDimension::TwoD);
    let camera = &mut scene.entities[0];
    camera.transform.translation.y = 1.2;
    camera.camera.as_mut().unwrap().size = 3.0;
    camera.camera_follow = Some(CameraFollow2d {
        tag: "runner".into(),
        offset: [1.0, 0.0],
        axes: [true, false],
        bounds: Some([[0.0, 1.2], [8.0, 1.2]]),
    });

    let ground = template_texture(
        &mut scene,
        path,
        "Tiles",
        include_bytes!("../templates/coin-runner/tiles.png"),
    )?;
    let floor = crate::layout::Tilemap2d {
        texture_id: ground,
        atlas: [2, 1],
        rows: vec![
            (-3..=12)
                .map(|x| (!(3..=4).contains(&x)).then_some(0))
                .collect(),
        ],
        solid: vec![0],
        origin: Some([-3.0, -0.5, 0.0]),
        tile_size: None,
    }
    .build(&scene)?;
    scene.entities.extend(floor);
    let hero = template_texture(
        &mut scene,
        path,
        "Runner",
        include_bytes!("../templates/coin-runner/hero_walk.png"),
    )?;
    let coin = template_texture(
        &mut scene,
        path,
        "Coin",
        include_bytes!("../templates/coin-runner/coin.png"),
    )?;
    let [score, pickup] = coin_rules(&mut scene, path)?;

    let mut player = Object::cube("Runner".into(), Vec3::new(-2.3, 0.43, 0.0));
    player.tags.push("runner".into());
    player.character = Some(CharacterController {
        mode: CharacterMode::SideScroller,
        radius: 0.24,
        height: 0.8,
        origin_height: 0.4,
        coyote_time: 0.10,
        jump_buffer: 0.12,
        ..Default::default()
    });
    template_sprite(&mut player, hero, 1.25, 1);
    player.sprite.as_mut().unwrap().sheet = Some(SpriteSheet {
        columns: 4,
        rows: 1,
        first_frame: 0,
        frames: 4,
        fps: 8.0,
        looping: true,
    });
    scripts::assign(&mut player, Some(score));
    player
        .script_fields
        .insert("fall_loss".into(), ScriptValue::Bool(true));
    scene.entities.push(player);
    for (index, x) in [-0.9, 5.6, 8.8].into_iter().enumerate() {
        let mut object = Object::cube(format!("Coin {}", index + 1), Vec3::new(x, 0.55, 0.0));
        object.tags.push("coin".into());
        template_sprite(&mut object, coin, 0.62, 2);
        scripts::assign(&mut object, Some(pickup));
        scene.entities.push(object);
    }
    add_objective_hud(&mut scene, "Coins / 3", "coins");
    Ok(scene)
}

fn power_maze_scene(path: &Path) -> Result<Scene, ErrorData> {
    let mut scene = Scene::new(SceneDimension::TwoD);
    let camera = &mut scene.entities[0];
    camera.transform.translation = Vec3::new(3.0, 3.0, 10.0);
    camera.camera.as_mut().unwrap().size = 8.0;
    let (mut player, mut exit, walls) = crate::layout::Maze2d {
        seed: 0,
        rooms: [2, 2],
        cell_size: None,
        origin: None,
    }
    .build()?;
    scene.entities.extend(walls);

    let hero = template_texture(
        &mut scene,
        path,
        "Explorer",
        include_bytes!("../templates/coin-runner/hero_walk.png"),
    )?;
    let power_texture = template_texture(
        &mut scene,
        path,
        "Power cell",
        include_bytes!("../templates/coin-runner/coin.png"),
    )?;
    player.name = "Explorer".into();
    player.transform.scale = Vec3::ONE;
    template_sprite(&mut player, hero, 1.5, 1);
    player.sprite.as_mut().unwrap().sheet = Some(SpriteSheet {
        columns: 4,
        rows: 1,
        first_frame: 0,
        frames: 4,
        fps: 8.0,
        looping: true,
    });
    let source = include_str!("../templates/power-maze.rhai");
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Power Maze");
    scripts::check(source, name, None)?;
    let rules = scripts::register(&mut scene, "Power Maze rules".into(), source.into());
    scripts::assign(&mut player, Some(rules));

    exit.name = "Exit".into();
    exit.transform.scale = Vec3::new(1.2, 1.2, 0.25);
    let mut cell = Object::cube("Power cell".into(), Vec3::new(6.0, 0.0, 0.0));
    cell.tags.push("power_cell".into());
    template_sprite(&mut cell, power_texture, 1.15, 2);
    scene.entities.extend([player, exit, cell]);
    add_objective_hud(&mut scene, "Power / 1", "power");
    Ok(scene)
}

fn template_texture(
    scene: &mut Scene,
    path: &Path,
    name: &str,
    bytes: &[u8],
) -> Result<Uuid, ErrorData> {
    let asset = Asset {
        id: Uuid::new_v4(),
        name: name.into(),
        kind: AssetKind::Texture,
    };
    crate::assets::validate_asset(scene, &asset, bytes)?;
    futty_assets::save_new(path, &asset, bytes).map_err(io_error)?;
    let id = asset.id;
    scene.assets.push(asset);
    Ok(id)
}

fn template_sprite(object: &mut Object, texture: Uuid, size: f32, layer: i16) {
    object.spin = false;
    object.color = [1.0; 3];
    object.sprite = Some(Sprite {
        texture,
        size: Vec2::splat(size),
        billboard: Billboard::Fixed,
        sheet: None,
        planar: Some(Sprite2d {
            layer,
            ..Sprite2d::default()
        }),
    });
}
