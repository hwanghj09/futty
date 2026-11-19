use std::{error::Error, path::Path};

use futty_core::{Asset, AssetKind, Light, LightKind, Primitive, Surface};
use futty_runtime::{
    AudioSource, AudioStart, Body, CharacterController, HudAnchor, HudKind, HudWidget, Material,
    Motion, Object, RangeConfig, Scene,
};
use glam::{Quat, Vec2, Vec3};
use uuid::Uuid;

fn object(name: &str, position: Vec3) -> Object {
    let mut result = Object::cube(name.into(), position);
    result.spin = false;
    result.color = [1.0; 3];
    result
}

fn add_range_colliders(scene: &mut Scene) {
    for (name, center, size, color) in [
        (
            "Deck collider",
            [0.0, -0.26, -1.75],
            [15.6, 0.5, 26.0],
            [0.04, 0.06, 0.08],
        ),
        (
            "Left boundary",
            [-7.67, 1.30, -1.9],
            [0.40, 2.60, 25.9],
            [0.08, 0.12, 0.15],
        ),
        (
            "Right boundary",
            [7.67, 1.30, -1.9],
            [0.40, 2.60, 25.9],
            [0.08, 0.12, 0.15],
        ),
        (
            "Backstop collider",
            [0.0, 2.38, -14.55],
            [15.6, 4.75, 0.45],
            [0.08, 0.12, 0.15],
        ),
        (
            "Rear railing collider",
            [0.0, 0.65, 11.10],
            [15.5, 1.29, 0.29],
            [0.08, 0.12, 0.15],
        ),
        (
            "Left practice cover",
            [-3.35, 1.01, 0.10],
            [1.65, 2.02, 0.70],
            [0.11, 0.19, 0.22],
        ),
        (
            "Right practice cover",
            [3.35, 1.01, -0.60],
            [1.65, 2.02, 0.70],
            [0.11, 0.19, 0.22],
        ),
    ] {
        let mut obstacle = object(name, Vec3::from(center));
        obstacle.transform.scale = Vec3::from(size);
        obstacle.color = color;
        obstacle.body = Some(Body {
            motion: Motion::Fixed,
            ..Default::default()
        });
        scene.entities.push(obstacle);
    }
}

fn add_range_hud(scene: &mut Scene) {
    use HudAnchor::{BottomRight, TopLeft, TopRight};
    use HudKind::{Gauge, Text};

    for (kind, anchor, label, binding, offset, size) in [
        (
            Text,
            TopLeft,
            "SCORE",
            "range.score",
            [16.0, 16.0],
            [132.0, 54.0],
        ),
        (
            Text,
            TopRight,
            "TIME LEFT",
            "range.time",
            [16.0, 16.0],
            [132.0, 54.0],
        ),
        (
            Gauge,
            TopRight,
            "",
            "range.time_fraction",
            [16.0, 76.0],
            [132.0, 6.0],
        ),
        (
            Text,
            BottomRight,
            "MAGAZINE",
            "range.ammo",
            [16.0, 16.0],
            [154.0, 54.0],
        ),
        (
            Text,
            TopLeft,
            "",
            "range.streak",
            [16.0, 76.0],
            [180.0, 30.0],
        ),
    ] {
        let mut widget = HudWidget::new(kind);
        widget.anchor = anchor;
        widget.label = label.into();
        widget.binding = binding.into();
        widget.offset = Vec2::from_array(offset);
        widget.size = Vec2::from_array(size);
        if binding == "range.streak" {
            widget.text_size = 13.0;
        }
        scene.hud.push(widget);
    }
}

fn asset(
    scene: &mut Scene,
    scene_file: &Path,
    source: &Path,
    kind: AssetKind,
) -> Result<Uuid, Box<dyn Error>> {
    let asset = Asset {
        id: Uuid::new_v4(),
        name: source
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or("Missing asset file name")?
            .into(),
        kind,
    };
    let bytes = futty_assets::read(source)?;
    if kind == AssetKind::Model {
        futty_assets::model(&bytes)?;
    } else {
        futty_audio::Audio::default().load(asset.id, kind.extension(), &bytes)?;
    }
    futty_assets::save(scene_file, &asset, &bytes)?;
    let id = asset.id;
    scene.assets.push(asset);
    Ok(id)
}

