use std::{
    collections::HashSet,
    error::Error,
    fs,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use futty_audio::Audio;
use futty_core::{Asset, AssetKind, Camera};
use futty_runtime::{
    Scene,
    project::{Project, SceneList, is_project},
};
use serde::{Deserialize, Serialize};

pub const MANIFEST: &str = "game.futty.build.ron";
pub const SCENE: &str = "game.futty.scene.ron";
pub const FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct Launch {
    pub format: u32,
    pub camera: Camera,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<SceneList>,
}

pub fn validate_windows_template(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut file = fs::File::open(path)?;
    let mut dos = [0; 64];
    file.read_exact(&mut dos)?;
    if &dos[..2] != b"MZ" {
        return Err("Expected a Windows x64 Futty executable".into());
    }
    let offset = u32::from_le_bytes(dos[60..64].try_into()?);
    file.seek(SeekFrom::Start(u64::from(offset)))?;
    let mut pe = [0; 6];
    file.read_exact(&mut pe)?;
    if pe != *b"PE\0\0\x64\x86" {
        return Err("Expected a Windows x64 Futty executable".into());
    }
    Ok(())
}

pub fn export(
    source: &Path,
    destination: &Path,
    camera: Camera,
    executable: &Path,
) -> Result<(), Box<dyn Error>> {
    if destination.try_exists()? {
        return Err("Choose a new output folder. The destination already exists.".into());
    }
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stage = tempfile::Builder::new()
        .prefix(".futty-build-")
        .tempdir_in(parent)?;
    let project = if is_project(source) {
        let project = Project::load(source)?;
        let mut list = project.list().clone();
        for (index, (id, entry)) in list.scenes.iter_mut().enumerate() {
            let folder = format!("Scenes/{index}");
            fs::create_dir_all(stage.path().join(&folder))?;
            *entry = format!("{folder}/{SCENE}");
            copy_scene(&project.resolve(id)?, &stage.path().join(&*entry))
                .map_err(|error| format!("Scene {id}: {error}"))?;
        }
        Some(list)
    } else {
        copy_scene(source, &stage.path().join(SCENE))?;
        None
    };
    let launch = Launch {
        format: if project.is_some() { 2 } else { FORMAT },
        camera,
        project,
    };
    fs::write(stage.path().join(MANIFEST), ron::to_string(&launch)?)?;
    fs::copy(executable, stage.path().join("Game.exe"))?;
    fs::rename(stage.path(), destination)?;
    Ok(())
}

fn copy_scene(source: &Path, entry: &Path) -> Result<(), Box<dyn Error>> {
    let mut scene = Scene::load(source)?;
    for object in &mut scene.entities {
        object.prefab = None;
    }
    if scene.entities.iter().all(|object| object.script.is_none()) {
        scene.prefabs.clear();
    } else {
        for prefab in scene.prefabs.values_mut() {
            prefab.source = None;
        }
    }
    let referenced_materials: HashSet<_> = scene
        .referenced_objects()
        .filter_map(|object| object.material)
        .collect();
    scene
        .materials
        .retain(|id, _| referenced_materials.contains(id));
    let mut used: HashSet<_> = scene
        .referenced_objects()
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
    used.extend(scene.hud.iter().filter_map(|widget| widget.image));
    scene.assets.retain(|asset| used.contains(&asset.id));
    scene.script_sources.retain(|id, _| used.contains(id));
    let mut audio = Audio::default();
    for asset in &scene.assets {
        if let Some(source_code) = scene.script_sources.get(&asset.id) {
            check_script(asset, source_code)?;
            continue;
        }
        let bytes = futty_assets::read_saved(source, asset)
            .map_err(|error| format!("{}: {error}", asset.name))?;
        validate_asset(&scene, asset, &bytes, &mut audio)
            .map_err(|error| format!("{}: {error}", asset.name))?;
        futty_assets::save(entry, asset, &bytes)?;
    }
    scene.save(entry)
}

fn validate_asset(
    scene: &Scene,
    asset: &Asset,
    bytes: &[u8],
    audio: &mut Audio,
) -> Result<(), Box<dyn Error>> {
    match asset.kind {
        AssetKind::Model => {
            futty_assets::model_with_clips(bytes, scene.model_animation_references(asset.id))?;
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
