use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    rc::Rc,
};

use futty_core::Transform;
use glam::{EulerRot, Quat, Vec3};
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

#[derive(Clone, Default)]
pub struct GameInput {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub held: Vec<&'static str>,
    pub range: crate::RangeInput,
}

impl GameInput {
    fn down(&self, action: &str) -> bool {
        match action {
            "MoveForward" => self.forward,
            "MoveBackward" => self.backward,
            "MoveLeft" => self.left,
            "MoveRight" => self.right,
            _ => false,
        }
    }
}

#[derive(Clone, Copy)]
struct EntityRef(Uuid);

#[derive(Default)]
pub(crate) struct Commands {
    pub velocity: Option<Vec3>,
    pub impulse: Vec3,
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

pub(crate) enum Callback {
    Start,
    Update(f32),
    Fixed(f32),
    Event(&'static str, Uuid),
}

impl Callback {
    fn name(&self) -> &'static str {
        match self {
            Self::Start => "on_start",
            Self::Update(_) => "on_update",
            Self::Fixed(_) => "on_fixed_update",
            Self::Event(name, _) => name,
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
        engine.on_print(|_| {});
        engine.on_debug(|_, _, _| {});
        let output = effects.clone();
        engine.register_fn("log", move |message: ImmutableString| {
            let mut effects = output.borrow_mut();
            if effects.logs.len() < 8 {
                effects.logs.push(message.to_string());
            }
        });
        let actions = effects.clone();
        engine.register_fn("input_down", move |action: ImmutableString| {
            actions.borrow().input.down(&action)
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

    fn push(&mut self, message: String) {
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
