use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    error::Error,
    fs,
    io::Write,
    path::Path,
};

use bevy_ecs::{
    prelude::{Component, Entity, World},
    query::QueryState,
};
use futty_core::{Asset, AssetKind, CameraLens, CameraView, Light, Primitive, Surface, Transform};
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

mod audio;
mod character;
mod health;
mod hud;
mod input;
mod prefab;
pub mod project;
mod range;
mod script;
mod sprite;
pub use audio::{AudioCue, AudioListener, AudioSource, AudioStart};
pub use character::{CharacterController, CharacterMode};
pub use health::{Health, HealthState};
pub use hud::{HudAnchor, HudKind, HudWidget, MAX_HUD_WIDGETS, range_hud_value};
pub use input::InputProfile;
pub use prefab::{Prefab, PrefabLink};
pub use range::{MAGAZINE, ROUND_SECONDS, RangeConfig, RangeInput, RangeStatus};

pub use physics::{
    Body, CollisionLayers, Motion, PhysicsEvent, RayHit, STEP as FIXED_STEP_SECONDS,
};
use physics::{MAX_STEPS, Physics, STEP};
pub use script::{
    ActionState, GameInput, InputAction, MAX_SCRIPT_BYTES, ScriptDiagnostic, ScriptFields,
    ScriptValue, inspect_script, inspect_script_diagnostic,
};
use script::{Callback, MAX_WORLD_COMMANDS, Scripts, WorldCommand};
pub use sprite::{Billboard, Sprite, SpriteSheet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GameOutcome {
    Success,
    Failure,
}

type SceneResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Serialize)]
pub struct SceneIssue {
    pub field: &'static str,
    pub object_id: Option<Uuid>,
    pub message: String,
}

impl std::fmt::Display for SceneIssue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SceneIssue {}

fn scene_issue(
    field: &'static str,
    object_id: Option<Uuid>,
    message: impl Into<String>,
) -> Box<dyn Error> {
    Box::new(SceneIssue {
        field,
        object_id,
        message: message.into(),
    })
}

pub fn valid_scene_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.trim() == id && !id.chars().any(char::is_control)
}
const FORMAT_VERSION: u32 = 19;
const MAX_AUDIO_CUES: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Animation {
    pub clip: u32,
    pub speed: f32,
    pub looping: bool,
}

