use std::path::PathBuf;

use futty_runtime::Scene;
use rmcp::{ErrorData, schemars};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{authoring::invalid, diff, scripts};

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Change {
    pub path: String,
    #[schemars(with = "String")]
    pub object_id: Uuid,
    /// Object tint in normalized RGB.
    color: Option<[f32; 3]>,
    /// A new embedded Rhai asset attached only to the selected object.
    script: Option<Replacement>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Replacement {
    name: String,
    source: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Apply {
    pub path: String,
    #[schemars(with = "String")]
    pub preview_token: Uuid,
}

pub(crate) struct Preview {
    pub path: PathBuf,
    pub revision: String,
    pub token: Uuid,
    pub after: Scene,
}

impl Change {
    pub fn prepare(self, before: &Scene) -> Result<(Scene, Value), ErrorData> {
        let id = self.object_id;
        let index = before
            .entities
            .iter()
            .position(|object| object.uuid == id)
            .ok_or_else(|| invalid(format!("Unknown object {id}")))?;
        if self.color.is_none() && self.script.is_none() {
            return Err(invalid("Provide a color, a replacement script, or both"));
        }
        let original = &before.entities[index];
        if original.prefab.is_some() {
            return Err(invalid(
                "This operation requires an object outside a prefab instance",
            ));
        }
        let mut after = before.clone();
        let script_id = if let Some(script) = self.script {
            let name = script.name.trim();
            if name.is_empty() || name.len() > 256 {
                return Err(invalid("Script name must contain 1 to 256 bytes"));
            }
            scripts::check(&script.source, &self.path, None)?;
            Some(scripts::register(&mut after, name.into(), script.source))
        } else {
            None
        };
        let target = &mut after.entities[index];
        if let Some(color) = self.color {
            target.color = color;
        }
        if let Some(script_id) = script_id {
            scripts::assign(target, Some(script_id));
        }
        let changed = original != target;
        let clears_script_fields =
            !original.script_fields.is_empty() && target.script_fields.is_empty();
        after.sync_prefabs(before).map_err(invalid)?;
        if after.prefabs != before.prefabs
            || after.entities.len() != before.entities.len()
            || after
                .entities
                .iter()
                .zip(&before.entities)
                .enumerate()
                .any(|(i, (next, old))| i != index && next != old)
        {
            return Err(invalid(
                "Changing this target would also update a prefab; use scene_preview_ops to review the wider change",
            ));
        }
        after.validate().map_err(invalid)?;
        let other_script_users = original.script.map_or(0, |script| {
            before
                .entities
                .iter()
                .filter(|object| object.uuid != id && object.script == Some(script))
                .count()
        });
        let info = json!({
            "valid": true,
            "target": {"id": id, "name": original.name},
            "changed": changed,
            "affected_object_ids": if changed { vec![id] } else { Vec::new() },
            "prior_script_id": original.script,
            "other_objects_using_prior_script": other_script_users,
            "new_script_id": script_id,
            "clears_script_fields": clears_script_fields,
            "changes": diff::changes(before, &after),
        });
        Ok((after, info))
    }
}
