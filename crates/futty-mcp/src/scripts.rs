use futty_core::{Asset, AssetKind};
use futty_runtime::{Object, Scene, ScriptFields, inspect_script_diagnostic};
use rmcp::{ErrorData, schemars};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::authoring::invalid;

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ValidateScript {
    /// Inline Rhai source. Provide either source or path.
    pub source: Option<String>,
    /// Existing workspace-relative .rhai file.
    pub path: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateScript {
    /// Existing workspace-relative scene.
    pub path: String,
    pub name: String,
    pub source: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateScript {
    pub path: String,
    #[schemars(with = "String")]
    pub script_id: Uuid,
    pub source: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScriptQuery {
    pub path: String,
    #[schemars(with = "String")]
    pub script_id: Uuid,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AttachScript {
    pub path: String,
    #[schemars(with = "String")]
    pub object_id: Uuid,
    /// Omit or use null to detach the script and clear its field overrides.
    #[schemars(with = "Option<String>")]
    pub script_id: Option<Uuid>,
}

pub(crate) fn get_asset(scene: &Scene, id: Uuid) -> Result<&Asset, ErrorData> {
    scene
        .assets
        .iter()
        .find(|asset| asset.id == id && asset.kind == AssetKind::Script)
        .ok_or_else(|| invalid(format!("Unknown script {id}")))
}

pub(crate) fn check(
    source: &str,
    scene_path: &str,
    id: Option<Uuid>,
) -> Result<ScriptFields, ErrorData> {
    inspect_script_diagnostic(source).map_err(|error| {
        let message = error.message.clone();
        ErrorData::invalid_params(
            message,
            Some(json!({
                "scene_path": scene_path, "script_id": id, "issue": error,
            })),
        )
    })
}

pub(crate) fn register(scene: &mut Scene, name: String, source: String) -> Uuid {
    let id = Uuid::new_v4();
    scene.assets.push(Asset {
        id,
        name,
        kind: AssetKind::Script,
    });
    scene.script_sources.insert(id, source);
    id
}

pub(crate) fn assign(object: &mut Object, script: Option<Uuid>) {
    if object.script != script || script.is_none() {
        object.script = script;
        object.script_fields.clear();
    }
}
