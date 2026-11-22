use std::{fs, path::Path};

use futty_core::{Light, LightKind, Primitive};
use futty_runtime::{Body, HudAnchor, HudKind, HudWidget, Motion, Object, Scene, project::Project};
use glam::{Quat, Vec2, Vec3};
use rmcp::{ErrorData, schemars};
use serde::Deserialize;

use crate::{authoring::invalid, camera_object, io_error, scripts};

pub(crate) const COIN_COLLECTION: &str = "coin_collection";
pub(crate) const PROJECT_FILE: &str = "Coin Collection.futty.project.ron";
pub(crate) const SCENE_FILE: &str = "Coin Field.futty.scene.ron";

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateGame {
    /// Template ID from project_template_list.
    pub template: String,
    /// New workspace-relative project directory. Its parent must exist.
    pub output_path: String,
}

pub(crate) fn create(template: &str, output: &Path) -> Result<(), ErrorData> {
    if template != COIN_COLLECTION {
        return Err(invalid(format!("Unknown project template {template:?}")));
    }
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
    let scene_path = stage.path().join(SCENE_FILE);
    coin_scene()?.save_new(&scene_path).map_err(io_error)?;
    Project::create(&stage.path().join(PROJECT_FILE), &scene_path).map_err(io_error)?;
    if output.symlink_metadata().is_ok() {
        return Err(invalid("Choose a new project folder"));
    }
    fs::rename(stage.path(), output).map_err(io_error)
}

fn coin_scene() -> Result<Scene, ErrorData> {
    let mut scene = Scene::default();
    scene.name = "Coin Field".into();
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

    let score = include_str!("../templates/coin-score.rhai");
    let pickup = include_str!("../templates/coin-pickup.rhai");
    scripts::check(score, SCENE_FILE, None)?;
    scripts::check(pickup, SCENE_FILE, None)?;
    let score = scripts::register(&mut scene, "Coin score".into(), score.into());
    let pickup = scripts::register(&mut scene, "Coin pickup".into(), pickup.into());
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

    let mut objective = HudWidget::new(HudKind::Text);
    objective.label.clear();
    objective.binding = "objective".into();
    objective.size = Vec2::new(305.0, 48.0);
    let mut coins = HudWidget::new(HudKind::Text);
    coins.label = "Coins / 3".into();
    coins.binding = "coins".into();
    coins.anchor = HudAnchor::TopRight;
    coins.color = [255, 210, 85];
    scene.hud.extend([objective, coins]);
    Ok(scene)
}
