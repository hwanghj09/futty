use std::collections::BTreeMap;

use futty_runtime::Scene;
use rmcp::schemars;
use serde::Deserialize;
use serde_json::{Map, Value, json};

const DETAIL_LIMIT: usize = 64;

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Compare {
    /// Workspace-relative original scene.
    pub before_path: String,
    /// Workspace-relative scene to compare against the original.
    pub after_path: String,
}

pub(crate) fn changes(before: &Scene, after: &Scene) -> Value {
    let Value::Object(mut before) = json!(before) else {
        unreachable!()
    };
    let Value::Object(mut after) = json!(after) else {
        unreachable!()
    };
    let mut diff = Map::new();
    for (field, label, id) in [
        ("entities", "objects", Some("uuid")),
        ("assets", "assets", Some("id")),
        ("hud", "hud", Some("id")),
        ("materials", "materials", None),
        ("prefabs", "prefabs", None),
        ("script_sources", "scripts", None),
    ] {
        let old = before.remove(field).unwrap_or_default();
        let new = after.remove(field).unwrap_or_default();
        if old != new {
            diff.insert(
                label.into(),
                collection(&old, &new, id, !matches!(label, "prefabs" | "scripts")),
            );
        }
    }
    let scene = fields(&before, &after);
    if !scene.is_empty() {
        diff.insert("scene".into(), Value::Object(scene));
    }
    Value::Object(diff)
}

fn indexed<'a>(value: &'a Value, id: Option<&str>) -> BTreeMap<String, &'a Value> {
    match (value, id) {
        (Value::Array(entries), Some(id)) => entries
            .iter()
            .filter_map(|entry| Some((entry.get(id)?.as_str()?.to_owned(), entry)))
            .collect(),
        (Value::Object(entries), None) => entries
            .iter()
            .map(|(id, entry)| (id.clone(), entry))
            .collect(),
        _ => BTreeMap::new(),
    }
}

fn fields(before: &Map<String, Value>, after: &Map<String, Value>) -> Map<String, Value> {
    let mut changes = Map::new();
    for (field, value) in before {
        let next = after.get(field);
        if Some(value) != next {
            changes.insert(field.clone(), json!({"before":value,"after":next}));
        }
    }
    for (field, value) in after {
        if !before.contains_key(field) {
            changes.insert(field.clone(), json!({"before":null,"after":value}));
        }
    }
    changes
}

fn collection(old: &Value, new: &Value, id: Option<&str>, include_fields: bool) -> Value {
    let before = indexed(old, id);
    let after = indexed(new, id);
    let removed: Vec<_> = before
        .iter()
        .filter(|(id, _)| !after.contains_key(*id))
        .collect();
    let added: Vec<_> = after
        .iter()
        .filter(|(id, _)| !before.contains_key(*id))
        .collect();
    let updated: Vec<_> = before
        .iter()
        .filter_map(|(id, old)| {
            let new = *after.get(id)?;
            (*old != new).then_some((id, *old, new))
        })
        .collect();
    let describe = |id: &str, value: &Value| json!({"id":id,"name":value.get("name").or_else(|| value.get("label"))});
    let added_items: Vec<_> = added
        .iter()
        .take(DETAIL_LIMIT)
        .map(|(id, value)| describe(id, value))
        .collect();
    let removed_items: Vec<_> = removed
        .iter()
        .take(DETAIL_LIMIT)
        .map(|(id, value)| describe(id, value))
        .collect();
    let updated_items: Vec<_> = updated
        .iter()
        .take(DETAIL_LIMIT)
        .map(|(id, old, new)| {
            let mut item = describe(id, new);
            if include_fields && let (Some(old), Some(new)) = (old.as_object(), new.as_object()) {
                item["fields"] = Value::Object(fields(old, new));
            }
            item
        })
        .collect();
    let mut result = json!({
        "added_count":added.len(),"removed_count":removed.len(),"updated_count":updated.len(),
        "added":added_items,"removed":removed_items,"updated":updated_items,
    });
    if let Some(id) = id {
        let order = |value: &Value| -> Vec<String> {
            value
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|entry| entry[id].as_str())
                .filter(|id| before.contains_key(*id) && after.contains_key(*id))
                .map(str::to_owned)
                .collect()
        };
        if order(old) != order(new) {
            result["order_changed"] = json!(true);
        }
    }
    result
}
