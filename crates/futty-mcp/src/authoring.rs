use std::{collections::BTreeMap, fmt::Display, path::PathBuf};

use futty_core::{AssetKind, Light, LightKind, Primitive};
use futty_runtime::{
    Body, CharacterController, HudAnchor, HudKind, HudWidget, Motion, Object, Scene,
};
use glam::{EulerRot, Quat, Vec2, Vec3};
use rmcp::{ErrorData, model::CallToolResult, schemars};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{
    camera_object,
    components::{ComponentKind, ComponentPatch},
};

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObjectQuery {
    pub path: String,
    tag: Option<String>,
    #[schemars(with = "Option<String>")]
    object_id: Option<Uuid>,
    #[serde(default)]
    offset: usize,
    /// Page size, from 1 to 100. Defaults to 50.
    limit: Option<usize>,
}

impl ObjectQuery {
    pub fn list(&self, scene: &Scene) -> CallToolResult {
        let matching = scene.entities.iter().filter(|object| {
            self.object_id.is_none_or(|id| id == object.uuid)
                && self
                    .tag
                    .as_ref()
                    .is_none_or(|tag| object.tags.contains(tag))
        });
        let total = matching.clone().count();
        let objects: Vec<_> = matching
            .skip(self.offset)
            .take(self.limit.unwrap_or(50).clamp(1, 100))
            .map(|object| {
                json!({
                    "id": object.uuid,
                    "name": object.name,
                    "tags": object.tags,
                    "parent": object.parent,
                    "position": object.transform.translation.to_array(),
                    "rotation_xyzw": object.transform.rotation.to_array(),
                    "scale": object.transform.scale.to_array(),
                    "shape": object.primitive,
                    "mesh": object.mesh,
                    "texture": object.texture,
                    "material": object.material,
                    "script": object.script,
                    "audio_source": object.audio_source,
                    "body": object.body,
                    "camera": object.camera,
                    "light": object.light,
                    "character": object.character,
                    "prefab": object.prefab,
                })
            })
            .collect();
        let end = self.offset.saturating_add(objects.len());
        CallToolResult::structured(json!({
            "objects": objects,
            "total": total,
            "next_offset": (end < total).then_some(end),
        }))
    }
}

