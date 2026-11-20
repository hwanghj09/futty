use futty_core::{Asset, AssetKind};
use futty_runtime::{Material, Scene};
use rmcp::{ErrorData, schemars};
use serde::{Deserialize, Deserializer};
use uuid::Uuid;

use crate::authoring::invalid;

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
