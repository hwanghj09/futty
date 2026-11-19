use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    rc::Rc,
};

use futty_core::Transform;
use glam::{EulerRot, Quat, Vec2, Vec3};
use rhai::{
    AST, CallFnOptions, Dynamic, Engine, EvalAltResult, FLOAT, ImmutableString, Map,
    OptimizationLevel, Scope,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::Motion;

type RhaiResult<T> = Result<T, Box<EvalAltResult>>;
pub const MAX_SCRIPT_BYTES: usize = 64 * 1024;
pub type ScriptFields = BTreeMap<String, ScriptValue>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ScriptValue {
    Number(f64),
    Bool(bool),
    Text(String),
}

impl ScriptValue {
    pub fn same_type(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    pub(crate) fn valid(&self) -> bool {
        match self {
            Self::Number(value) => value.is_finite(),
            Self::Bool(_) => true,
            Self::Text(value) => value.len() <= 1024,
        }
    }

    fn from_dynamic(value: &Dynamic) -> Option<Self> {
        if let Ok(value) = value.as_bool() {
            Some(Self::Bool(value))
        } else if let Ok(value) = numeric(value) {
            Some(Self::Number(value))
        } else {
            value
                .clone()
                .try_cast::<ImmutableString>()
                .map(|value| Self::Text(value.to_string()))
        }
    }

    fn to_dynamic(&self) -> Dynamic {
        match self {
            Self::Number(value) => Dynamic::from_float(*value),
            Self::Bool(value) => Dynamic::from_bool(*value),
            Self::Text(value) => Dynamic::from(value.clone()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum InputAction {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    Jump,
    Fire,
    Reload,
    Interact,
    Retry,
}

impl InputAction {
    pub const ALL: [Self; 9] = [
        Self::MoveForward,
        Self::MoveBackward,
        Self::MoveLeft,
        Self::MoveRight,
        Self::Jump,
        Self::Fire,
        Self::Reload,
        Self::Interact,
        Self::Retry,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::MoveForward => "MoveForward",
            Self::MoveBackward => "MoveBackward",
            Self::MoveLeft => "MoveLeft",
            Self::MoveRight => "MoveRight",
            Self::Jump => "Jump",
            Self::Fire => "Fire",
            Self::Reload => "Reload",
            Self::Interact => "Interact",
            Self::Retry => "Retry",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActionState {
    pub held: bool,
    pub pressed: bool,
    pub released: bool,
}

#[derive(Clone, Default)]
pub struct GameInput {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub held: Vec<&'static str>,
    pub range: crate::RangeInput,
    pub actions: [ActionState; InputAction::ALL.len()],
    pub mouse_delta: Vec2,
}

impl GameInput {
    pub fn action(&self, action: InputAction) -> ActionState {
        let mut state = self.actions[action as usize];
        state.held |= match action {
            InputAction::MoveForward => self.forward,
            InputAction::MoveBackward => self.backward,
            InputAction::MoveLeft => self.left,
            InputAction::MoveRight => self.right,
            _ => false,
        };
        state
    }

    fn named_action(&self, name: &str) -> ActionState {
        InputAction::ALL
            .into_iter()
            .find(|action| action.name() == name)
            .map_or_else(ActionState::default, |action| self.action(action))
    }
}

#[derive(Clone, Copy)]
struct EntityRef(Uuid);

pub(crate) const MAX_WORLD_COMMANDS: usize = 128;

pub(crate) struct Message {
    pub sender: Uuid,
    pub target: Uuid,
    pub name: String,
}

pub(crate) enum WorldCommand {
    Message(Message),
    Spawn {
        prefab: Uuid,
        root: Uuid,
        position: Vec3,
    },
    Despawn(Uuid),
    HudSet {
        name: String,
        value: ScriptValue,
    },
    LoadScene(String),
}

#[derive(Default)]
pub(crate) struct Commands {
    pub velocity: Option<Vec3>,
    pub impulse: Vec3,
    pub world: Vec<WorldCommand>,
    pub animation: Option<(u32, bool)>,
}

#[derive(Default)]
struct Effects {
    entity: Option<Uuid>,
    motion: Option<Motion>,
    pose: Transform,
    input: Rc<GameInput>,
    fields: Rc<ScriptFields>,
    physics: Commands,
    logs: Vec<String>,
}

fn world_command(effects: &mut Effects, command: WorldCommand) -> RhaiResult<()> {
    if effects.entity.is_none() || effects.physics.world.len() >= 8 {
        return Err("An active entity can queue up to eight world commands per callback".into());
    }
    effects.physics.world.push(command);
    Ok(())
}

pub(crate) enum Callback {
    Start,
    Update(f32),
    Fixed(f32),
    Event(&'static str, Uuid),
    Message(Uuid, String),
}

impl Callback {
    fn name(&self) -> &'static str {
        match self {
            Self::Start => "on_start",
            Self::Update(_) => "on_update",
            Self::Fixed(_) => "on_fixed_update",
            Self::Event(name, _) => name,
            Self::Message(_, _) => "on_message",
        }
    }
}

struct Program {
    ast: AST,
    defaults: ScriptFields,
    functions: HashSet<String>,
}

struct Instance {
    fields: Rc<ScriptFields>,
    state: Dynamic,
}

pub(crate) struct Scripts {
    engine: Engine,
    effects: Rc<RefCell<Effects>>,
    programs: HashMap<Uuid, (String, Result<Program, String>)>,
    instances: HashMap<Uuid, Instance>,
    disabled: HashSet<Uuid>,
    pub(crate) log: VecDeque<String>,
    pub(crate) input: Rc<GameInput>,
    pub(crate) tags: Rc<RefCell<HashMap<String, Vec<Uuid>>>>,
    pub(crate) prefabs: Rc<RefCell<HashMap<String, Uuid>>>,
    pub(crate) hud_clicks: Rc<RefCell<HashSet<String>>>,
}

pub fn inspect_script(source: &str) -> Result<ScriptFields, String> {
    let scripts = Scripts::new();
    scripts
        .build_program(source)
        .map(|program| program.defaults)
}

impl Scripts {
    pub(crate) fn new() -> Self {
        let effects = Rc::new(RefCell::new(Effects::default()));
        let mut engine = Engine::new();
        engine
            .set_max_operations(5_000)
            .set_max_call_levels(12)
            .set_max_expr_depths(32, 24)
            .set_max_array_size(128)
            .set_max_map_size(64)
            .set_max_string_size(1024)
            .set_max_variables(64)
            .set_max_functions(24)
            .set_optimization_level(OptimizationLevel::None);
        engine.disable_symbol("eval");
        engine.register_type_with_name::<EntityRef>("Entity");
        let playback = effects.clone();
        engine.register_fn(
            "play_animation",
            move |entity: EntityRef, clip: rhai::INT, looping: bool| -> RhaiResult<()> {
                let mut effects = playback.borrow_mut();
                if effects.entity != Some(entity.0) || !(0..256).contains(&clip) {
                    return Err(
                        "Expected the current entity and a GLB clip index from 0 to 255".into(),
                    );
                }
                effects.physics.animation = Some((clip as u32, looping));
                Ok(())
            },
        );
        let tags = Rc::<RefCell<HashMap<String, Vec<Uuid>>>>::default();
        let prefabs = Rc::<RefCell<HashMap<String, Uuid>>>::default();
        let hud_clicks = Rc::<RefCell<HashSet<String>>>::default();
        let scene_effects = effects.clone();
        engine.register_fn("load_scene", move |id: ImmutableString| -> RhaiResult<()> {
            if !crate::valid_scene_id(&id) {
                return Err("Expected a scene ID of 1–128 bytes without control characters".into());
            }
            world_command(
                &mut scene_effects.borrow_mut(),
                WorldCommand::LoadScene(id.to_string()),
            )
        });
        let clicks = hud_clicks.clone();
        engine.register_fn("hud_pressed", move |name: ImmutableString| {
            clicks.borrow().contains(name.as_str())
        });
        let values = effects.clone();
        engine.register_fn(
            "set_hud",
            move |name: ImmutableString, value: Dynamic| -> RhaiResult<()> {
                if !crate::hud::valid_binding(&name) {
                    return Err("Expected a HUD binding name of 1–64 bytes".into());
                }
                let value = ScriptValue::from_dynamic(&value)
                    .filter(ScriptValue::valid)
                    .ok_or("HUD values must be finite numbers, booleans or text")?;
                world_command(
                    &mut values.borrow_mut(),
                    WorldCommand::HudSet {
                        name: name.to_string(),
                        value,
                    },
                )
            },
        );
        let lookup = tags.clone();
        engine.register_fn("find_tag", move |tag: ImmutableString| -> Dynamic {
            lookup
                .borrow()
                .get(tag.as_str())
                .and_then(|ids| ids.first())
                .map_or(Dynamic::UNIT, |&id| Dynamic::from(EntityRef(id)))
        });
        let lookup = tags.clone();
        engine.register_fn(
            "find_all_tagged",
            move |tag: ImmutableString| -> rhai::Array {
                lookup
                    .borrow()
                    .get(tag.as_str())
                    .into_iter()
                    .flatten()
                    .take(128)
                    .map(|&id| Dynamic::from(EntityRef(id)))
                    .collect()
            },
        );
        let messages = effects.clone();
        engine.register_fn(
            "send_event",
            move |target: EntityRef, name: ImmutableString| -> RhaiResult<()> {
                let mut effects = messages.borrow_mut();
                let sender = effects
                    .entity
                    .ok_or("send_event requires an active entity")?;
                if name.is_empty() || name.len() > 64 {
                    return Err("Expected a 1–64 byte event name".into());
                }
                world_command(
                    &mut effects,
                    WorldCommand::Message(Message {
                        sender,
                        target: target.0,
                        name: name.to_string(),
                    }),
                )
            },
        );
        let templates = prefabs.clone();
        let spawn = effects.clone();
        engine.register_fn(
            "spawn_prefab",
            move |name: ImmutableString,
                  x: Dynamic,
                  y: Dynamic,
                  z: Dynamic|
                  -> RhaiResult<EntityRef> {
                let prefab = templates
                    .borrow()
                    .get(name.as_str())
                    .copied()
                    .ok_or("Unknown prefab")?;
                let position = Vec3::new(
                    bounded(&x, 10_000.0)?,
                    bounded(&y, 10_000.0)?,
                    bounded(&z, 10_000.0)?,
                );
                let root = Uuid::new_v4();
                world_command(
                    &mut spawn.borrow_mut(),
                    WorldCommand::Spawn {
                        prefab,
                        root,
                        position,
                    },
                )?;
                Ok(EntityRef(root))
            },
        );
        let despawn = effects.clone();
        engine.register_fn("despawn", move |root: EntityRef| -> RhaiResult<()> {
            world_command(&mut despawn.borrow_mut(), WorldCommand::Despawn(root.0))
        });
        engine.on_print(|_| {});
        engine.on_debug(|_, _, _| {});
        let output = effects.clone();
        engine.register_fn("log", move |message: ImmutableString| {
            let mut effects = output.borrow_mut();
            if effects.logs.len() < 8 {
                effects.logs.push(message.to_string());
            }
        });
        for (name, read) in [
            (
                "input_down",
                (|state: ActionState| state.held) as fn(ActionState) -> bool,
            ),
            ("input_pressed", |state| state.pressed),
            ("input_released", |state| state.released),
        ] {
            let actions = effects.clone();
            engine.register_fn(name, move |action: ImmutableString| {
                read(actions.borrow().input.named_action(&action))
            });
        }
        let pointer = effects.clone();
        engine.register_fn("mouse_delta_x", move || {
            FLOAT::from(pointer.borrow().input.mouse_delta.x)
        });
        let pointer = effects.clone();
        engine.register_fn("mouse_delta_y", move || {
            FLOAT::from(pointer.borrow().input.mouse_delta.y)
        });
        let keys = effects.clone();
        engine.register_fn("key_down", move |key: ImmutableString| {
            keys.borrow()
                .input
                .held
                .iter()
                .any(|held| held.eq_ignore_ascii_case(&key))
        });
        let parameters = effects.clone();
        engine.register_fn(
            "param",
            move |name: ImmutableString| -> RhaiResult<Dynamic> {
                parameters
                    .borrow()
                    .fields
                    .get(name.as_str())
                    .map(ScriptValue::to_dynamic)
                    .ok_or_else(|| format!("Unknown Inspector field: {name}").into())
            },
        );
        let position = effects.clone();
        engine.register_fn(
            "get_position",
            move |entity: EntityRef| -> RhaiResult<Map> {
                let effects = position.borrow();
                check_owner(&effects, entity)?;
                let [x, y, z] = effects.pose.translation.to_array();
                Ok(Map::from([
                    ("x".into(), FLOAT::from(x).into()),
                    ("y".into(), FLOAT::from(y).into()),
                    ("z".into(), FLOAT::from(z).into()),
                ]))
            },
        );
        let yaw = effects.clone();
        engine.register_fn(
            "rotate_y",
            move |entity: EntityRef, degrees: Dynamic| -> RhaiResult<()> {
                let mut effects = yaw.borrow_mut();
                check_motion(&effects, entity)?;
                let degrees = bounded(&degrees, 3600.0)?;
                effects.pose.rotation = (effects.pose.rotation
                    * Quat::from_rotation_y(degrees.to_radians()))
                .normalize();
                Ok(())
            },
        );
        for operation in [
            VecOperation::TranslateLocal,
            VecOperation::Translate,
            VecOperation::SetPosition,
            VecOperation::Rotate,
            VecOperation::SetScale,
            VecOperation::SetVelocity,
            VecOperation::AddImpulse,
        ] {
            let target = effects.clone();
            engine.register_fn(
                operation.name(),
                move |entity: EntityRef, x: Dynamic, y: Dynamic, z: Dynamic| -> RhaiResult<()> {
                    operation.apply(&mut target.borrow_mut(), entity, [x, y, z])
                },
            );
        }
        Self {
            engine,
            effects,
            programs: HashMap::new(),
            instances: HashMap::new(),
            disabled: HashSet::new(),
            log: VecDeque::new(),
            input: Rc::default(),
            tags,
            prefabs,
            hud_clicks,
        }
    }

    fn build_program(&self, source: &str) -> Result<Program, String> {
        if source.len() > MAX_SCRIPT_BYTES {
            return Err("Script exceeds the 64 KiB source limit".into());
        }
        let ast = self
            .engine
            .compile(source)
            .map_err(|error| error.to_string())?;
        let functions: HashSet<_> = ast
            .iter_functions()
            .map(|function| function.name.to_owned())
            .collect();
        let mut defaults = ScriptFields::new();
        if functions.contains("fields") {
            let entries = self
                .engine
                .call_fn_with_options::<Map>(
                    CallFnOptions::new().eval_ast(false),
                    &mut Scope::new(),
                    &ast,
                    "fields",
                    (),
                )
                .map_err(|error| format!("fields(): {error}"))?;
            for (name, value) in entries {
                let value = ScriptValue::from_dynamic(&value)
                    .filter(ScriptValue::valid)
                    .ok_or_else(|| {
                        format!("fields(): {name} must be a finite number, boolean or text")
                    })?;
                if name.is_empty() || name.len() > 64 {
                    return Err("fields(): names must contain 1 to 64 bytes".into());
                }
                defaults.insert(name.to_string(), value);
            }
        }
        Ok(Program {
            ast,
            defaults,
            functions,
        })
    }

    pub(crate) fn compile(&mut self, id: Uuid, name: &str, source: Option<&str>) {
        let compiled = source
            .ok_or_else(|| "Script source is missing".to_owned())
            .and_then(|source| self.build_program(source));
        self.programs.insert(id, (name.into(), compiled));
    }

    pub(crate) fn bind(&mut self, object: Uuid, asset: Uuid, overrides: &ScriptFields) {
        let Some((_, Ok(program))) = self.programs.get(&asset) else {
            return;
        };
        let mut fields = program.defaults.clone();
        for (name, value) in overrides {
            if let Some(default) = fields.get_mut(name)
                && default.same_type(value)
                && value.valid()
            {
                *default = value.clone();
            }
        }
        self.instances.insert(
            object,
            Instance {
                fields: Rc::new(fields),
                state: Dynamic::from_map(Map::new()),
            },
        );
    }

    pub(crate) fn forget(&mut self, object: Uuid) {
        self.instances.remove(&object);
        self.disabled.remove(&object);
        self.tags.borrow_mut().retain(|_, ids| {
            ids.retain(|id| *id != object);
            !ids.is_empty()
        });
    }

    pub(crate) fn call(
        &mut self,
        id: Uuid,
        script: Uuid,
        label: &str,
        transform: &mut Transform,
        motion: Option<Motion>,
        callback: Callback,
    ) -> Commands {
        if self.disabled.contains(&id) {
            return Commands::default();
        }
        let function = callback.name();
        let Some((asset, program)) = self.programs.get(&script) else {
            return Commands::default();
        };
        let program = match program {
            Ok(program) => program,
            Err(error) => {
                self.disable(id, label, function, format!("{asset}.rhai · {error}"));
                return Commands::default();
            }
        };
        if !program.functions.contains(function) {
            return Commands::default();
        }
        let Some(instance) = self.instances.get_mut(&id) else {
            return Commands::default();
        };
        *self.effects.borrow_mut() = Effects {
            entity: Some(id),
            motion,
            pose: *transform,
            input: self.input.clone(),
            fields: instance.fields.clone(),
            ..Default::default()
        };
        let mut state = instance.state.clone();
        let options = CallFnOptions::new()
            .eval_ast(false)
            .bind_this_ptr(&mut state);
        let mut scope = Scope::new();
        let entity = EntityRef(id);
        let result = match callback {
            Callback::Start => self.engine.call_fn_with_options::<Dynamic>(
                options,
                &mut scope,
                &program.ast,
                function,
                (entity,),
            ),
            Callback::Update(dt) | Callback::Fixed(dt) => {
                self.engine.call_fn_with_options::<Dynamic>(
                    options,
                    &mut scope,
                    &program.ast,
                    function,
                    (entity, FLOAT::from(dt)),
                )
            }
            Callback::Event(_, other) => self.engine.call_fn_with_options::<Dynamic>(
                options,
                &mut scope,
                &program.ast,
                function,
                (entity, EntityRef(other)),
            ),
            Callback::Message(sender, name) => self.engine.call_fn_with_options::<Dynamic>(
                options,
                &mut scope,
                &program.ast,
                function,
                (entity, EntityRef(sender), name),
            ),
        };
        let effects = std::mem::take(&mut *self.effects.borrow_mut());
        if let Err(error) = result {
            self.disable(id, label, function, format!("{asset}.rhai · {error}"));
            return Commands::default();
        }
        if !state.is::<Map>() {
            self.disable(
                id,
                label,
                function,
                format!("{asset}.rhai · this must remain a map"),
            );
            return Commands::default();
        }
        if let Err(error) = self.engine.ensure_data_size_within_limits(&state) {
            self.disable(id, label, function, format!("{asset}.rhai · {error}"));
            return Commands::default();
        }
        instance.state = state;
        *transform = effects.pose;
        for log in effects.logs {
            self.push(format!("{label} · {log}"));
        }
        effects.physics
    }

    fn disable(&mut self, id: Uuid, label: &str, function: &str, reason: String) {
        self.disabled.insert(id);
        self.push(format!("{label} ({id}) · {function} · {reason}"));
    }

    pub(crate) fn push(&mut self, message: String) {
        if self.log.len() == 32 {
            self.log.pop_front();
        }
        self.log.push_back(message);
    }
}

#[derive(Clone, Copy)]
enum VecOperation {
    TranslateLocal,
    Translate,
    SetPosition,
    Rotate,
    SetScale,
    SetVelocity,
    AddImpulse,
}

impl VecOperation {
    fn name(self) -> &'static str {
        match self {
            Self::TranslateLocal => "translate_local",
            Self::Translate => "translate",
            Self::SetPosition => "set_position",
            Self::Rotate => "rotate",
            Self::SetScale => "set_scale",
            Self::SetVelocity => "set_velocity",
            Self::AddImpulse => "add_impulse",
        }
    }

    fn apply(
        self,
        effects: &mut Effects,
        entity: EntityRef,
        [x, y, z]: [Dynamic; 3],
    ) -> RhaiResult<()> {
        check_owner(effects, entity)?;
        let limit = match self {
            Self::SetPosition => 10_000.0,
            Self::Rotate => 3600.0,
            Self::SetScale | Self::AddImpulse => 1_000.0,
            _ => 100.0,
        };
        let vector = Vec3::new(
            bounded(&x, limit)?,
            bounded(&y, limit)?,
            bounded(&z, limit)?,
        );
        match self {
            Self::SetVelocity | Self::AddImpulse => {
                if effects.motion != Some(Motion::Dynamic) {
                    return Err("Velocity and impulse require a dynamic rigid body".into());
                }
                if matches!(self, Self::SetVelocity) {
                    effects.physics.velocity = Some(vector);
                } else {
                    effects.physics.impulse = (effects.physics.impulse + vector)
                        .clamp(Vec3::splat(-1000.0), Vec3::splat(1000.0));
                }
            }
            Self::SetScale => {
                if effects.motion.is_some() || vector.min_element() < 0.01 {
                    return Err(
                        "Scale requires values of at least 0.01 and an entity without a rigid body"
                            .into(),
                    );
                }
                effects.pose.scale = vector;
            }
            Self::Rotate => {
                check_motion(effects, entity)?;
                let [x, y, z] = vector.to_array().map(f32::to_radians);
                effects.pose.rotation =
                    (effects.pose.rotation * Quat::from_euler(EulerRot::XYZ, x, y, z)).normalize();
            }
            Self::TranslateLocal | Self::Translate | Self::SetPosition => {
                check_motion(effects, entity)?;
                let translation = match self {
                    Self::TranslateLocal => {
                        effects.pose.translation + effects.pose.rotation * vector
                    }
                    Self::Translate => effects.pose.translation + vector,
                    _ => vector,
                };
                effects.pose.translation =
                    translation.clamp(Vec3::splat(-10_000.0), Vec3::splat(10_000.0));
            }
        }
        Ok(())
    }
}

fn numeric(value: &Dynamic) -> RhaiResult<f64> {
    value
        .as_float()
        .or_else(|_| value.as_int().map(|value| value as f64))
        .map_err(|_| "Expected a number".into())
}

fn bounded(value: &Dynamic, limit: f32) -> RhaiResult<f32> {
    let value = numeric(value)? as f32;
    if value.is_finite() && value.abs() <= limit {
        Ok(value)
    } else {
        Err(format!("Expected a finite number between -{limit} and {limit}").into())
    }
}

fn check_owner(effects: &Effects, entity: EntityRef) -> RhaiResult<()> {
    if effects.entity == Some(entity.0) {
        Ok(())
    } else {
        Err("A script can control only its attached entity".into())
    }
}

fn check_motion(effects: &Effects, entity: EntityRef) -> RhaiResult<()> {
    check_owner(effects, entity)?;
    if matches!(effects.motion, None | Some(Motion::Kinematic)) {
        Ok(())
    } else {
        Err("Direct movement requires a kinematic body or an entity without a rigid body".into())
    }
}