fn sound(clip: Uuid, volume: f32) -> AudioSource {
    AudioSource {
        clip,
        volume,
        start: AudioStart::Manual,
        ..AudioSource::new(clip)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let scene_path = args
        .next()
        .ok_or("Usage: build_pulse_range <MCP scene file> <model directory>")?;
    let source = args.next().ok_or("Missing model directory")?;
    let scene_path = Path::new(&scene_path);
    let source = Path::new(&source);
    let mut scene = Scene::load(scene_path)?;
    let camera_index = scene
        .camera_index(None)
        .ok_or("Create the FPS camera with Futty MCP first")?;
    if scene.training_range.is_some() || scene.entities.len() != 2 || !scene.assets.is_empty() {
        return Err(
            "Expected the initial MCP-created scene with its default cube and FPS camera".into(),
        );
    }
    let mut camera = scene.entities[camera_index].clone();
    camera.transform.translation = Vec3::new(0.0, 1.65, 8.7);
    camera.transform.rotation = Quat::IDENTITY;
    camera.character = Some(CharacterController::default());
    let lens = camera.camera.as_mut().unwrap();
    lens.fov = 68.0;
    lens.far = 130.0;
    let camera_id = camera.uuid;
    scene.main_camera = Some(camera_id);
    scene.entities = vec![camera];

    let courtyard_mesh = asset(
        &mut scene,
        scene_path,
        &source.join("Courtyard.glb"),
        AssetKind::Model,
    )?;
    let drone_mesh = asset(
        &mut scene,
        scene_path,
        &source.join("Drone.glb"),
        AssetKind::Model,
    )?;
    let blaster_mesh = asset(
        &mut scene,
        scene_path,
        &source.join("Blaster.glb"),
        AssetKind::Model,
    )?;
    let pulse_sound = asset(
        &mut scene,
        scene_path,
        &source.join("Pulse.wav"),
        AssetKind::Wav,
    )?;
    let confirm_sound = asset(
        &mut scene,
        scene_path,
        &source.join("Confirm.wav"),
        AssetKind::Wav,
    )?;

    let mut courtyard = object("Training courtyard", Vec3::ZERO);
    courtyard.mesh = Some(courtyard_mesh);
    scene.entities.push(courtyard);

    let mut sun = object("Sun", Vec3::new(-5.0, 9.0, 6.0));
    sun.transform.rotation =
        Quat::from_rotation_arc(Vec3::Z, Vec3::new(-0.46, 0.82, 0.48).normalize());
    sun.light = Some(Light {
        color: [1.0, 0.91, 0.77],
        intensity: 0.9,
        ..Light::new(LightKind::Directional)
    });
    scene.entities.push(sun);
    let mut fill = object("Sky fill", Vec3::new(3.0, 7.0, -8.0));
    fill.transform.rotation =
        Quat::from_rotation_arc(Vec3::Z, Vec3::new(0.43, 0.66, -0.61).normalize());
    fill.light = Some(Light {
        color: [0.48, 0.73, 1.0],
        intensity: 0.72,
        ..Light::new(LightKind::Directional)
    });
    scene.entities.push(fill);

    let mut weapon = object("Pulse blaster", Vec3::new(0.29, -0.255, -0.63));
    weapon.transform.rotation = Quat::from_rotation_y(0.16);
    weapon.parent = Some(camera_id);
    weapon.mesh = Some(blaster_mesh);
    weapon.audio_source = Some(sound(pulse_sound, 0.72));
    let weapon_id = weapon.uuid;
    scene.entities.push(weapon);

    let flash_material = Uuid::new_v4();
    scene.materials.insert(
        flash_material,
        Material {
            name: "Pulse emission".into(),
            color: [0.16, 0.88, 0.76],
            surface: Surface {
                emission: [0.24, 0.95, 0.79],
                emission_strength: 1.3,
                ..Surface::default()
            },
            ..Material::default()
        },
    );
    let mut flash = object("Muzzle pulse", Vec3::new(0.0, 0.052, -0.49));
    flash.parent = Some(weapon_id);
    flash.primitive = Primitive::Sphere;
    flash.transform.scale = Vec3::new(0.19, 0.19, 0.31);
    flash.material = Some(flash_material);
    let flash_id = flash.uuid;
    scene.entities.push(flash);

    let mut drones = Vec::new();
    for (index, position) in [
        [-4.35, 1.72, -6.4],
        [-2.39, 2.32, -9.0],
        [0.0, 1.65, -3.2],
        [2.35, 2.02, -7.7],
        [4.28, 1.40, -5.9],
    ]
    .into_iter()
    .enumerate()
    {
        let mut drone = object(
            &format!("Target drone {:02}", index + 1),
            Vec3::from(position),
        );
        drone.mesh = Some(drone_mesh);
        drone.audio_source = Some(sound(confirm_sound, 0.7));
        drones.push(drone.uuid);
        scene.entities.push(drone);
    }
    scene.training_range = Some(RangeConfig {
        weapon: weapon_id,
        flash: flash_id,
        drones,
    });
    add_range_colliders(&mut scene);
    add_range_hud(&mut scene);
    scene.save(scene_path)?;
    println!("Assembled {}", scene_path.display());
    Ok(())
}
