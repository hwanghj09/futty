use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use futty_core::{Asset, AssetKind};
use futty_runtime::{Billboard, HudKind, Material, Scene};
use rmcp::{ErrorData, model::CallToolResult, schemars};
use serde::{Deserialize, Deserializer};
use serde_json::json;
use uuid::Uuid;

use crate::{
    Futty,
    authoring::{self, invalid},
    io_error,
};

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImportAsset {
    /// Workspace-relative saved scene.
    pub path: String,
    /// Workspace-relative source GLB, PNG, WAV or Ogg file.
    pub source: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReimportAsset {
    pub path: String,
    pub source: String,
    #[schemars(with = "String")]
    pub asset_id: Uuid,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImportAssetBatch {
    /// Workspace-relative saved scene.
    pub path: String,
    /// One to 32 imports or replacements, with up to 64 bindings in total.
    assets: Vec<BatchAsset>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct BatchAsset {
    /// Unique key for looking up the asset in the result.
    key: String,
    source: String,
    /// Set to replace a registered asset of the same kind while retaining its UUID.
    #[schemars(with = "Option<String>")]
    asset_id: Option<Uuid>,
    #[serde(default)]
    bind: Vec<AssetBinding>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(tag = "target", rename_all = "snake_case", deny_unknown_fields)]
enum AssetBinding {
    /// Attach a model, object texture, existing sprite texture or audio clip.
    Object {
        #[schemars(with = "String")]
        id: Uuid,
    },
    /// Configure the target object as a Sprite3D using a PNG.
    Sprite {
        #[schemars(with = "String")]
        id: Uuid,
        size: Option<[f32; 2]>,
        #[schemars(with = "Option<String>")]
        billboard: Option<Billboard>,
    },
    Material {
        #[schemars(with = "String")]
        id: Uuid,
    },
    HudImage {
        #[schemars(with = "String")]
        id: Uuid,
    },
}

impl AssetBinding {
    fn apply(self, scene: &mut Scene, asset: &Asset) -> Result<(), ErrorData> {
        let find_object = |id| {
            scene
                .entities
                .iter()
                .position(|object| object.uuid == id)
                .ok_or_else(|| invalid(format!("Unknown scene object {id}")))
        };
        match self {
            Self::Object { id } => {
                let index = find_object(id)?;
                if scene.entities[index].prefab.is_some() {
                    return Err(invalid(
                        "Bind assets to the prefab source instead of an instance",
                    ));
                }
                authoring::attach_asset(&mut scene.entities[index], asset.id, asset.kind)?;
            }
            Self::Sprite {
                id,
                size,
                billboard,
            } if asset.kind == AssetKind::Texture => {
                let index = find_object(id)?;
                authoring::set_sprite(&mut scene.entities[index], asset.id, size, billboard)?;
            }
            Self::Material { id } if asset.kind == AssetKind::Texture => {
                scene
                    .materials
                    .get_mut(&id)
                    .ok_or_else(|| invalid(format!("Unknown material {id}")))?
                    .texture = Some(asset.id);
            }
            Self::HudImage { id } if asset.kind == AssetKind::Texture => {
                let widget = scene
                    .hud
                    .iter_mut()
                    .find(|widget| widget.id == id && widget.kind == HudKind::Image)
                    .ok_or_else(|| invalid(format!("Unknown image widget {id}")))?;
                widget.image = Some(asset.id);
            }
            _ => return Err(invalid("This binding requires a PNG texture")),
        }
        Ok(())
    }
}

struct PreparedAsset {
    asset: Asset,
    bytes: Vec<u8>,
    previous: Option<Vec<u8>>,
}

pub(crate) fn import_batch(
    server: &Futty,
    input: ImportAssetBatch,
) -> Result<CallToolResult, ErrorData> {
    if !(1..=32).contains(&input.assets.len())
        || input
            .assets
            .iter()
            .map(|asset| asset.bind.len())
            .sum::<usize>()
            > 64
    {
        return Err(invalid("Provide 1 to 32 assets and up to 64 bindings"));
    }
    let _lock = server.lock_edits().map_err(io_error)?;
    let path = server.scene_path(&input.path, true)?;
    let original = fs::read_to_string(&path).map_err(io_error)?;
    let before = Scene::decode(&original).map_err(io_error)?;
    let mut after = before.clone();
    let mut prepared = Vec::new();
    let mut keys = BTreeMap::new();
    let mut replacements = HashSet::new();
    let mut total_bytes = 0;
    for entry in input.assets {
        let (source, kind) = server.asset_source(&entry.source)?;
        let asset = if let Some(id) = entry.asset_id {
            if !replacements.insert(id) {
                return Err(invalid(format!("Duplicate replacement for asset {id}")));
            }
            let asset = before
                .assets
                .iter()
                .find(|asset| asset.id == id)
                .ok_or_else(|| invalid(format!("Unknown asset {id}")))?;
            if asset.kind != kind {
                return Err(invalid(
                    "The replacement must match the registered asset kind",
                ));
            }
            asset.clone()
        } else {
            let asset = new_asset(&entry.source, kind);
            after.assets.push(asset.clone());
            asset
        };
        authoring::register(&mut keys, entry.key, asset.id)?;
        let bytes = futty_assets::read(&source).map_err(io_error)?;
        let previous = if entry.asset_id.is_some() {
            read_previous(&path, &asset)?
        } else {
            None
        };
        total_bytes += bytes.len() + previous.as_ref().map_or(0, Vec::len);
        if total_bytes > 128 * 1024 * 1024 {
            return Err(invalid(
                "Batch exceeds 128 MiB including replacement backups",
            ));
        }
        for binding in entry.bind {
            binding.apply(&mut after, &asset)?;
        }
        prepared.push(PreparedAsset {
            asset,
            bytes,
            previous,
        });
    }
    after.sync_prefabs(&before).map_err(invalid)?;
    after.validate().map_err(invalid)?;
    for item in &prepared {
        validate_asset(&after, &item.asset, &item.bytes)
            .map_err(|error| invalid(format!("{}: {}", item.asset.name, error.message)))?;
    }
    publish_batch(&path, &prepared, || {
        if fs::read_to_string(&path)? != original {
            return Err("Scene changed on disk during the batch".into());
        }
        if before != after {
            after.save(&path)?;
        }
        Ok(())
    })
    .map_err(io_error)?;
    Ok(CallToolResult::structured(
        json!({"path":input.path,"assets":keys}),
    ))
}

pub(crate) fn new_asset(source: &str, kind: AssetKind) -> Asset {
    Asset {
        id: Uuid::new_v4(),
        name: Path::new(source)
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("Asset")
            .into(),
        kind,
    }
}

pub(crate) fn read_previous(path: &Path, asset: &Asset) -> Result<Option<Vec<u8>>, ErrorData> {
    match futty_assets::read_saved(path, asset) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
        {
            Ok(None)
        }
        Err(error) => Err(io_error(error)),
    }
}

fn publish_batch(
    scene: &Path,
    assets: &[PreparedAsset],
    save_scene: impl FnOnce() -> futty_assets::AssetResult<()>,
) -> futty_assets::AssetResult<()> {
    let mut written: Vec<(&PreparedAsset, Option<PathBuf>)> = Vec::new();
    // shortcut: rollback reported write errors in process, add a disk journal for crash recovery
    let result = (|| {
        for item in assets {
            let new_path = if let Some(previous) = item.previous.as_deref() {
                futty_assets::replace_saved(scene, &item.asset, Some(previous), &item.bytes)?;
                if previous == item.bytes {
                    continue;
                }
                None
            } else {
                Some(futty_assets::save_new(scene, &item.asset, &item.bytes)?)
            };
            written.push((item, new_path));
        }
        save_scene()
    })();
    if let Err(error) = result {
        let mut failures = Vec::new();
        for (item, new_path) in written.into_iter().rev() {
            let restored = if let Some(new_path) = new_path {
                futty_assets::read_saved(scene, &item.asset).and_then(|bytes| {
                    if bytes != item.bytes {
                        return Err("Asset changed on disk after the batch write".into());
                    }
                    Ok(fs::remove_file(new_path)?)
                })
            } else {
                futty_assets::replace_saved(
                    scene,
                    &item.asset,
                    Some(&item.bytes),
                    item.previous.as_deref().unwrap(),
                )
            };
            if let Err(failure) = restored {
                failures.push(format!(
                    "{} ({}): {failure}",
                    item.asset.name, item.asset.id
                ));
            }
        }
        if !failures.is_empty() {
            return Err(
                format!("{error}; asset rollback failed for {}", failures.join(", ")).into(),
            );
        }
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_file_conflict_restores_old_bytes_and_removes_new_files() {
        let dir = tempfile::tempdir().unwrap();
        let scene = dir.path().join("Scene.ron");
        let old = new_asset("Original.png", AssetKind::Texture);
        let fresh = new_asset("New.png", AssetKind::Texture);
        let conflict = new_asset("Conflict.png", AssetKind::Texture);
        futty_assets::save_new(&scene, &old, b"old").unwrap();
        futty_assets::save_new(&scene, &conflict, b"concurrent").unwrap();
        let assets = [
            PreparedAsset {
                asset: old,
                bytes: b"edited".to_vec(),
                previous: Some(b"old".to_vec()),
            },
            PreparedAsset {
                asset: fresh,
                bytes: b"new".to_vec(),
                previous: None,
            },
            PreparedAsset {
                asset: conflict,
                bytes: b"replacement".to_vec(),
                previous: Some(b"stale".to_vec()),
            },
        ];
        let error = publish_batch(&scene, &assets, || {
            panic!("Failed batches must not save the scene")
        })
        .unwrap_err();
        assert!(error.to_string().contains("changed on disk"));
        assert_eq!(
            futty_assets::read_saved(&scene, &assets[0].asset).unwrap(),
            b"old"
        );
        assert!(read_previous(&scene, &assets[1].asset).unwrap().is_none());
        assert_eq!(
            futty_assets::read_saved(&scene, &assets[2].asset).unwrap(),
            b"concurrent"
        );
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateMaterial {
    pub path: String,
    pub settings: MaterialPatch,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateMaterial {
    pub path: String,
    #[schemars(with = "String")]
    pub material_id: Uuid,
    pub settings: MaterialPatch,
}

pub(crate) fn validate_asset(scene: &Scene, asset: &Asset, bytes: &[u8]) -> Result<(), ErrorData> {
    match asset.kind {
        AssetKind::Model => {
            futty_assets::model_with_clips(bytes, scene.model_animation_references(asset.id))
                .map_err(invalid)?;
        }
        AssetKind::Texture => {
            futty_assets::png(bytes).map_err(invalid)?;
        }
        AssetKind::Wav | AssetKind::Ogg => {
            futty_audio::Audio::default()
                .load(asset.id, asset.kind.extension(), bytes)
                .map_err(invalid)?;
        }
        AssetKind::Script => return Err(invalid("Use set_script to author Rhai scripts")),
    }
    Ok(())
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct MaterialPatch {
    name: Option<String>,
    color: Option<[f32; 3]>,
    /// Omit to keep the current texture. Null clears the texture.
    #[serde(default, deserialize_with = "texture_patch")]
    #[schemars(with = "Option<String>")]
    texture: Option<Option<Uuid>>,
    roughness: Option<f32>,
    emission: Option<[f32; 3]>,
    emission_strength: Option<f32>,
}

fn texture_patch<'de, D: Deserializer<'de>>(input: D) -> Result<Option<Option<Uuid>>, D::Error> {
    Option::deserialize(input).map(Some)
}

impl MaterialPatch {
    pub fn apply(self, material: &mut Material) {
        if let Some(name) = self.name {
            material.name = name;
        }
        material.color = self.color.unwrap_or(material.color);
        material.texture = self.texture.unwrap_or(material.texture);
        material.surface.roughness = self.roughness.unwrap_or(material.surface.roughness);
        material.surface.emission = self.emission.unwrap_or(material.surface.emission);
        material.surface.emission_strength = self
            .emission_strength
            .unwrap_or(material.surface.emission_strength);
    }
}
