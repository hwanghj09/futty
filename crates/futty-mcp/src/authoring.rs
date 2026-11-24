use std::{collections::BTreeMap, fmt::Display, path::PathBuf};

use futty_core::{AssetKind, Light, LightKind, Primitive};
use futty_runtime::{
    Body, CharacterController, CharacterFeel, HudAnchor, HudKind, HudWidget, Motion, Object, Scene,
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
                    "sprite": object.sprite,
                    "camera_follow": object.camera_follow,
                    "material": object.material,
                    "script": object.script,
                    "audio_source": object.audio_source,
                    "body": object.body,
                    "camera": object.camera,
                    "light": object.light,
                    "character": object.character,
                    "interaction": object.interaction,
                    "health": object.health,
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
    /// Optional scene revision returned by scene_preview_ops. Reject if the scene file has changed.
    expected_revision: Option<String>,
    edits: Vec<Operation>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Operation {
    /// Enable or disable shared-screen local keyboard co-op.
    SetLocalCoop {
        enabled: bool,
    },
    /// Generate a connected 2D maze with fixed walls, a top-down player, and a goal marker.
    GenerateMaze2d {
        start_key: String,
        goal_key: String,
        layout: crate::layout::Maze2d,
    },
    /// Paint an atlas grid into a scene as editable sprites with optional fixed colliders.
    PaintTilemap2d {
        map: crate::layout::Tilemap2d,
    },
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
        /// Set an orthographic camera's vertical half-size in world units.
        orthographic_size: Option<f32>,
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
    TuneCharacter {
        object: String,
        speed: Option<f32>,
        jump_speed: Option<f32>,
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
    SetSprite {
        object: String,
        #[schemars(with = "String")]
        texture_id: Uuid,
        /// Width and height in world units.
        size: Option<[f32; 2]>,
        /// Fixed, FaceCamera or Upright. Defaults to Fixed on a new sprite.
        #[schemars(with = "Option<String>")]
        billboard: Option<futty_runtime::Billboard>,
    },
    SetSpriteSheet {
        object: String,
        /// Omit or use null to show the entire texture.
        sheet: Option<SpriteSheetSettings>,
    },
    /// Set a PNG sprite on the XY plane with explicit layer, pivot and flip settings.
    SetSprite2d {
        object: String,
        #[schemars(with = "String")]
        texture_id: Uuid,
        size: Option<[f32; 2]>,
        layer: Option<i16>,
        pivot: Option<[f32; 2]>,
        flip: Option<[bool; 2]>,
    },
    SetCameraFollow {
        object: String,
        /// Target scene tag. Omit or use null to disable following.
        tag: Option<String>,
        /// World-space X and Y offsets, defaulting to zero.
        offset: Option<[f32; 2]>,
        /// Enable X and Y following, defaulting to both axes.
        axes: Option<[bool; 2]>,
        /// Optional camera center bounds, as [minimum XY, maximum XY].
        bounds: Option<[[f32; 2]; 2]>,
    },
    /// Enable integer zoom and a world-space pixel grid on an orthographic camera.
    SetCameraPixelGrid {
        object: String,
        /// Sprite pixels per world unit, 1 to 1024. Omit or use null to disable.
        pixels_per_unit: Option<u16>,
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
#[serde(deny_unknown_fields)]
pub(crate) struct SpriteSheetSettings {
    columns: u16,
    rows: u16,
    /// Zero-based cell index, in row-major order.
    first_frame: u16,
    frames: u16,
    fps: f32,
    looping: bool,
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

pub(crate) fn read_revision(path: &std::path::Path) -> Result<(Scene, String), ErrorData> {
    let original = std::fs::read_to_string(path).map_err(crate::io_error)?;
    let mut fingerprint = crate::replay::Fingerprint::new();
    fingerprint.add(original.as_bytes());
    Ok((
        Scene::decode(&original).map_err(crate::io_error)?,
        fingerprint.finish(),
    ))
}

impl Batch {
    pub fn tune_character(
        path: String,
        expected_revision: Option<String>,
        object: Uuid,
        speed: Option<f32>,
        jump_speed: Option<f32>,
    ) -> Self {
        Self {
            path,
            expected_revision,
            edits: vec![Operation::TuneCharacter {
                object: object.to_string(),
                speed,
                jump_speed,
            }],
        }
    }

    pub fn place_prefab(input: crate::PlacePrefab) -> Self {
        Self {
            path: input.path,
            expected_revision: None,
            edits: vec![Operation::PlacePrefab {
                key: "instance".into(),
                prefab_id: input.prefab_id,
                position: input.position,
            }],
        }
    }

    pub fn attach_script(input: crate::scripts::AttachScript) -> Self {
        Self {
            path: input.path,
            expected_revision: None,
            edits: vec![Operation::AttachScript {
                object: input.object_id.to_string(),
                script_id: input.script_id,
            }],
        }
    }

    pub fn read(&self, path: &std::path::Path) -> Result<(Scene, String), ErrorData> {
        let (scene, revision) = read_revision(path)?;
        if let Some(expected) = &self.expected_revision
            && *expected != revision
        {
            return Err(ErrorData::invalid_params(
                "Scene changed since the requested revision",
                Some(json!({"expected_revision":expected,"current_revision":revision})),
            ));
        }
        Ok((scene, revision))
    }

    pub fn prepare(self, before: &Scene) -> Result<(Scene, BTreeMap<String, Uuid>), ErrorData> {
        if !(1..=64).contains(&self.edits.len()) {
            return Err(invalid("Provide between 1 and 64 scene edits"));
        }
        let mut scene = before.clone();
        let mut keys = BTreeMap::new();
        for (index, edit) in self.edits.into_iter().enumerate() {
            apply_edit(edit, &mut scene, &mut keys).map_err(|error| {
                let message = format!("Edit {}: {}", index + 1, error.message);
                ErrorData::invalid_params(
                    message.clone(),
                    Some(json!({
                        "issue":{"edit":index + 1,"message":message},
                    })),
                )
            })?;
        }
        scene.sync_prefabs(before).map_err(batch_issue)?;
        scene.validate().map_err(batch_issue)?;
        let ids: std::collections::HashSet<_> =
            scene.entities.iter().map(|object| object.uuid).collect();
        keys.retain(|_, id| ids.contains(id));
        Ok((scene, keys))
    }
}

fn batch_issue(error: Box<dyn std::error::Error>) -> ErrorData {
    let issue = error
        .downcast_ref::<futty_runtime::SceneIssue>()
        .map_or_else(
            || json!({"message":error.to_string()}),
            |issue| json!(issue),
        );
    ErrorData::invalid_params(error.to_string(), Some(json!({"issue":issue})))
}

fn apply_edit(
    edit: Operation,
    scene: &mut Scene,
    keys: &mut BTreeMap<String, Uuid>,
) -> Result<(), ErrorData> {
    match edit {
        Operation::PaintTilemap2d { map } => {
            let tiles = map.build(scene)?;
            scene.entities.extend(tiles);
        }
        Operation::SetLocalCoop { enabled } => scene.local_coop = enabled,
        Operation::GenerateMaze2d {
            start_key,
            goal_key,
            layout,
        } => {
            let (start, goal, walls) = layout.build()?;
            register(keys, start_key, start.uuid)?;
            register(keys, goal_key, goal.uuid)?;
            scene.entities.extend([start, goal]);
            scene.entities.extend(walls);
        }
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
            orthographic_size,
            character,
        } => {
            let mut object = camera_object(name, position, target)?;
            crate::set_orthographic(&mut object, orthographic_size)?;
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
        Operation::TuneCharacter {
            object,
            speed,
            jump_speed,
        } => {
            let index = object_index(&object, scene, keys)?;
            let controller = scene.entities[index]
                .character
                .as_mut()
                .ok_or_else(|| invalid("Object must already have a character controller"))?;
            if speed.is_none() && jump_speed.is_none() {
                return Err(invalid("Provide a move speed or jump speed"));
            }
            if jump_speed.is_some()
                && !CharacterFeel::supports_jump(controller.mode, scene.dimension)
            {
                return Err(invalid("This character's movement mode does not jump"));
            }
            CharacterFeel {
                speed: speed.unwrap_or(controller.speed),
                jump_speed: jump_speed.unwrap_or(controller.jump_speed),
            }
            .apply(controller)
            .map_err(invalid)?;
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
            attach_asset(&mut scene.entities[index], asset_id, kind)?;
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
        Operation::SetSprite {
            object,
            texture_id,
            size,
            billboard,
        } => {
            let index = object_index(&object, scene, keys)?;
            if !scene
                .assets
                .iter()
                .any(|asset| asset.id == texture_id && asset.kind == AssetKind::Texture)
            {
                return Err(invalid("Sprite3D requires a registered PNG texture"));
            }
            set_sprite(&mut scene.entities[index], texture_id, size, billboard)?;
        }
        Operation::SetSpriteSheet { object, sheet } => {
            let index = object_index(&object, scene, keys)?;
            let object = &mut scene.entities[index];
            if object.prefab.is_some() {
                return Err(invalid("Edit the prefab source to change its sprite sheet"));
            }
            let sprite = object
                .sprite
                .as_mut()
                .ok_or_else(|| invalid("Set a sprite first"))?;
            sprite.sheet = sheet.map(|sheet| futty_runtime::SpriteSheet {
                columns: sheet.columns,
                rows: sheet.rows,
                first_frame: sheet.first_frame,
                frames: sheet.frames,
                fps: sheet.fps,
                looping: sheet.looping,
            });
        }
        Operation::SetSprite2d {
            object,
            texture_id,
            size,
            layer,
            pivot,
            flip,
        } => {
            let index = object_index(&object, scene, keys)?;
            if !scene
                .assets
                .iter()
                .any(|asset| asset.id == texture_id && asset.kind == AssetKind::Texture)
            {
                return Err(invalid("Sprite2D requires a registered PNG texture"));
            }
            let object = &mut scene.entities[index];
            set_sprite(
                object,
                texture_id,
                size,
                Some(futty_runtime::Billboard::Fixed),
            )?;
            let planar = object
                .sprite
                .as_mut()
                .unwrap()
                .planar
                .get_or_insert_with(Default::default);
            planar.layer = layer.unwrap_or(planar.layer);
            planar.pivot = pivot.unwrap_or(planar.pivot);
            planar.flip = flip.unwrap_or(planar.flip);
        }
        Operation::SetCameraFollow {
            object,
            tag,
            offset,
            axes,
            bounds,
        } => {
            let index = object_index(&object, scene, keys)?;
            let object = &mut scene.entities[index];
            if object.prefab.is_some() {
                return Err(invalid("Edit the prefab source to change camera follow"));
            }
            if tag.is_some()
                && object
                    .camera
                    .is_none_or(|lens| lens.projection != futty_core::Projection::Orthographic)
            {
                return Err(invalid("Camera follow requires an orthographic camera"));
            }
            object.camera_follow = tag.map(|tag| futty_runtime::CameraFollow2d {
                tag,
                offset: offset.unwrap_or([0.0; 2]),
                axes: axes.unwrap_or([true; 2]),
                bounds,
            });
        }
        Operation::SetCameraPixelGrid {
            object,
            pixels_per_unit,
        } => {
            let index = object_index(&object, scene, keys)?;
            let object = &mut scene.entities[index];
            if object.prefab.is_some() {
                return Err(invalid("Edit the prefab source to change the camera"));
            }
            let lens = object
                .camera
                .as_mut()
                .ok_or_else(|| invalid("Choose a camera object"))?;
            if pixels_per_unit.is_some() && lens.projection != futty_core::Projection::Orthographic
            {
                return Err(invalid("Pixel grid requires an orthographic camera"));
            }
            if pixels_per_unit.is_some_and(|pixels| !(1..=1024).contains(&pixels)) {
                return Err(invalid("Pixels per unit must be between 1 and 1024"));
            }
            lens.pixels_per_unit = pixels_per_unit;
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

pub(crate) fn attach_asset(
    object: &mut Object,
    asset_id: Uuid,
    kind: AssetKind,
) -> Result<(), ErrorData> {
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
        AssetKind::Script => crate::scripts::assign(object, Some(asset_id)),
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
    Ok(())
}

pub(crate) fn set_sprite(
    object: &mut Object,
    texture_id: Uuid,
    size: Option<[f32; 2]>,
    billboard: Option<futty_runtime::Billboard>,
) -> Result<(), ErrorData> {
    if !object.has_renderer()
        || object.mesh.is_some()
        || object.material.is_some()
        || object.prefab.is_some()
    {
        return Err(invalid(
            "Choose a primitive object or the source of an existing Sprite3D",
        ));
    }
    let current = object.sprite.unwrap_or(futty_runtime::Sprite {
        texture: texture_id,
        size: Vec2::ONE,
        billboard: futty_runtime::Billboard::Fixed,
        sheet: None,
        planar: None,
    });
    object.sprite = Some(futty_runtime::Sprite {
        texture: texture_id,
        size: size.map_or(current.size, Into::into),
        billboard: billboard.unwrap_or(current.billboard),
        sheet: current.sheet,
        planar: current.planar,
    });
    Ok(())
}

pub(crate) fn register(
    keys: &mut BTreeMap<String, Uuid>,
    key: String,
    id: Uuid,
) -> Result<(), ErrorData> {
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