impl Default for Animation {
    fn default() -> Self {
        Self {
            clip: 0,
            speed: 1.0,
            looping: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Interaction {
    pub prompt: String,
    pub range: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraFollow2d {
    pub tag: String,
    pub offset: [f32; 2],
    pub axes: [bool; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<[[f32; 2]; 2]>,
}

impl Default for Interaction {
    fn default() -> Self {
        Self {
            prompt: "Interact".into(),
            range: 2.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Object {
    pub uuid: Uuid,
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub transform: Transform,
    pub color: [f32; 3],
    pub spin: bool,
    #[serde(default)]
    pub parent: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefab: Option<PrefabLink>,
    #[serde(default)]
    pub mesh: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<Animation>,
    #[serde(default, skip_serializing_if = "Primitive::is_cube")]
    pub primitive: Primitive,
    #[serde(default)]
    pub texture: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<Uuid>,
    #[serde(default)]
    pub sprite: Option<Sprite>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<CameraLens>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_follow: Option<CameraFollow2d>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<Light>,
    #[serde(default)]
    pub body: Option<Body>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<CharacterController>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction: Option<Interaction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<Health>,
    #[serde(default)]
    pub script: Option<Uuid>,
    #[serde(default)]
    pub script_fields: ScriptFields,
    #[serde(default)]
    pub audio_source: Option<AudioSource>,
    #[serde(default)]
    pub audio_listener: Option<AudioListener>,
}

impl Object {
    pub fn has_renderer(&self) -> bool {
        self.camera.is_none() && self.light.is_none()
    }

    pub fn cube(name: String, translation: Vec3) -> Self {
        Self {
            uuid: Uuid::new_v4(),
            name,
            tags: Vec::new(),
            transform: Transform {
                translation,
                ..Default::default()
            },
            color: [0.42; 3],
            spin: true,
            parent: None,
            prefab: None,
            mesh: None,
            animation: None,
            primitive: Primitive::Cube,
            texture: None,
            material: None,
            sprite: None,
            camera: None,
            camera_follow: None,
            light: None,
            body: None,
            character: None,
            interaction: None,
            health: None,
            script: None,
            script_fields: ScriptFields::new(),
            audio_source: None,
            audio_listener: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scene {
    format_version: u32,
    pub scene_uuid: Uuid,
    pub name: String,
    pub entities: Vec<Object>,
    #[serde(default)]
    pub assets: Vec<Asset>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub materials: BTreeMap<Uuid, Material>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub prefabs: BTreeMap<Uuid, Prefab>,
    #[serde(default)]
    pub script_sources: BTreeMap<Uuid, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main_camera: Option<Uuid>,
    #[serde(default)]
    pub sky: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub training_range: Option<RangeConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hud: Vec<HudWidget>,
    #[serde(default, skip_serializing_if = "InputProfile::is_empty")]
    pub input: InputProfile,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub local_coop: bool,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            scene_uuid: Uuid::new_v4(),
            name: "Main".into(),
            entities: vec![Object::cube("Cube".into(), Vec3::Y * 0.65)],
            assets: Vec::new(),
            materials: BTreeMap::new(),
            prefabs: BTreeMap::new(),
            script_sources: BTreeMap::new(),
            main_camera: None,
            sky: true,
            training_range: None,
            hud: Vec::new(),
            input: InputProfile::default(),
            local_coop: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub color: [f32; 3],
    pub texture: Option<Uuid>,
    pub surface: Surface,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            name: "Material".into(),
            color: [0.5; 3],
            texture: None,
            surface: Surface::default(),
        }
    }
}

impl Scene {
    pub fn local_player_two(&self) -> Option<usize> {
        if !self.local_coop || self.training_range.is_some() {
            return None;
        }
        let first = self.player_index()?;
        self.entities
            .iter()
            .enumerate()
            .position(|(index, object)| index != first && object.character.is_some())
    }

    pub fn player_index(&self) -> Option<usize> {
        self.camera_index(None)
            .filter(|&index| self.entities[index].character.is_some())
            .or_else(|| {
                self.entities
                    .iter()
                    .position(|object| object.character.is_some())
            })
    }

    pub fn camera_index(&self, preferred: Option<Uuid>) -> Option<usize> {
        let by_id = |id| {
            self.entities
                .iter()
                .position(|object| object.uuid == id && object.camera.is_some())
        };
        preferred
            .and_then(by_id)
            .or_else(|| self.main_camera.and_then(by_id))
            .or_else(|| {
                self.entities
                    .iter()
                    .position(|object| object.camera.is_some())
            })
    }

    pub fn camera_view(
        &self,
        index: usize,
        hierarchy: &Hierarchy,
        worlds: &[Mat4],
        local: &[Transform],
    ) -> Option<CameraView> {
        let lens = self.entities[index].camera?;
        let mut eye = worlds[index].w_axis.truncate();
        // shortcut: follow the first tagged scene object, add runtime target selection if gameplay needs it
        if lens.projection == futty_core::Projection::Orthographic
            && let Some(follow) = &self.entities[index].camera_follow
            && let Some((target, _)) = self
                .entities
                .iter()
                .enumerate()
                .find(|(target, object)| *target != index && object.tags.contains(&follow.tag))
        {
            let point = worlds[target].w_axis.truncate();
            for axis in 0..2 {
                if follow.axes[axis] && point[axis].is_finite() {
                    eye[axis] = point[axis] + follow.offset[axis];
                    if let Some([min, max]) = follow.bounds {
                        eye[axis] = eye[axis].clamp(min[axis], max[axis]);
                    }
                }
            }
        }
        let rotation = hierarchy.world_rotation(index, |index| local[index].rotation);
        (lens.valid() && eye.is_finite() && rotation.is_normalized()).then_some(CameraView {
            eye,
            rotation,
            lens,
        })
    }

    fn validate(&self) -> SceneResult<()> {
        if self.format_version != FORMAT_VERSION {
            return Err(format!("Unsupported scene version {}", self.format_version).into());
        }
        if self.scene_uuid.is_nil() {
            return Err("Missing scene UUID".into());
        }
        self.input
            .validate()
            .map_err(|message| scene_issue("input", None, message))?;
        if self.main_camera.is_some_and(|id| {
            !self
                .entities
                .iter()
                .any(|object| object.uuid == id && object.camera.is_some())
        }) {
            return Err(scene_issue(
                "main_camera",
                self.main_camera,
                "Main Camera must reference a camera object in the scene",
            ));
        }
        if let Some(range) = &self.training_range {
            range.validate(self)?;
        }
        let mut assets = HashMap::new();
        for asset in &self.assets {
            if asset.id.is_nil() || assets.insert(asset.id, asset.kind).is_some() {
                return Err(format!("Missing or duplicate asset UUID: {}", asset.id).into());
            }
        }
        let mut widgets = HashSet::new();
        if self.hud.len() > MAX_HUD_WIDGETS
            || self.hud.iter().any(|widget| {
                !widgets.insert(widget.id) || !widget.valid(|id| assets.get(&id).copied())
            })
        {
            return Err("Invalid HUD widget or missing HUD image".into());
        }
        for (id, source) in &self.script_sources {
            if assets.get(id) != Some(&AssetKind::Script) || source.len() > MAX_SCRIPT_BYTES {
                return Err(format!("Invalid script source: {id}").into());
            }
        }
        for (id, material) in &self.materials {
            if id.is_nil()
                || assets.contains_key(id)
                || material.name.trim().is_empty()
                || material.name.len() > 256
                || !material
                    .color
                    .iter()
                    .all(|value| (0.0..=1.0).contains(value))
                || !material.surface.valid()
                || material
                    .texture
                    .is_some_and(|id| assets.get(&id) != Some(&AssetKind::Texture))
            {
                return Err(format!("Invalid material: {}", material.name).into());
            }
        }
        self.validate_objects(&self.entities, &assets)?;
        for (id, prefab) in &self.prefabs {
            if id.is_nil()
                || assets.contains_key(id)
                || self.materials.contains_key(id)
                || prefab.name.trim().is_empty()
                || prefab.name.len() > 256
                || prefab.entities.is_empty()
                || prefab.entities[0].parent.is_some()
                || prefab
                    .source
                    .is_some_and(|source| source != prefab.entities[0].uuid)
                || prefab
                    .entities
                    .iter()
                    .skip(1)
                    .any(|object| object.parent.is_none())
                || prefab.entities.iter().any(|object| object.prefab.is_some())
            {
                return Err(format!("Invalid prefab: {}", prefab.name).into());
            }
            self.validate_objects(&prefab.entities, &assets)
                .map_err(|error| match error.downcast::<SceneIssue>() {
                    Ok(mut issue) => {
                        issue.message = format!("Prefab {}: {}", prefab.name, issue.message);
                        issue as Box<dyn Error>
                    }
                    Err(error) => format!("Prefab {}: {error}", prefab.name).into(),
                })?;
        }
        self.validate_prefab_links()
    }

    fn validate_objects(
        &self,
        objects: &[Object],
        assets: &HashMap<Uuid, AssetKind>,
    ) -> SceneResult<()> {
        let mut ids = HashSet::new();
        let mut listener = false;
        for object in objects {
            if object.camera_follow.as_ref().is_some_and(|follow| {
                object.camera.is_none()
                    || !hud::valid_binding(&follow.tag)
                    || follow
                        .offset
                        .iter()
                        .any(|value| !(-100_000.0..=100_000.0).contains(value))
                    || !follow.axes.into_iter().any(|axis| axis)
                    || follow.bounds.is_some_and(|[min, max]| {
                        (0..2).any(|axis| {
                            !(-100_000.0..=100_000.0).contains(&min[axis])
                                || !(min[axis]..=100_000.0).contains(&max[axis])
                        })
                    })
            }) {
                return Err(scene_issue(
                    "camera_follow",
                    Some(object.uuid),
                    "Invalid orthographic camera follow settings",
                ));
            }
            if object
                .health
                .is_some_and(|health| !(0.01..=1_000_000.0).contains(&health.max))
            {
                return Err(scene_issue(
                    "health",
                    Some(object.uuid),
                    "Maximum health must be in 0.01–1000000",
                ));
            }
            if object.interaction.as_ref().is_some_and(|interaction| {
                !(0.1..=25.0).contains(&interaction.range)
                    || interaction.prompt.len() > 128
                    || interaction.prompt.chars().any(char::is_control)
            }) {
                return Err(scene_issue(
                    "interaction",
                    Some(object.uuid),
                    "Invalid interaction range or prompt",
                ));
            }
            if object.uuid.is_nil() || !ids.insert(object.uuid) {
                return Err(scene_issue(
                    "uuid",
                    Some(object.uuid),
                    format!("Missing or duplicate object UUID: {}", object.uuid),
                ));
            }
            let mut tags = HashSet::new();
            if object.tags.len() > 32
                || object.tags.iter().any(|tag| {
                    tag.is_empty()
                        || tag.len() > 64
                        || tag.trim() != tag
                        || tag.chars().any(char::is_control)
                        || !tags.insert(tag)
                })
            {
                return Err(format!("Invalid or duplicate tags on {}", object.name).into());
            }
            let transform = object.transform;
            if object
                .material
                .is_some_and(|id| !self.materials.contains_key(&id))
            {
                return Err(scene_issue(
                    "material",
                    Some(object.uuid),
                    format!("Missing material on {}", object.name),
                ));
            }
            if !transform.translation.is_finite()
                || !transform.rotation.is_normalized()
                || !transform.scale.is_finite()
                || transform.scale.min_element() < 0.01
                || !object.color.iter().all(|value| (0.0..=1.0).contains(value))
            {
                return Err(format!("Invalid transform or color on {}", object.name).into());
            }
            for (field, id, kind) in [
                ("mesh", object.mesh, AssetKind::Model),
                ("texture", object.texture, AssetKind::Texture),
                (
                    "sprite.texture",
                    object.sprite.map(|sprite| sprite.texture),
                    AssetKind::Texture,
                ),
                ("script", object.script, AssetKind::Script),
            ] {
                if id.is_some_and(|id| assets.get(&id) != Some(&kind)) {
                    return Err(scene_issue(
                        field,
                        Some(object.uuid),
                        format!("Invalid asset reference on {}", object.name),
                    ));
                }
            }
            if object.animation.is_some_and(|animation| {
                object.mesh.is_none()
                    || !object.has_renderer()
                    || object.sprite.is_some()
                    || animation.clip >= 256
                    || !(0.05..=8.0).contains(&animation.speed)
            }) {
                return Err(format!("Invalid model animation on {}", object.name).into());
            }
            if object.sprite.is_some_and(|sprite| !sprite.valid()) {
                return Err(format!("Invalid Sprite3D size on {}", object.name).into());
            }
            if object.camera.is_some_and(|lens| !lens.valid()) {
                return Err(scene_issue(
                    "camera",
                    Some(object.uuid),
                    format!("Invalid camera settings on {}", object.name),
                ));
            }
            if object.light.is_some_and(|light| !light.valid()) {
                return Err(format!("Invalid light settings on {}", object.name).into());
            }
            if object.script_fields.len() > 64
                || object
                    .script_fields
                    .iter()
                    .any(|(name, value)| name.is_empty() || name.len() > 64 || !value.valid())
            {
                return Err(format!("Invalid script field on {}", object.name).into());
            }
            if let Some(body) = object.body {
                if object.parent.is_some() {
                    return Err(
                        format!("Physics body {} must be at the scene root", object.name).into(),
                    );
                }
                if !body.valid(transform) {
                    return Err(format!("Invalid physics settings on {}", object.name).into());
                }
            }
            if let Some(character) = object.character
                && (object.parent.is_some() || object.body.is_some() || !character.valid())
            {
                return Err(format!(
                    "Invalid character controller on {}. Use a root object without a rigid body.",
                    object.name
                )
                .into());
            }
            if let Some(source) = object.audio_source
                && (!assets.get(&source.clip).is_some_and(|kind| kind.is_audio())
                    || !(0.0..=1.0).contains(&source.volume))
            {
                return Err(format!("Invalid audio source on {}", object.name).into());
            }
            if let Some(config) = object.audio_listener {
                if listener || !(0.0..=1.0).contains(&config.volume) {
                    return Err(
                        format!("Invalid or duplicate audio listener on {}", object.name).into(),
                    );
                }
                listener = true;
            }
        }
        Hierarchy::new(objects)?;
        Ok(())
    }

    pub fn load(path: &Path) -> SceneResult<Self> {
        Self::decode(&fs::read_to_string(path)?)
    }

    pub fn decode(source: &str) -> SceneResult<Self> {
        let mut scene: Self = ron::from_str(source)?;
        if (1..FORMAT_VERSION).contains(&scene.format_version) {
            scene.format_version = FORMAT_VERSION;
        }
        scene.validate()?;
        Ok(scene)
    }

    pub fn save(&self, path: &Path) -> SceneResult<()> {
        self.write_scene(path, true)
    }

    pub fn save_new(&self, path: &Path) -> SceneResult<()> {
        self.write_scene(path, false)
    }

    fn write_scene(&self, path: &Path, replace: bool) -> SceneResult<()> {
        self.validate()?;
        let encoded = ron::ser::to_string_pretty(self, Default::default())?;
        let directory = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temp = tempfile::NamedTempFile::new_in(directory)?;
        temp.write_all(encoded.as_bytes())?;
        temp.as_file().sync_all()?;
        if replace {
            temp.persist(path)?;
        } else {
            temp.persist_noclobber(path)?;
        }
        Ok(())
    }

    pub fn hierarchy(&self) -> SceneResult<Hierarchy> {
        Hierarchy::new(&self.entities)
    }
}

pub struct Hierarchy {
    pub parents: Vec<Option<usize>>,
    pub order: Vec<(usize, usize)>,
}

impl Hierarchy {
    pub fn new(objects: &[Object]) -> SceneResult<Self> {
        let indices: HashMap<_, _> = objects
            .iter()
            .enumerate()
            .map(|(index, object)| (object.uuid, index))
            .collect();
        let mut children = vec![Vec::new(); objects.len()];
        let mut roots = Vec::new();
        let mut parents = Vec::with_capacity(objects.len());
        for (index, object) in objects.iter().enumerate() {
            let parent = match object.parent {
                Some(id) => {
                    let &parent = indices.get(&id).ok_or_else(|| {
                        scene_issue(
                            "parent",
                            Some(object.uuid),
                            format!("Missing parent of {}", object.name),
                        )
                    })?;
                    children[parent].push(index);
                    Some(parent)
                }
                None => {
                    roots.push(index);
                    None
                }
            };
            parents.push(parent);
        }
        let mut stack: Vec<_> = roots.iter().rev().map(|&index| (index, 0)).collect();
        let mut order = Vec::with_capacity(objects.len());
        while let Some((index, depth)) = stack.pop() {
            order.push((index, depth));
            stack.extend(
                children[index]
                    .iter()
                    .rev()
                    .map(|&child| (child, depth + 1)),
            );
        }
        if order.len() != objects.len() {
            let visited: HashSet<_> = order.iter().map(|&(index, _)| index).collect();
            let blocked = objects
                .iter()
                .enumerate()
                .find(|(index, _)| !visited.contains(index));
            return Err(scene_issue(
                "parent",
                blocked.map(|(_, object)| object.uuid),
                "Scene hierarchy contains a cycle",
            ));
        }
        Ok(Hierarchy { parents, order })
    }
    pub fn world_rotation(&self, index: usize, local: impl Fn(usize) -> Quat) -> Quat {
        let mut rotation = local(index);
        let mut parent = self.parents[index];
        while let Some(index) = parent {
            rotation = local(index) * rotation;
            parent = self.parents[index];
        }
        rotation.normalize()
    }

    pub fn world_matrices(&self, local: impl Fn(usize) -> Mat4) -> Vec<Mat4> {
        let mut matrices = vec![Mat4::IDENTITY; self.parents.len()];
        for &(index, _) in &self.order {
            let parent = self.parents[index].map_or(Mat4::IDENTITY, |parent| matrices[parent]);
            matrices[index] = parent * local(index);
        }
        matrices
    }
}

#[derive(Component)]
struct RuntimeObject {
    uuid: Uuid,
    parent: Option<Uuid>,
    name: String,
    transform: Transform,
    color: [f32; 3],
    spin: bool,
    body: Option<(rapier3d::prelude::RigidBodyHandle, Motion)>,
    kinematic_target: Option<Transform>,
    script: Option<Uuid>,
    character: Option<character::CharacterMotion>,
    interaction: Option<Interaction>,
    can_animate: bool,
    animation: Option<(Animation, f64)>,
    sprite_time: Option<f64>,
}

impl RuntimeObject {
    fn new(object: &Object, physics: &mut Physics) -> Self {
        Self {
            uuid: object.uuid,
            parent: object.parent,
            name: object.name.clone(),
            transform: object.transform,
            color: object.color,
            spin: object.spin,
            body: object.body.map(|body| {
                (
                    physics.insert(object.uuid, object.transform, body),
                    body.motion,
                )
            }),
            kinematic_target: object
                .body
                .filter(|body| body.motion == Motion::Kinematic)
                .map(|_| object.transform),
            script: object.script,
            character: object.character.map(character::CharacterMotion::new),
            interaction: object.interaction.clone(),
            can_animate: object.mesh.is_some() && object.sprite.is_none() && object.has_renderer(),
            animation: object.animation.map(|animation| (animation, 0.0)),
            sprite_time: object
                .sprite
                .and_then(|sprite| sprite.sheet)
                .filter(|sheet| sheet.frames > 1)
                .map(|_| 0.0),
        }
    }

    fn run_script(
        &mut self,
        scripts: &mut Scripts,
        physics: &mut Physics,
        callback: Callback,
        world_commands: &mut Vec<WorldCommand>,
    ) {
        if scripts.inactive.borrow().contains(&self.uuid) {
            return;
        }
        if let Some(script) = self.script {
            let commands = scripts.call(
                self.uuid,
                script,
                &self.name,
                self.kinematic_target
                    .as_mut()
                    .unwrap_or(&mut self.transform),
                self.body.map(|(_, motion)| motion),
                callback,
            );
            if let Some((handle, Motion::Dynamic)) = self.body {
                physics.apply_commands(handle, &commands);
            }
            if let Some((clip, looping)) = commands.animation
                && self.can_animate
            {
                let mut animation = self
                    .animation
                    .map_or_else(Animation::default, |(animation, _)| animation);
                animation.clip = clip;
                animation.looping = looping;
                self.animation = Some((animation, 0.0));
            }
            let available = MAX_WORLD_COMMANDS.saturating_sub(world_commands.len());
            world_commands.extend(commands.world.into_iter().take(available));
        }
    }
}

pub struct Runtime {
    world: World,
    objects: QueryState<&'static mut RuntimeObject>,
    physics: Physics,
    accumulated: f32,
    inactive: std::rc::Rc<std::cell::RefCell<HashSet<Uuid>>>,
    scripts: Option<Scripts>,
    audio_sources: HashMap<Uuid, AudioSource>,
    audio_cues: VecDeque<AudioCue>,
    audio_gain: f32,
    range: Option<range::Range>,
    input: GameInput,
    player_inputs: HashMap<Uuid, std::rc::Rc<GameInput>>,
    secondary_character: Option<Uuid>,
    primary_character: Option<Uuid>,
    commands: Vec<WorldCommand>,
    prefabs: BTreeMap<Uuid, Prefab>,
    spawned: Vec<Object>,
    branches: HashMap<Uuid, Vec<(Uuid, Entity)>>,
    stopped_audio: Vec<Uuid>,
    hud_values: BTreeMap<String, ScriptValue>,
    progress: std::rc::Rc<ScriptFields>,
    checkpoints: HashMap<Uuid, Checkpoint>,
    health: health::HealthStates,
    scene_request: Option<String>,
    outcome: Option<GameOutcome>,
}

struct Checkpoint {
    transform: Transform,
    progress: std::rc::Rc<ScriptFields>,
    hud: BTreeMap<String, ScriptValue>,
    health: Option<HealthState>,
}

impl Runtime {
    pub fn new(scene: &Scene) -> SceneResult<Self> {
        scene.validate()?;
        let mut world = World::new();
        let mut physics = Physics::new();
        for object in &scene.entities {
            world.spawn(RuntimeObject::new(object, &mut physics));
        }
        let objects = world.query();
        let audio_cues = scene
            .entities
            .iter()
            .filter_map(|object| {
                object
                    .audio_source
                    .filter(|source| source.start == AudioStart::OnPlay)
                    .map(|source| AudioCue {
                        object: object.uuid,
                        source,
                    })
            })
            .take(MAX_AUDIO_CUES)
            .collect();
        let audio_sources = scene
            .entities
            .iter()
            .filter_map(|object| object.audio_source.map(|source| (object.uuid, source)))
            .collect();
        let audio_gain = scene
            .entities
            .iter()
            .find_map(|object| object.audio_listener)
            .map_or(1.0, |listener| listener.volume);
        Ok(Self {
            world,
            objects,
            physics,
            accumulated: 0.0,
            inactive: Default::default(),
            scripts: None,
            audio_sources,
            audio_cues,
            audio_gain,
            range: scene
                .training_range
                .as_ref()
                .map(|config| range::Range::new(scene, config))
                .transpose()?,
            input: GameInput::default(),
            player_inputs: HashMap::new(),
            secondary_character: scene
                .local_player_two()
                .map(|index| scene.entities[index].uuid),
            primary_character: scene.player_index().map(|index| scene.entities[index].uuid),
            commands: Vec::new(),
            prefabs: scene.prefabs.clone(),
            spawned: Vec::new(),
            branches: HashMap::new(),
            stopped_audio: Vec::new(),
            hud_values: BTreeMap::new(),
            progress: Default::default(),
            checkpoints: HashMap::new(),
            health: std::rc::Rc::new(std::cell::RefCell::new(
                scene
                    .entities
                    .iter()
                    .filter_map(|object| object.health.map(|health| (object.uuid, health.into())))
                    .collect(),
            )),
            scene_request: None,
            outcome: None,
        })
    }

    pub fn start_scripts(&mut self, scene: &Scene, sources: &HashMap<Uuid, String>) {
        self.start_scripts_with_seed(scene, sources, None);
    }

    pub fn game_state(&self) -> &ScriptFields {
        &self.progress
    }

    /// Use before starting the destination scene's scripts.
    pub fn with_game_state(mut self, state: ScriptFields) -> Self {
        self.progress = state.into();
        self
    }

    pub fn start_seeded_scripts(
        &mut self,
        scene: &Scene,
        sources: &HashMap<Uuid, String>,
        seed: u32,
    ) {
        self.start_scripts_with_seed(scene, sources, Some(seed));
    }

    fn start_scripts_with_seed(
        &mut self,
        scene: &Scene,
        sources: &HashMap<Uuid, String>,
        seed: Option<u32>,
    ) {
        let referenced: HashSet<_> = scene
            .referenced_objects()
            .filter_map(|object| object.script)
            .collect();
        if referenced.is_empty() || self.scripts.is_some() {
            return;
        }
        let ids = seed.map(|seed| {
            script::SpawnIds::new(seed, scene.entities.iter().map(|object| object.uuid))
        });
        let mut scripts = Scripts::new(ids, self.health.clone());
        scripts.progress = self.progress.clone();
        scripts.input = self.input.clone().into();
        scripts.player_inputs.clone_from(&self.player_inputs);
        self.inactive = scripts.inactive.clone();
        self.refresh_query_poses(&scripts.poses);
        for (&id, prefab) in &self.prefabs {
            scripts
                .prefabs
                .borrow_mut()
                .entry(prefab.name.clone())
                .or_insert(id);
        }
        {
            let mut tags = scripts.tags.borrow_mut();
            for object in &scene.entities {
                for tag in &object.tags {
                    tags.entry(tag.clone()).or_default().push(object.uuid);
                }
            }
        }
        for asset in scene
            .assets
            .iter()
            .filter(|asset| referenced.contains(&asset.id))
        {
            scripts.compile(
                asset.id,
                &asset.name,
                scene
                    .script_sources
                    .get(&asset.id)
                    .or_else(|| sources.get(&asset.id))
                    .map(String::as_str),
            );
        }
        for object in &scene.entities {
            if let Some(asset) = object.script {
                scripts.bind(object.uuid, asset, &object.script_fields);
            }
        }
        for mut object in self.objects.iter_mut(&mut self.world) {
            object.run_script(
                &mut scripts,
                &mut self.physics,
                Callback::Start,
                &mut self.commands,
            );
        }
        self.scripts = Some(scripts);
    }

    pub fn game_input(&mut self, input: GameInput) {
        self.player_inputs.clear();
        self.input = input;
        if let Some(scripts) = &mut self.scripts {
            scripts.input = self.input.clone().into();
            scripts.player_inputs.clear();
        }
    }

    pub fn local_game_input(&mut self, first: GameInput, second: GameInput) {
        if let (Some(primary), Some(secondary)) = (self.primary_character, self.secondary_character)
        {
            self.game_input(GameInput::default());
            self.game_input_for(primary, first);
            self.game_input_for(secondary, second);
        } else {
            self.game_input(first);
        }
    }

    /// Supply input for one character. Unassigned players receive neutral input.
    pub fn game_input_for(&mut self, id: Uuid, input: GameInput) -> bool {
        if !self
            .objects
            .iter(&self.world)
            .any(|object| object.uuid == id && object.character.is_some())
        {
            return false;
        }
        if self.player_inputs.is_empty() {
            self.game_input(GameInput::default());
        }
        let input = std::rc::Rc::new(input);
        self.player_inputs.insert(id, input.clone());
        if let Some(scripts) = &mut self.scripts {
            scripts.player_inputs.insert(id, input);
        }
        true
    }

    /// Focused target and prompt for an active character looking at an interactable.
    pub fn interaction_focus(&mut self, player: Uuid) -> Option<(Uuid, String)> {
        if self
            .health_state(player)
            .is_some_and(|health| !health.alive())
        {
            return None;
        }
        let inactive = self.inactive.borrow();
        if inactive.contains(&player)
            || !self
                .objects
                .iter(&self.world)
                .any(|o| o.interaction.is_some())
        {
            return None;
        }
        let poses: script::QueryPoses = self
            .objects
            .iter(&self.world)
            .map(|object| (object.uuid, (object.parent, object.transform)))
            .collect();
        let origin = script::world_position(&poses, player)?;
        let actor = self
            .objects
            .iter(&self.world)
            .find(|o| o.uuid == player && o.character.is_some())?;
        let forward = actor.transform.rotation * Vec3::NEG_Z;
        let directional = actor
            .character
            .as_ref()
            .is_some_and(character::CharacterMotion::is_spatial);
        self.objects
            .iter(&self.world)
            .filter_map(|object| {
                let interaction = object.interaction.as_ref()?;
                if object.uuid == player || inactive.contains(&object.uuid) {
                    return None;
                }
                let delta = script::world_position(&poses, object.uuid)? - origin;
                let distance = delta.length();
                if distance > interaction.range
                    || (directional && delta.try_normalize()?.dot(forward) < 0.866)
                {
                    return None;
                }
                if self
                    .physics
                    .raycast(origin, delta, distance, u32::MAX)
                    .is_some_and(|hit| hit.entity != object.uuid)
                {
                    return None;
                }
                Some((distance, object.uuid, &interaction.prompt))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
            .map(|(_, id, prompt)| (id, prompt.clone()))
    }

    pub fn prepare_play(&mut self) {
        self.apply_world_commands();
        self.physics.prepare_queries();
    }

    pub fn script_log(&self) -> impl DoubleEndedIterator<Item = &str> {
        self.scripts
            .iter()
            .flat_map(|scripts| scripts.log.iter().map(String::as_str))
    }

    pub fn take_scene_request(&mut self) -> Option<String> {
        self.scene_request.take()
    }

    pub fn game_outcome(&self) -> Option<GameOutcome> {
        self.outcome
    }

    pub fn look_character(&mut self, id: Uuid, yaw_degrees: f32, pitch_degrees: f32) -> bool {
        if !yaw_degrees.is_finite() || !pitch_degrees.is_finite() {
            return false;
        }
        let Some(mut object) = self.objects.iter_mut(&mut self.world).find(|object| {
            object.uuid == id
                && object
                    .character
                    .as_ref()
                    .is_some_and(character::CharacterMotion::is_spatial)
        }) else {
            return false;
        };
        let forward = object.transform.rotation * Vec3::NEG_Z;
        let yaw = forward.x.atan2(-forward.z) + yaw_degrees.to_radians();
        let limit = 89.0_f32.to_radians();
        let pitch =
            (forward.y.clamp(-1.0, 1.0).asin() + pitch_degrees.to_radians()).clamp(-limit, limit);
        object.transform.rotation =
            (Quat::from_rotation_y(-yaw) * Quat::from_rotation_x(pitch)).normalize();
        true
    }

    pub fn log_scene_error(&mut self, message: String) {
        if let Some(scripts) = &mut self.scripts {
            scripts.push(message);
        }
    }

    pub fn audio_gain(&self) -> f32 {
        self.audio_gain
    }

    pub fn range_status(&self) -> Option<RangeStatus> {
        self.range.as_ref().map(range::Range::status)
    }

    pub fn hud_value(&self, name: &str) -> Option<ScriptValue> {
        self.range_status()
            .and_then(|status| range_hud_value(name, Some(status)))
            .or_else(|| {
                let id = match name {
                    "player.health" => self.primary_character,
                    "player2.health" => self.secondary_character,
                    _ => None,
                }?;
                self.health_state(id)
                    .map(|health| ScriptValue::Number(f64::from(health.current / health.max)))
            })
            .or_else(|| self.hud_values.get(name).cloned())
    }

    pub fn click_hud(&mut self, name: &str) {
        if let Some(scripts) = &mut self.scripts
            && hud::valid_binding(name)
        {
            let mut clicks = scripts.hud_clicks.borrow_mut();
            if clicks.len() < MAX_HUD_WIDGETS {
                clicks.insert(name.to_owned());
            }
        }
    }

    pub fn drain_audio(&mut self) -> impl Iterator<Item = AudioCue> + '_ {
        self.audio_cues.drain(..)
    }

    pub fn drain_stopped_audio(&mut self) -> impl Iterator<Item = Uuid> + '_ {
        self.stopped_audio.drain(..)
    }

    pub fn spawned_objects(&self) -> &[Object] {
        &self.spawned
    }

    pub fn is_active(&self, id: Uuid) -> bool {
        !self.inactive.borrow().contains(&id)
    }

    pub fn health_state(&self, id: Uuid) -> Option<HealthState> {
        self.health.borrow().get(&id).copied()
    }

    pub fn inactive_objects(&self) -> std::cell::Ref<'_, HashSet<Uuid>> {
        self.inactive.borrow()
    }

    fn refresh_query_poses(&mut self, poses: &std::cell::RefCell<script::QueryPoses>) {
        let mut poses = poses.borrow_mut();
        poses.clear();
        poses.extend(
            self.objects
                .iter(&self.world)
                .map(|object| (object.uuid, (object.parent, object.transform))),
        );
    }

    fn apply_world_commands(&mut self) {
        if self.commands.is_empty() {
            return;
        }
        let Some(mut scripts) = self.scripts.take() else {
            return;
        };
        for command in std::mem::take(&mut self.commands) {
            match command {
                WorldCommand::HealthChange {
                    target,
                    source,
                    delta,
                } => {
                    self.change_health(&mut scripts, target, source, delta);
                }
                WorldCommand::Interact { actor, target } => {
                    if self.is_active(actor)
                        && self.health_state(actor).is_none_or(HealthState::alive)
                        && let Some(mut object) = self
                            .objects
                            .iter_mut(&mut self.world)
                            .find(|object| object.uuid == target)
                    {
                        object.run_script(
                            &mut scripts,
                            &mut self.physics,
                            Callback::Event("on_interact", actor),
                            &mut self.commands,
                        );
                    }
                }
                WorldCommand::Message(message) => {
                    if let Some(mut target) = self
                        .objects
                        .iter_mut(&mut self.world)
                        .find(|object| object.uuid == message.target)
                    {
                        target.run_script(
                            &mut scripts,
                            &mut self.physics,
                            Callback::Message(message.sender, message.name),
                            &mut self.commands,
                        );
                    }
                }
                WorldCommand::Spawn {
                    prefab,
                    root,
                    position,
                } => self.spawn_prefab(&mut scripts, prefab, root, position),
                WorldCommand::Despawn(root) => self.despawn_prefab(&mut scripts, root),
                WorldCommand::SetActive(id, active) => {
                    if let Some(object) = self
                        .objects
                        .iter(&self.world)
                        .find(|object| object.uuid == id)
                    {
                        if active {
                            self.inactive.borrow_mut().remove(&id);
                        } else if self.inactive.borrow_mut().insert(id) {
                            self.audio_cues.retain(|cue| cue.object != id);
                            if self.audio_sources.contains_key(&id) {
                                self.stopped_audio.push(id);
                            }
                        }
                        if let Some((body, _)) = object.body {
                            self.physics.set_enabled(body, active);
                        }
                    }
                }
                WorldCommand::HudSet { name, value } => {
                    if self.hud_values.len() < 128 || self.hud_values.contains_key(&name) {
                        self.hud_values.insert(name, value);
                    }
                }
                WorldCommand::GameSet(key, value) => {
                    let progress = std::rc::Rc::make_mut(&mut self.progress);
                    if let Some(value) = value {
                        if progress.len() < 128 || progress.contains_key(&key) {
                            progress.insert(key, value);
                        } else {
                            scripts.push("Game state limit reached (128 keys)".into());
                        }
                    } else {
                        progress.remove(&key);
                    }
                    scripts.progress = self.progress.clone();
                }
                WorldCommand::SaveCheckpoint(id) => {
                    if self.health_state(id).is_some_and(|health| !health.alive()) {
                        continue;
                    }
                    if let Some(object) = self
                        .objects
                        .iter(&self.world)
                        .find(|object| object.uuid == id && object.character.is_some())
                    {
                        self.checkpoints.insert(
                            id,
                            Checkpoint {
                                transform: object.transform,
                                progress: self.progress.clone(),
                                hud: self.hud_values.clone(),
                                health: self.health_state(id),
                            },
                        );
                    }
                }
                WorldCommand::RestoreCheckpoint(id) => {
                    if let Some(checkpoint) = self.checkpoints.get(&id)
                        && let Some(mut object) = self
                            .objects
                            .iter_mut(&mut self.world)
                            .find(|object| object.uuid == id)
                    {
                        object.transform = checkpoint.transform;
                        if let Some(character) = &mut object.character {
                            character.reset();
                        }
                        self.inactive.borrow_mut().remove(&id);
                        self.progress = checkpoint.progress.clone();
                        scripts.progress = self.progress.clone();
                        self.hud_values.clone_from(&checkpoint.hud);
                        if let Some(health) = checkpoint.health {
                            self.health.borrow_mut().insert(id, health);
                        }
                        scripts
                            .poses
                            .borrow_mut()
                            .insert(id, (object.parent, object.transform));
                    }
                }
                WorldCommand::LoadScene(id) => {
                    self.scene_request.get_or_insert(id);
                }
                WorldCommand::FinishGame(outcome) => {
                    self.outcome.get_or_insert(outcome);
                }
            }
        }
        self.scripts = Some(scripts);
    }

    fn change_health(&mut self, scripts: &mut Scripts, target: Uuid, source: Uuid, delta: f32) {
        if !self.is_active(target) {
            return;
        }
        let (amount, died) = {
            let mut states = self.health.borrow_mut();
            let Some(state) = states.get_mut(&target).filter(|state| state.alive()) else {
                return;
            };
            let before = state.current;
            state.current = (before + delta).clamp(0.0, state.max);
            ((state.current - before).abs(), !state.alive())
        };
        if amount > 0.0
            && let Some(mut object) = self
                .objects
                .iter_mut(&mut self.world)
                .find(|object| object.uuid == target)
        {
            let event = if delta < 0.0 { "on_damage" } else { "on_heal" };
            object.run_script(
                scripts,
                &mut self.physics,
                Callback::Health(event, source, amount),
                &mut self.commands,
            );
            if died {
                object.run_script(
                    scripts,
                    &mut self.physics,
                    Callback::Event("on_death", source),
                    &mut self.commands,
                );
            }
        }
    }

    fn spawn_prefab(&mut self, scripts: &mut Scripts, id: Uuid, root: Uuid, position: Vec3) {
        let Some(prefab) = self.prefabs.get(&id) else {
            return;
        };
        if self.spawned.len() + prefab.entities.len() > 1024 {
            scripts.push("Runtime prefab limit reached (1024 objects)".into());
            return;
        }
        let copies = prefab.copies(root, position, true, || scripts.next_spawn_id());
        let mut branch = Vec::with_capacity(copies.len());
        for object in &copies {
            if let Some(health) = object.health {
                self.health.borrow_mut().insert(object.uuid, health.into());
            }
            let entity = self
                .world
                .spawn(RuntimeObject::new(object, &mut self.physics))
                .id();
            branch.push((object.uuid, entity));
            scripts
                .poses
                .borrow_mut()
                .insert(object.uuid, (object.parent, object.transform));
            for tag in &object.tags {
                scripts
                    .tags
                    .borrow_mut()
                    .entry(tag.clone())
                    .or_default()
                    .push(object.uuid);
            }
            if let Some(script) = object.script {
                scripts.bind(object.uuid, script, &object.script_fields);
            }
            if let Some(source) = object.audio_source {
                self.audio_sources.insert(object.uuid, source);
                if source.start == AudioStart::OnPlay && self.audio_cues.len() < MAX_AUDIO_CUES {
                    self.audio_cues.push_back(AudioCue {
                        object: object.uuid,
                        source,
                    });
                }
            }
        }
        for &(_, entity) in &branch {
            self.objects
                .get_mut(&mut self.world, entity)
                .expect("just spawned")
                .run_script(
                    scripts,
                    &mut self.physics,
                    Callback::Start,
                    &mut self.commands,
                );
        }
        self.branches.insert(root, branch);
        self.spawned.extend(copies);
    }

    fn despawn_prefab(&mut self, scripts: &mut Scripts, root: Uuid) {
        let Some(branch) = self.branches.remove(&root) else {
            return;
        };
        let removed: HashSet<_> = branch.iter().map(|&(id, _)| id).collect();
        for (id, entity) in branch {
            if let Some((body, _)) = self
                .world
                .get::<RuntimeObject>(entity)
                .and_then(|object| object.body)
            {
                self.physics.remove(body);
            }
            self.world.despawn(entity);
            self.checkpoints.remove(&id);
            self.health.borrow_mut().remove(&id);
            scripts.forget(id);
            if self.audio_sources.remove(&id).is_some() {
                self.stopped_audio.push(id);
            }
        }
        self.spawned
            .retain(|object| !removed.contains(&object.uuid));
        self.audio_cues.retain(|cue| !removed.contains(&cue.object));
    }

    pub fn tick(&mut self, seconds: f32) {
        if !seconds.is_finite() {
            return;
        }
        let frame = seconds.clamp(0.0, 0.1);
        if let Some(poses) = self.scripts.as_ref().map(|scripts| scripts.poses.clone()) {
            self.refresh_query_poses(&poses);
        }
        self.apply_world_commands();
        if self.scripts.is_some() {
            let mut actors: Vec<_> = self
                .objects
                .iter(&self.world)
                .filter(|object| {
                    object.character.is_some()
                        && (!self.player_inputs.is_empty()
                            || self.primary_character == Some(object.uuid))
                        && self
                            .player_inputs
                            .get(&object.uuid)
                            .map_or(&self.input, AsRef::as_ref)
                            .action(InputAction::Interact)
                            .pressed
                })
                .map(|object| object.uuid)
                .collect();
            actors.sort_unstable();
            for actor in actors {
                if self.commands.len() >= MAX_WORLD_COMMANDS {
                    break;
                }
                if let Some((target, _)) = self.interaction_focus(actor) {
                    self.commands.push(WorldCommand::Interact { actor, target });
                }
            }
        }
        let inactive = self.inactive.borrow();
        if self.physics.count() != 0 || self.scripts.is_some() {
            self.accumulated += frame;
            let requested = (self.accumulated / STEP) as usize;
            self.accumulated -= requested as f32 * STEP;
            for _ in 0..requested.min(MAX_STEPS) {
                if let Some(scripts) = &mut self.scripts {
                    scripts.tick = scripts.tick.saturating_add(1);
                    for mut object in self.objects.iter_mut(&mut self.world) {
                        object.run_script(
                            scripts,
                            &mut self.physics,
                            Callback::Fixed(STEP),
                            &mut self.commands,
                        );
                    }
                }
                for mut object in self.objects.iter_mut(&mut self.world) {
                    if inactive.contains(&object.uuid) {
                        continue;
                    }
                    let spin = object.spin;
                    if let Some((body, Motion::Kinematic)) = object.body
                        && let Some(target) = &mut object.kinematic_target
                    {
                        if spin {
                            target.rotation =
                                (Quat::from_rotation_y(STEP * 0.7) * target.rotation).normalize();
                        }
                        self.physics.move_kinematic(body, *target);
                    }
                }
                if self.physics.count() > 0 {
                    self.physics.step();
                    for event in self
                        .physics
                        .recent()
                        .iter()
                        .filter(|event| event.sensor && event.started)
                    {
                        for object in [event.first, event.second] {
                            if let Some(&source) = self.audio_sources.get(&object)
                                && !inactive.contains(&object)
                                && source.start == AudioStart::OnTriggerEnter
                                && self.audio_cues.len() < MAX_AUDIO_CUES
                            {
                                self.audio_cues.push_back(AudioCue { object, source });
                            }
                        }
                    }
                    if let Some(scripts) = &mut self.scripts {
                        for event in self.physics.recent().to_vec() {
                            let name = match (event.sensor, event.started) {
                                (true, true) => "on_trigger_enter",
                                (true, false) => "on_trigger_exit",
                                (false, true) => "on_collision_enter",
                                (false, false) => "on_collision_exit",
                            };
                            for mut object in self.objects.iter_mut(&mut self.world) {
                                if object.uuid == event.first || object.uuid == event.second {
                                    let other = if object.uuid == event.first {
                                        event.second
                                    } else {
                                        event.first
                                    };
                                    if let Some((handle, _)) = object.body {
                                        self.physics.pose(handle, &mut object.transform);
                                    }
                                    object.run_script(
                                        scripts,
                                        &mut self.physics,
                                        Callback::Event(name, other),
                                        &mut self.commands,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(range) = &mut self.range {
            for object in range
                .update(frame, &self.input, &self.physics)
                .into_iter()
                .flatten()
            {
                if let Some(&source) = self.audio_sources.get(&object)
                    && !inactive.contains(&object)
                    && source.start == AudioStart::Manual
                    && self.audio_cues.len() < MAX_AUDIO_CUES
                {
                    self.audio_cues.push_back(AudioCue { object, source });
                }
            }
        }
        let rotation = Quat::from_rotation_y(frame * 0.7);
        let neutral = GameInput::default();
        for mut object in self.objects.iter_mut(&mut self.world) {
            if inactive.contains(&object.uuid) {
                continue;
            }
            if let Some((animation, time)) = &mut object.animation {
                *time += f64::from(frame * animation.speed);
            }
            if let Some(time) = &mut object.sprite_time {
                *time += f64::from(frame);
            }
            if let Some(pose) = self
                .range
                .as_ref()
                .and_then(|range| range.pose(object.uuid))
            {
                object.transform = pose;
                continue;
            }
            if let Some((body, _)) = object.body {
                self.physics.pose(body, &mut object.transform);
            } else if object.spin && object.character.is_none() {
                object.transform.rotation = (rotation * object.transform.rotation).normalize();
            }
            if let Some(scripts) = &mut self.scripts {
                object.run_script(
                    scripts,
                    &mut self.physics,
                    Callback::Update(frame),
                    &mut self.commands,
                );
            }
            let transform = object.transform;
            let input = self
                .player_inputs
                .get(&object.uuid)
                .map(AsRef::as_ref)
                .or_else(|| {
                    (self.player_inputs.is_empty() && self.primary_character == Some(object.uuid))
                        .then_some(&self.input)
                })
                .unwrap_or(&neutral);
            if self
                .health
                .borrow()
                .get(&object.uuid)
                .is_none_or(|state| state.alive())
                && let Some(character) = &mut object.character
            {
                object.transform.translation = character.step(
                    &self.physics,
                    frame,
                    transform.translation,
                    character.direction(input, transform.rotation * Vec3::NEG_Z),
                    input.action(InputAction::Jump).pressed,
                );
            }
        }
        if let Some(scripts) = &mut self.scripts {
            scripts.hud_clicks.borrow_mut().clear();
        }
    }

    pub fn physics_body_count(&self) -> usize {
        self.physics.count()
    }

    pub fn raycast(
        &self,
        origin: Vec3,
        direction: Vec3,
        distance: f32,
        mask: u32,
    ) -> Option<RayHit> {
        self.physics.raycast(origin, direction, distance, mask)
    }

    pub fn physics_events(&self) -> impl DoubleEndedIterator<Item = &PhysicsEvent> {
        self.physics.events().iter()
    }

    pub fn recent_physics_events(&self) -> &[PhysicsEvent] {
        self.physics.recent()
    }

    pub fn extract(&mut self) -> impl Iterator<Item = (Uuid, Transform, [f32; 3])> + '_ {
        self.objects
            .iter(&self.world)
            .map(|object| (object.uuid, object.transform, object.color))
    }

    pub fn animated_objects(&mut self) -> impl Iterator<Item = (Uuid, (Animation, f64))> + '_ {
        self.objects
            .iter(&self.world)
            .filter_map(|object| object.animation.map(|animation| (object.uuid, animation)))
    }

    pub fn animated_sprites(&mut self) -> impl Iterator<Item = (Uuid, f64)> + '_ {
        self.objects
            .iter(&self.world)
            .filter_map(|object| object.sprite_time.map(|time| (object.uuid, time)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_state_crosses_scene_start_and_checkpoints_restore_progress_and_pose() {
        let mut first = Scene::default();
        let player_id = first.entities[0].uuid;
        first.entities[0].character = Some(CharacterController {
            mode: CharacterMode::TopDown2d,
            ..CharacterController::default()
        });
        let script = Uuid::new_v4();
        first.assets.push(Asset {
            id: script,
            name: "Progress".into(),
            kind: AssetKind::Script,
        });
        first.entities[0].script = Some(script);
        first.script_sources.insert(
            script,
            r#"
            fn on_start(entity) {
                game_set("score", 7);
                set_hud("score", game_get("score"));
                save_checkpoint(entity);
            }
            fn on_update(entity, dt) {
                if input_pressed("Interact") {
                    game_set("score", 99);
                    set_hud("score", 99);
                    translate(entity, 3.0, 0.0, 0.0);
                }
                if input_pressed("Jump") { restore_checkpoint(entity); }
            }
        "#
            .into(),
        );
        let mut runtime = Runtime::new(&first).unwrap();
        runtime.start_scripts(&first, &HashMap::new());
        runtime.prepare_play();
        assert_eq!(runtime.game_state()["score"], ScriptValue::Number(7.0));
        let press = |runtime: &mut Runtime, action: InputAction| {
            let mut input = GameInput::default();
            input.actions[action as usize].pressed = true;
            runtime.game_input(input);
            runtime.tick(STEP);
            runtime.game_input(GameInput::default());
            runtime.tick(STEP);
        };
        press(&mut runtime, InputAction::Interact);
        assert_eq!(runtime.game_state()["score"], ScriptValue::Number(99.0));
        assert_eq!(
            runtime
                .extract()
                .find(|(id, _, _)| *id == player_id)
                .unwrap()
                .1
                .translation
                .x,
            3.0
        );
        press(&mut runtime, InputAction::Jump);
        assert_eq!(runtime.game_state()["score"], ScriptValue::Number(7.0));
        assert_eq!(runtime.hud_value("score"), Some(ScriptValue::Number(7.0)));
        assert_eq!(
            runtime
                .extract()
                .find(|(id, _, _)| *id == player_id)
                .unwrap()
                .1
                .translation
                .x,
            0.0
        );

        let mut next = first.clone();
        next.script_sources.insert(
            script,
            "fn on_start(entity) { set_hud(\"entry\", game_get(\"score\")); }".into(),
        );
        let mut destination = Runtime::new(&next)
            .unwrap()
            .with_game_state(runtime.game_state().clone());
        destination.start_scripts(&next, &HashMap::new());
        destination.prepare_play();
        assert_eq!(
            destination.hud_value("entry"),
            Some(ScriptValue::Number(7.0))
        );
    }

    #[test]
    fn named_timers_follow_fixed_ticks_and_each_instance_has_its_own_cooldown() {
        let mut scene = Scene::default();
        let id = Uuid::new_v4();
        scene.assets.push(Asset {
            id,
            name: "Cooldown".into(),
            kind: AssetKind::Script,
        });
        scene.script_sources.insert(
            id,
            r#"
            fn fields() { #{ counter: "a" } }
            fn on_start(entity) {
                set_timer("round", 3);
                set_timer("cancelled", 30);
                cancel_timer("cancelled");
                set_hud("cancelled", timer_remaining("cancelled"));
                this.count = 0;
                if try_cooldown("dash", 4) { this.count += 1; }
                set_hud(param("counter"), this.count);
            }
            fn on_fixed_update(entity, dt) {
                set_hud("round", timer_remaining("round"));
                if try_cooldown("dash", 4) {
                    this.count += 1;
                    set_hud(param("counter"), this.count);
                }
            }
        "#
            .into(),
        );
        scene.entities[0].script = Some(id);
        let mut other = Object::cube("Other".into(), Vec3::X);
        other.script = Some(id);
        other
            .script_fields
            .insert("counter".into(), ScriptValue::Text("b".into()));
        scene.entities.push(other);
        let mut runtime = Runtime::new(&scene).unwrap();
        runtime.start_scripts(&scene, &HashMap::new());
        runtime.prepare_play();
        assert_eq!(runtime.hud_value("a"), Some(ScriptValue::Number(1.0)));
        assert_eq!(runtime.hud_value("b"), Some(ScriptValue::Number(1.0)));
        assert_eq!(
            runtime.hud_value("cancelled"),
            Some(ScriptValue::Number(0.0))
        );
        runtime.tick(STEP * 3.0);
        runtime.tick(0.0);
        assert_eq!(runtime.hud_value("round"), Some(ScriptValue::Number(0.0)));
        assert_eq!(runtime.hud_value("a"), Some(ScriptValue::Number(1.0)));
        runtime.tick(STEP);
        runtime.tick(0.0);
        assert_eq!(runtime.hud_value("a"), Some(ScriptValue::Number(2.0)));
        assert_eq!(runtime.hud_value("b"), Some(ScriptValue::Number(2.0)));
    }

    #[test]
    fn nearby_search_uses_world_positions_and_activation_controls_colliders() {
        let mut scene = Scene::default();
        let mut driver = Object::cube("Driver".into(), Vec3::ZERO);
        let mut parent = Object::cube("Parent".into(), Vec3::X * 8.0);
        parent.spin = false;
        let mut near = Object::cube("Near".into(), Vec3::X * -7.0);
        near.parent = Some(parent.uuid);
        near.tags.push("pickup".into());
        let mut far = Object::cube("Far".into(), Vec3::X * 0.1);
        far.parent = Some(parent.uuid);
        far.tags.push("pickup".into());
        let mut block = Object::cube("Block".into(), Vec3::Z * -3.0);
        block.tags.push("block".into());
        block.body = Some(Body {
            motion: Motion::Fixed,
            ..Body::default()
        });
        let block_id = block.uuid;
        let script = Uuid::new_v4();
        driver.script = Some(script);
        scene.assets.push(Asset {
            id: script,
            name: "Driver".into(),
            kind: AssetKind::Script,
        });
        scene.script_sources.insert(
            script,
            r#"
            fn on_start(entity) {
                let hits = find_nearby(entity, "pickup", 2.0);
                set_hud("nearby", hits.len);
                this.block = find_tag("block");
                set_active(this.block, false);
            }
            fn on_update(entity, dt) {
                set_hud("hidden", find_tag("block") == ());
                if input_pressed("Interact") { set_active(this.block, true); }
            }
        "#
            .into(),
        );
        scene.entities = vec![driver, parent, near, far, block];
        let mut runtime = Runtime::new(&scene).unwrap();
        runtime.start_scripts(&scene, &HashMap::new());
        runtime.prepare_play();
        runtime.tick(STEP);
        runtime.tick(STEP);
        assert!(!runtime.is_active(block_id));
        assert_eq!(runtime.hud_value("nearby"), Some(ScriptValue::Number(1.0)));
        assert_eq!(runtime.hud_value("hidden"), Some(ScriptValue::Bool(true)));
        assert!(
            runtime
                .raycast(Vec3::ZERO, Vec3::NEG_Z, 10.0, u32::MAX)
                .is_none()
        );

        let mut input = GameInput::default();
        input.actions[InputAction::Interact as usize].pressed = true;
        runtime.game_input(input);
        runtime.tick(STEP);
        runtime.game_input(GameInput::default());
        runtime.tick(STEP);
        assert!(runtime.is_active(block_id));
        assert_eq!(
            runtime
                .raycast(Vec3::ZERO, Vec3::NEG_Z, 10.0, u32::MAX)
                .unwrap()
                .entity,
            block_id
        );
    }

    #[test]
    fn seeded_runs_assign_identical_ids_to_the_whole_spawned_branch() {
        let mut scene = Scene::default();
        let root = scene.entities[0].uuid;
        let mut child = Object::cube("Child".into(), Vec3::Y);
        child.parent = Some(root);
        scene.entities.push(child);
        scene.create_prefab(root).unwrap();
        let script = Uuid::new_v4();
        scene.assets.push(Asset {
            id: script,
            name: "Spawner".into(),
            kind: AssetKind::Script,
        });
        scene.script_sources.insert(
            script,
            r#"fn on_start(entity) { spawn_prefab("Cube", 1, 0, 0); }"#.into(),
        );
        scene.entities[0].script = Some(script);
        let ids = |seed| {
            let mut runtime = Runtime::new(&scene).unwrap();
            runtime.start_seeded_scripts(&scene, &HashMap::new(), seed);
            runtime.prepare_play();
            runtime
                .spawned_objects()
                .iter()
                .map(|object| (object.uuid, object.parent))
                .collect::<Vec<_>>()
        };
        let first = ids(7);
        assert_eq!(first.len(), 2);
        assert_eq!(first[1].1, Some(first[0].0));
        assert_eq!(first, ids(7));
        assert_ne!(first, ids(8));
    }

    #[test]
    fn hud_layout_round_trips_and_button_dispatches_one_script_click() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hud.futty.scene.ron");
        let mut scene = Scene::default();
        let script = Uuid::new_v4();
        scene.assets.push(Asset {
            id: script,
            name: "HUD controls".into(),
            kind: AssetKind::Script,
        });
        scene.entities[0].script = Some(script);
        scene.script_sources.insert(
            script,
            r#"
                fn on_start(entity) { set_hud("energy", 0.5); }
                fn on_update(entity, dt) {
                    if hud_pressed("charge") {
                        set_hud("energy", 0.75);
                    } else {
                        set_hud("energy", 0.5);
                    }
                }
            "#
            .into(),
        );
        let mut gauge = HudWidget::new(HudKind::Gauge);
        gauge.binding = "energy".into();
        let mut button = HudWidget::new(HudKind::Button);
        button.binding = "charge".into();
        scene.hud.extend([gauge, button]);
        scene.save(&path).unwrap();
        let loaded = Scene::load(&path).unwrap();
        assert_eq!(loaded, scene);

        let mut runtime = Runtime::new(&loaded).unwrap();
        runtime.start_scripts(&loaded, &HashMap::new());
        runtime.tick(STEP);
        assert_eq!(runtime.hud_value("energy"), Some(ScriptValue::Number(0.5)));
        runtime.click_hud("charge");
        runtime.tick(STEP);
        runtime.tick(STEP);
        assert_eq!(runtime.hud_value("energy"), Some(ScriptValue::Number(0.75)));
        runtime.tick(STEP);
        assert_eq!(runtime.hud_value("energy"), Some(ScriptValue::Number(0.5)));

        scene.hud.push(scene.hud[0].clone());
        assert!(scene.save(&path).is_err());
        scene.hud.pop();
        let mut image = HudWidget::new(HudKind::Image);
        image.image = Some(Uuid::new_v4());
        scene.hud.push(image);
        assert!(scene.save(&path).is_err());
        assert_eq!(Scene::load(&path).unwrap(), loaded);
    }

    #[test]
    fn input_event_reaches_tagged_entity_on_next_tick() {
        let mut scene = Scene::default();
        let switch = Uuid::new_v4();
        let receiver = Uuid::new_v4();
        scene.assets.extend([
            Asset {
                id: switch,
                name: "Switch".into(),
                kind: AssetKind::Script,
            },
            Asset {
                id: receiver,
                name: "Receiver".into(),
                kind: AssetKind::Script,
            },
        ]);
        scene.script_sources.insert(
            switch,
            r#"
            fn on_update(entity, dt) {
                if input_pressed("Interact") {
                    let target = find_tag("door");
                    if type_of(target) == "Entity" { send_event(target, "open"); }
                }
            }
        "#
            .into(),
        );
        scene.script_sources.insert(
            receiver,
            r#"
            fn on_message(entity, sender, name) {
                if name == "open" { translate(entity, 2, 0, 0); }
            }
        "#
            .into(),
        );
        scene.entities[0].script = Some(switch);
        let mut door = Object::cube("Door".into(), Vec3::ZERO);
        door.spin = false;
        door.tags.push("door".into());
        door.script = Some(receiver);
        let door_id = door.uuid;
        scene.entities.push(door);
        let mut runtime = Runtime::new(&scene).unwrap();
        runtime.start_scripts(&scene, &HashMap::new());
        let mut input = GameInput::default();
        input.actions[InputAction::Interact as usize].pressed = true;
        runtime.game_input(input);
        let translation = |runtime: &mut Runtime| {
            runtime
                .extract()
                .find(|object| object.0 == door_id)
                .unwrap()
                .1
                .translation
        };
        runtime.tick(STEP);
        assert_eq!(translation(&mut runtime), Vec3::ZERO);
        runtime.game_input(GameInput::default());
        runtime.tick(STEP);
        assert_eq!(translation(&mut runtime), Vec3::X * 2.0);
        runtime.tick(STEP);
        assert_eq!(translation(&mut runtime), Vec3::X * 2.0);
        assert_eq!(scene.entities[1].transform.translation, Vec3::ZERO);
    }

    #[test]
    fn embedded_script_fields_and_state_are_per_object_and_drive_kinematics() {
        let mut scene = Scene::default();
        let script = Uuid::new_v4();
        scene.assets.push(Asset {
            id: script,
            name: "Controller".into(),
            kind: AssetKind::Script,
        });
        scene.script_sources.insert(script, r#"
            fn fields() { #{ speed: 2, enabled: true, greeting: "ready" } }
            fn on_start(entity) {
                this.steps = 0;
                log(param("greeting"));
            }
            fn on_update(entity, dt) {
                if param("enabled") && key_down("Space") {
                    this.steps += 1;
                    let position = get_position(entity);
                    set_position(entity, position.x + this.steps.to_float() * param("speed"), position.y, position.z);
                }
            }
        "#.into());
        scene.entities[0].script = Some(script);
        scene.entities[0]
            .script_fields
            .insert("speed".into(), ScriptValue::Number(0.5));
        let mut second = Object::cube("Kinematic".into(), Vec3::X * 10.0);
        second.script = Some(script);
        second.spin = false;
        second.body = Some(Body {
            motion: Motion::Kinematic,
            ..Default::default()
        });
        scene.entities.push(second);

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("controller.ron");
        scene.save(&path).unwrap();
        let reloaded = Scene::load(&path).unwrap();
        assert_eq!(reloaded, scene);
        let mut runtime = Runtime::new(&reloaded).unwrap();
        runtime.start_scripts(&reloaded, &HashMap::new());
        runtime.game_input(GameInput {
            held: vec!["Space"],
            ..Default::default()
        });
        runtime.tick(STEP);
        runtime.tick(STEP);
        runtime.game_input(GameInput::default());
        runtime.tick(STEP);
        let poses: HashMap<_, _> = runtime
            .extract()
            .map(|(id, transform, _)| (id, transform.translation.x))
            .collect();
        assert!((poses[&scene.entities[0].uuid] - 1.5).abs() < 1e-4);
        assert!((poses[&scene.entities[1].uuid] - 16.0).abs() < 1e-4);
        assert_eq!(runtime.script_log().count(), 2);
        assert_eq!(scene.entities[1].transform.translation.x, 10.0);
    }

    #[test]
    fn dynamic_scripts_drive_velocity_and_impulse_through_physics() {
        let mut scene = Scene::default();
        let script = Uuid::new_v4();
        scene.assets.push(Asset {
            id: script,
            name: "Launch".into(),
            kind: AssetKind::Script,
        });
        scene.script_sources.insert(
            script,
            r#"
            fn on_start(entity) {
                set_velocity(entity, 2, 0, 0);
                add_impulse(entity, 1, 0, 0);
            }
        "#
            .into(),
        );
        scene.entities[0].script = Some(script);
        scene.entities[0].body = Some(Body {
            gravity_scale: 0.0,
            ..Default::default()
        });
        let mut runtime = Runtime::new(&scene).unwrap();
        runtime.start_scripts(&scene, &HashMap::new());
        for _ in 0..30 {
            runtime.tick(STEP);
        }
        assert!(runtime.script_log().next().is_none());
        let position = runtime.extract().next().unwrap().1.translation;
        assert!((position.x - 1.5).abs() < 0.02, "{position:?}");
    }

    #[test]
    fn startup_audio_and_listener_are_scene_data() {
        let mut scene = Scene::default();
        let clip = Uuid::new_v4();
        scene.assets.push(Asset {
            id: clip,
            name: "Chime".into(),
            kind: AssetKind::Wav,
        });
        scene.entities[0].audio_source = Some(AudioSource::new(clip));
        scene.entities[0].audio_listener = Some(AudioListener { volume: 0.4 });
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sound.ron");
        scene.save(&path).unwrap();
        let loaded = Scene::load(&path).unwrap();
        assert_eq!(loaded, scene);
        let mut runtime = Runtime::new(&loaded).unwrap();
        assert_eq!(runtime.audio_gain(), 0.4);
        assert_eq!(
            runtime.drain_audio().collect::<Vec<_>>(),
            [AudioCue {
                object: scene.entities[0].uuid,
                source: AudioSource::new(clip),
            }]
        );
        runtime.tick(STEP);
        assert!(runtime.drain_audio().next().is_none());
    }

    #[test]
    fn rhai_start_and_wasd_move_only_the_runtime_copy() {
        let mut scene = Scene::default();
        let id = Uuid::new_v4();
        scene.assets.push(Asset {
            id,
            name: "Move".into(),
            kind: AssetKind::Script,
        });
        scene.entities[0].script = Some(id);
        scene.entities[0].spin = false;
        let sources = HashMap::from([(
            id,
            include_str!("../../../examples/script-playground/Move.rhai").into(),
        )]);
        let mut runtime = Runtime::new(&scene).unwrap();
        runtime.start_scripts(&scene, &sources);
        runtime.start_scripts(&scene, &sources);
        runtime.game_input(GameInput {
            forward: true,
            ..Default::default()
        });
        for _ in 0..60 {
            runtime.tick(STEP);
        }
        assert_eq!(runtime.script_log().count(), 1);
        assert!(runtime.script_log().next().unwrap().contains("Ready"));
        let transform = runtime.extract().next().unwrap().1;
        assert!((transform.translation.z + 2.4).abs() < 1e-4);
        assert_eq!(scene.entities[0].transform.translation.z, 0.0);
    }

    #[test]
    fn runaway_rhai_callback_discards_queued_transform_and_disables_instance() {
        let mut scene = Scene::default();
        let id = Uuid::new_v4();
        scene.assets.push(Asset {
            id,
            name: "Runaway".into(),
            kind: AssetKind::Script,
        });
        scene.entities[0].script = Some(id);
        scene.entities[0].spin = false;
        let mut runtime = Runtime::new(&scene).unwrap();
        runtime.start_scripts(
            &scene,
            &HashMap::from([(
                id,
                r#"
            fn on_update(entity, dt) {
                translate_local(entity, 1.0, 0.0, 0.0);
                loop { }
            }
        "#
                .into(),
            )]),
        );
        runtime.tick(STEP);
        runtime.tick(STEP);
        let message = runtime
            .script_log()
            .next()
            .expect("the runaway callback must log an error");
        assert!(
            message.contains("Runaway.rhai") && message.contains("on_update"),
            "{message}"
        );
        assert_eq!(runtime.script_log().count(), 1);
        assert_eq!(
            runtime.extract().next().unwrap().1,
            scene.entities[0].transform
        );
    }

    #[test]
    fn dynamic_box_crosses_sensor_and_lands_on_fixed_floor() {
        let mut scene = Scene::default();
        let dropped = &mut scene.entities[0];
        dropped.transform.translation.y = 3.0;
        dropped.body = Some(Body::default());
        let dropped_id = dropped.uuid;

        let mut floor = Object::cube("Floor".into(), Vec3::Y * -0.25);
        floor.transform.scale = Vec3::new(6.0, 0.5, 6.0);
        floor.body = Some(Body {
            motion: Motion::Fixed,
            ..Default::default()
        });
        let floor_id = floor.uuid;
        scene.entities.push(floor);

        let mut sensor = Object::cube("Sensor".into(), Vec3::Y * 1.5);
        sensor.body = Some(Body {
            motion: Motion::Fixed,
            size: Vec3::new(2.0, 0.2, 2.0),
            sensor: true,
            ..Default::default()
        });
        let sensor_id = sensor.uuid;
        let clip = Uuid::new_v4();
        let source = AudioSource {
            start: AudioStart::OnTriggerEnter,
            ..AudioSource::new(clip)
        };
        sensor.audio_source = Some(source);
        scene.assets.push(Asset {
            id: clip,
            name: "Sensor chime".into(),
            kind: AssetKind::Ogg,
        });
        scene.entities.push(sensor);

        let before = scene.clone();
        let mut runtime = Runtime::new(&scene).unwrap();
        for _ in 0..180 {
            runtime.tick(STEP);
        }
        let (floor_transform, _) = runtime
            .extract()
            .find(|&(id, _, _)| id == floor_id)
            .map(|(_, transform, color)| (transform, color))
            .unwrap();
        assert_eq!(floor_transform.rotation, Quat::IDENTITY);
        let (_, dropped_transform, _) = runtime
            .extract()
            .find(|&(id, _, _)| id == dropped_id)
            .unwrap();
        assert!((dropped_transform.translation.y - 0.5).abs() < 0.025);
        let trigger_events: Vec<_> = runtime
            .physics_events()
            .filter(|event| event.sensor && event.first == sensor_id && event.second == dropped_id)
            .map(|event| event.started)
            .collect();
        assert_eq!(trigger_events, [true, false]);
        assert_eq!(
            runtime.drain_audio().collect::<Vec<_>>(),
            [AudioCue {
                object: sensor_id,
                source
            }]
        );
        assert!(
            runtime
                .physics_events()
                .any(|event| !event.sensor && event.started)
        );
        assert_eq!(scene, before);

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("physics.ron");
        scene.save(&path).unwrap();
        assert_eq!(Scene::load(&path).unwrap(), scene);
    }

    #[test]
    fn physics_caps_catchup_and_drives_kinematic_rotation() {
        let mut scene = Scene::default();
        scene.entities[0].body = Some(Body {
            motion: Motion::Kinematic,
            ..Default::default()
        });
        let mut runtime = Runtime::new(&scene).unwrap();
        runtime.tick(1.0);
        let (_, transform, _) = runtime.extract().next().unwrap();
        let expected = Quat::from_rotation_y((MAX_STEPS as f32) * STEP * 0.7);
        assert!(
            transform.rotation.abs_diff_eq(expected, 1e-4),
            "{transform:?}"
        );
        let previous = transform.rotation;
        runtime.tick(0.0);
        assert_eq!(runtime.extract().next().unwrap().1.rotation, previous);
    }

    #[test]
    fn hierarchy_uses_parent_matrices_and_rejects_cycles_and_missing_parents() {
        let mut scene = Scene::default();
        let parent = scene.entities[0].uuid;
        scene.entities[0].transform.translation = Vec3::X * 4.0;
        scene.entities[0].transform.scale = Vec3::splat(2.0);
        let mut child = Object::cube("Child".into(), Vec3::Y);
        child.parent = Some(parent);
        scene.entities.insert(0, child);
        let world = scene
            .hierarchy()
            .unwrap()
            .world_matrices(|index| scene.entities[index].transform.matrix());
        assert_eq!(world[0].w_axis.truncate(), Vec3::new(4.0, 2.0, 0.0));
        scene.entities[1].parent = Some(scene.entities[0].uuid);
        assert!(scene.validate().unwrap_err().to_string().contains("cycle"));
        scene.entities.remove(1);
        assert!(
            scene
                .validate()
                .unwrap_err()
                .to_string()
                .contains("Missing parent")
        );
    }

    #[test]
    fn opens_initial_scene_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.ron");
        let object = Uuid::new_v4();
        let transform = ron::to_string(&Transform::default()).unwrap();
        fs::write(&path, format!(
            r#"(format_version:1,scene_uuid:"{}",name:"Old",entities:[(uuid:"{object}",name:"Cube",transform:{transform},color:(1.0,1.0,1.0),spin:true)])"#,
            Uuid::new_v4()
        )).unwrap();
        let scene = Scene::load(&path).unwrap();
        assert_eq!(scene.format_version, FORMAT_VERSION);
        assert!(!scene.sky);
        assert_eq!(scene.entities[0].uuid, object);
        assert_eq!(scene.entities[0].primitive, Primitive::Cube);
        assert!(scene.assets.is_empty() && scene.entities[0].parent.is_none());
    }

    #[test]
    fn save_load_and_replace() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.futty.scene.ron");
        let mut scene = Scene::default();
        assert!(scene.sky);
        scene.save(&path).unwrap();
        assert_eq!(Scene::load(&path).unwrap(), scene);
        scene.entities[0].transform.translation.x = 2.0;
        scene.sky = false;
        scene.save(&path).unwrap();
        assert_eq!(Scene::load(&path).unwrap(), scene);
        scene.entities.push(scene.entities[0].clone());
        assert!(scene.save(&path).is_err());
        assert_eq!(Scene::load(&path).unwrap().entities.len(), 1);
    }

    #[test]
    fn shared_materials_round_trip_and_validate_before_replacing_a_scene() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("materials.ron");
        let mut scene = Scene::default();
        let id = Uuid::new_v4();
        scene.materials.insert(
            id,
            Material {
                name: "Copper".into(),
                color: [0.7, 0.3, 0.1],
                surface: Surface {
                    roughness: 0.2,
                    emission: [1.0, 0.0, 0.0],
                    emission_strength: 2.0,
                },
                ..Default::default()
            },
        );
        scene.entities[0].material = Some(id);
        scene.save(&path).unwrap();
        assert_eq!(Scene::load(&path).unwrap(), scene);
        let saved = fs::read(&path).unwrap();
        for roughness in [f32::NAN, -0.1, 1.1] {
            scene.materials.get_mut(&id).unwrap().surface.roughness = roughness;
            assert!(scene.save(&path).is_err());
            assert_eq!(fs::read(&path).unwrap(), saved);
        }
        scene.materials.get_mut(&id).unwrap().surface.roughness = 0.5;
        scene.materials.get_mut(&id).unwrap().texture = Some(Uuid::new_v4());
        assert!(scene.validate().is_err());
        scene.materials.clear();
        assert!(
            scene
                .validate()
                .unwrap_err()
                .to_string()
                .contains("Missing material")
        );
    }

    #[test]
    fn sprite_scene_preserves_settings_and_validates_texture_reference() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sprite.futty.scene.ron");
        let mut scene = Scene::default();
        let texture = Uuid::new_v4();
        let sprite = Sprite {
            texture,
            size: glam::Vec2::new(2.0, 1.0),
            billboard: Billboard::Upright,
            sheet: None,
        };
        scene.entities[0].sprite = Some(sprite);
        assert!(scene.save(&path).is_err());
        scene.assets.push(Asset {
            id: texture,
            name: "Sprite".into(),
            kind: AssetKind::Texture,
        });
        scene.save(&path).unwrap();
        assert_eq!(Scene::load(&path).unwrap().entities[0].sprite, Some(sprite));
        scene.entities[0].sprite.as_mut().unwrap().size.x = 0.0;
        assert!(scene.save(&path).is_err());
    }

    #[test]
    fn built_in_shapes_survive_save_and_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shapes.futty.scene.ron");
        let scene = Scene {
            entities: Primitive::ALL
                .into_iter()
                .map(|shape| Object {
                    primitive: shape,
                    ..Object::cube(shape.label().into(), Vec3::ZERO)
                })
                .collect(),
            ..Scene::default()
        };
        scene.save(&path).unwrap();
        assert_eq!(Scene::load(&path).unwrap(), scene);
    }

    #[test]
    fn scene_persists_main_camera_and_validates_camera_references() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cameras.futty.scene.ron");
        let mut scene = Scene::default();
        let mut a = Object::cube("Camera A".into(), Vec3::ZERO);
        a.camera = Some(CameraLens::default());
        let mut b = Object::cube("Camera B".into(), Vec3::Y);
        b.camera = Some(CameraLens {
            projection: futty_core::Projection::Orthographic,
            size: 3.0,
            ..Default::default()
        });
        let (a_id, b_id) = (a.uuid, b.uuid);
        scene.entities.extend([a, b]);
        scene.main_camera = Some(b_id);
        scene.save(&path).unwrap();
        let loaded = Scene::load(&path).unwrap();
        assert_eq!(loaded, scene);
        assert_eq!(loaded.camera_index(None), Some(2));
        assert_eq!(loaded.camera_index(Some(a_id)), Some(1));
        scene.entities[2].camera.as_mut().unwrap().far = 0.01;
        assert!(scene.save(&path).is_err());
        scene.entities[2].camera = None;
        assert!(scene.save(&path).is_err());
    }

    #[test]
    fn scene_preserves_light_parameters_and_rejects_invalid_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lights.futty.scene.ron");
        let mut scene = Scene::default();
        let mut object = Object::cube("Lamp".into(), Vec3::Y);
        object.light = Some(Light {
            color: [0.1, 0.5, 1.0],
            range: 6.25,
            ..Light::new(futty_core::LightKind::Point)
        });
        scene.entities.push(object);
        scene.save(&path).unwrap();
        assert_eq!(Scene::load(&path).unwrap(), scene);
        scene.entities[1].light.as_mut().unwrap().intensity = -1.0;
        assert!(scene.save(&path).is_err());
    }

    #[test]
    fn directional_shadows_round_trip_and_older_lights_default_to_unshadowed() {
        let old: Light =
            ron::from_str("(kind:Directional,color:(1.0,1.0,1.0),intensity:0.75,range:8.0)")
                .unwrap();
        assert!(!old.cast_shadows && old.valid());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shadows.futty.scene.ron");
        let mut scene = Scene::default();
        let mut light = Object::cube("Sun".into(), Vec3::Y);
        light.light = Some(Light {
            shadow_distance: 42.0,
            ..Light::new(futty_core::LightKind::Directional)
        });
        scene.entities.push(light);
        scene.save(&path).unwrap();
        assert_eq!(Scene::load(&path).unwrap(), scene);
        scene.entities[1].light.as_mut().unwrap().shadow_distance = 0.0;
        assert!(scene.save(&path).is_err());
    }

    #[test]
    fn play_world_preserves_edit_world() {
        let scene = Scene::default();
        for _ in 0..100 {
            let mut runtime = Runtime::new(&scene).unwrap();
            let object = &scene.entities[0];
            assert_eq!(runtime.extract().next().unwrap().1, object.transform);
            runtime.tick(1.0 / 60.0);
            let (id, transform, _) = runtime.extract().next().unwrap();
            assert_eq!(id, object.uuid);
            assert_ne!(transform.rotation, object.transform.rotation);
            assert_eq!(object.transform.rotation, Quat::IDENTITY);
        }
    }
}
mod physics;