pub(crate) struct UndoRecord {
    pub path: PathBuf,
    pub token: Uuid,
    pub before: Scene,
    pub after: Scene,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Undo {
    pub path: String,
    pub undo_token: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Batch {
    pub path: String,
    edits: Vec<Operation>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    AddObject {
        /// Key for later operations in this batch.
        key: String,
        name: String,
        /// Cube, Sphere, Cylinder, Cone or Plane. Defaults to Cube.
        shape: Option<String>,
        properties: Option<Properties>,
    },
    AddCamera {
        key: String,
        name: String,
        position: [f32; 3],
        target: [f32; 3],
        /// Add a player capsule driven by the existing movement actions.
        #[serde(default)]
        character: bool,
    },
    AddPointLight {
        key: String,
        name: String,
        position: [f32; 3],
    },
    EditObject {
        /// Existing object UUID or a key defined earlier in the batch.
        object: String,
        properties: Properties,
    },
    SetComponent {
        object: String,
        component: ComponentPatch,
    },
    RemoveComponent {
        object: String,
        component: ComponentKind,
    },
    SetParent {
        object: String,
        /// Existing UUID or batch key. Omit or use null to move to scene root. Local transform is kept.
        parent: Option<String>,
    },
    RemoveLeaf {
        object: String,
    },
    PlacePrefab {
        key: String,
        #[schemars(with = "String")]
        prefab_id: Uuid,
        position: [f32; 3],
    },
    AttachAsset {
        object: String,
        #[schemars(with = "String")]
        asset_id: Uuid,
    },
    SetMaterial {
        object: String,
        /// Omit or use null to clear the shared material assignment.
        #[schemars(with = "Option<String>")]
        material_id: Option<Uuid>,
    },
    SetScript {
        object: String,
        name: String,
        source: String,
    },
    AttachScript {
        object: String,
        #[schemars(with = "Option<String>")]
        script_id: Option<Uuid>,
    },
    AddHudText {
        label: String,
        binding: String,
        /// Top-left offset in HUD pixels.
        offset: [f32; 2],
        size: [f32; 2],
    },
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Collider {
    None,
    Fixed,
    Kinematic,
    Dynamic,
    Trigger,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Properties {
    name: Option<String>,
    tags: Option<Vec<String>>,
    position: Option<[f32; 3]>,
    /// XYZ Euler angles in degrees.
    rotation: Option<[f32; 3]>,
    scale: Option<[f32; 3]>,
    color: Option<[f32; 3]>,
    collider: Option<Collider>,
}

impl Properties {
    fn apply(self, object: &mut Object) -> Result<(), ErrorData> {
        if let Some(name) = self.name {
            object.name = name;
        }
        if let Some(tags) = self.tags {
            object.tags = tags;
        }
        if let Some(position) = self.position {
            object.transform.translation = position.into();
        }
        if let Some([x, y, z]) = self.rotation {
            if ![x, y, z].into_iter().all(f32::is_finite) {
                return Err(invalid("Rotation angles must be finite"));
            }
            object.transform.rotation = Quat::from_euler(
                EulerRot::XYZ,
                x.to_radians(),
                y.to_radians(),
                z.to_radians(),
            )
            .normalize();
        }
        if let Some(scale) = self.scale {
            object.transform.scale = scale.into();
        }
        if let Some(color) = self.color {
            object.color = color;
        }
        if let Some(collider) = self.collider {
            object.body = match collider {
                Collider::None => None,
                collider => Some(Body {
                    motion: match collider {
                        Collider::Kinematic => Motion::Kinematic,
                        Collider::Dynamic => Motion::Dynamic,
                        _ => Motion::Fixed,
                    },
                    sensor: matches!(collider, Collider::Trigger),
                    ..object.body.unwrap_or_default()
                }),
            };
        }
        Ok(())
    }
}

impl Batch {
    pub fn attach_script(input: crate::scripts::AttachScript) -> Self {
        Self {
            path: input.path,
            edits: vec![Operation::AttachScript {
                object: input.object_id.to_string(),
                script_id: input.script_id,
            }],
        }
    }

    pub fn apply(self, scene: &mut Scene) -> Result<BTreeMap<String, Uuid>, ErrorData> {
        if !(1..=64).contains(&self.edits.len()) {
            return Err(invalid("Provide between 1 and 64 scene edits"));
        }
        let mut keys = BTreeMap::new();
        for (index, edit) in self.edits.into_iter().enumerate() {
            apply_edit(edit, scene, &mut keys)
                .map_err(|error| invalid(format!("Edit {}: {}", index + 1, error.message)))?;
        }
        Ok(keys)
    }
}

fn apply_edit(
    edit: Operation,
    scene: &mut Scene,
    keys: &mut BTreeMap<String, Uuid>,
) -> Result<(), ErrorData> {
    match edit {
        Operation::AddObject {
            key,
            name,
            shape,
            properties,
        } => {
            let mut object = Object::cube(name, Vec3::ZERO);
            object.spin = false;
            if let Some(shape) = shape {
                object.primitive = Primitive::ALL
                    .into_iter()
                    .find(|primitive| primitive.label().eq_ignore_ascii_case(&shape))
                    .ok_or_else(|| invalid(format!("Unknown shape {shape:?}")))?;
            }
            if let Some(properties) = properties {
                properties.apply(&mut object)?;
            }
            register(keys, key, object.uuid)?;
            scene.entities.push(object);
        }
        Operation::AddCamera {
            key,
            name,
            position,
            target,
            character,
        } => {
            let mut object = camera_object(name, position, target)?;
            object.character = character.then(CharacterController::default);
            register(keys, key, object.uuid)?;
            if scene.camera_index(None).is_none() {
                scene.main_camera = Some(object.uuid);
            }
            scene.entities.push(object);
        }
        Operation::AddPointLight {
            key,
            name,
            position,
        } => {
            let mut object = Object::cube(name, position.into());
            object.spin = false;
            object.light = Some(Light::new(LightKind::Point));
            register(keys, key, object.uuid)?;
            scene.entities.push(object);
        }
        Operation::EditObject { object, properties } => {
            let index = object_index(&object, scene, keys)?;
            properties.apply(&mut scene.entities[index])?;
        }
        Operation::SetComponent { object, component } => {
            let index = object_index(&object, scene, keys)?;
            component.apply(&mut scene.entities[index])?;
        }
        Operation::RemoveComponent { object, component } => {
            let index = object_index(&object, scene, keys)?;
            component.remove(&mut scene.entities[index])?;
        }
        Operation::SetParent { object, parent } => {
            let index = object_index(&object, scene, keys)?;
            let parent = parent
                .map(|parent| object_index(&parent, scene, keys))
                .transpose()?
                .map(|parent| scene.entities[parent].uuid);
            scene.entities[index].parent = parent;
        }
        Operation::RemoveLeaf { object } => {
            let index = object_index(&object, scene, keys)?;
            let id = scene.entities[index].uuid;
            if scene.entities[index].prefab.is_some()
                || scene
                    .entities
                    .iter()
                    .any(|object| object.parent == Some(id))
            {
                return Err(invalid(
                    "Remove only leaf objects outside linked prefab instances",
                ));
            }
            scene.entities.remove(index);
            keys.retain(|_, value| *value != id);
            if scene.main_camera == Some(id) {
                scene.main_camera = None;
            }
        }
        Operation::PlacePrefab {
            key,
            prefab_id,
            position,
        } => {
            let id = scene
                .instantiate_prefab(prefab_id, position.into())
                .map_err(invalid)?;
            register(keys, key, id)?;
        }
        Operation::AttachAsset { object, asset_id } => {
            let index = object_index(&object, scene, keys)?;
            let kind = scene
                .assets
                .iter()
                .find(|asset| asset.id == asset_id)
                .ok_or_else(|| invalid(format!("Unknown asset {asset_id}")))?
                .kind;
            let object = &mut scene.entities[index];
            match kind {
                AssetKind::Model if object.has_renderer() && object.sprite.is_none() => {
                    if object.mesh != Some(asset_id) {
                        object.mesh = Some(asset_id);
                        object.animation = None;
                    }
                }
                AssetKind::Texture if object.has_renderer() => {
                    if let Some(sprite) = &mut object.sprite {
                        sprite.texture = asset_id;
                    } else {
                        object.texture = Some(asset_id);
                    }
                }
                AssetKind::Script => {
                    crate::scripts::assign(object, Some(asset_id));
                }
                AssetKind::Wav | AssetKind::Ogg if object.prefab.is_none() => {
                    let mut source = object
                        .audio_source
                        .unwrap_or_else(|| futty_runtime::AudioSource::new(asset_id));
                    source.clip = asset_id;
                    object.audio_source = Some(source);
                }
                _ => {
                    return Err(invalid(
                        "This object does not accept the selected asset kind",
                    ));
                }
            }
        }
        Operation::SetMaterial {
            object,
            material_id,
        } => {
            let index = object_index(&object, scene, keys)?;
            if let Some(id) = material_id
                && !scene.materials.contains_key(&id)
            {
                return Err(invalid(format!("Unknown material {id}")));
            }
            let object = &mut scene.entities[index];
            if material_id.is_some() && (!object.has_renderer() || object.sprite.is_some()) {
                return Err(invalid("Shared materials require a 3D renderable object"));
            }
            object.material = material_id;
        }
        Operation::SetScript {
            object,
            name,
            source,
        } => {
            let index = object_index(&object, scene, keys)?;
            futty_runtime::inspect_script(&source).map_err(invalid)?;
            let id = crate::scripts::register(scene, name, source);
            crate::scripts::assign(&mut scene.entities[index], Some(id));
        }
        Operation::AttachScript { object, script_id } => {
            let index = object_index(&object, scene, keys)?;
            if let Some(id) = script_id {
                crate::scripts::get_asset(scene, id)?;
            }
            let object = &mut scene.entities[index];
            if object.prefab.is_some() {
                return Err(invalid(
                    "Edit the prefab source object to change its script",
                ));
            }
            crate::scripts::assign(object, script_id);
        }
        Operation::AddHudText {
            label,
            binding,
            offset,
            size,
        } => {
            let mut widget = HudWidget::new(HudKind::Text);
            widget.anchor = HudAnchor::TopLeft;
            widget.label = label;
            widget.binding = binding;
            widget.offset = Vec2::from_array(offset);
            widget.size = Vec2::from_array(size);
            scene.hud.push(widget);
        }
    }
    Ok(())
}

fn register(keys: &mut BTreeMap<String, Uuid>, key: String, id: Uuid) -> Result<(), ErrorData> {
    if key.is_empty()
        || key.len() > 40
        || !key
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        || Uuid::parse_str(&key).is_ok()
        || keys.contains_key(&key)
    {
        return Err(invalid(format!("Invalid or duplicate batch key {key:?}")));
    }
    keys.insert(key, id);
    Ok(())
}

fn object_index(
    value: &str,
    scene: &Scene,
    keys: &BTreeMap<String, Uuid>,
) -> Result<usize, ErrorData> {
    let id = Uuid::parse_str(value)
        .ok()
        .or_else(|| keys.get(value).copied());
    scene
        .entities
        .iter()
        .position(|object| Some(object.uuid) == id)
        .ok_or_else(|| invalid(format!("Unknown scene object {value:?}")))
}

pub(crate) fn invalid(error: impl Display) -> ErrorData {
    ErrorData::invalid_params(error.to_string(), None)
}
