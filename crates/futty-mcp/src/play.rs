use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

use futty_runtime::{
    ActionState, FIXED_STEP_SECONDS, GameInput, GameOutcome, Hierarchy, HudKind, InputAction,
    Runtime, Scene, ScriptValue,
};
use rmcp::{ErrorData, model::CallToolResult, schemars};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    authoring::invalid,
    io_error,
    replay::{Fingerprint, MAX_TICKS, Mark, PlayTest, Replay},
};

const MAX_SESSIONS: usize = 4;
const MAX_STEP_TICKS: u32 = 120;
const MAX_VISIBLE: usize = 32;
const MOVEMENT_ACTIONS: [&str; 6] = ["forward", "backward", "left", "right", "jump", "interact"];

type Sessions = HashMap<Uuid, Session>;
type Job = Box<dyn FnOnce(&mut Sessions) + Send>;

#[derive(Clone)]
pub(crate) struct PlayRunner(mpsc::Sender<Job>);

impl PlayRunner {
    pub fn new() -> Self {
        let (send, receive) = mpsc::channel::<Job>();
        // Each Runtime owns a Rhai engine that stays on this thread across MCP requests.
        thread::spawn(move || {
            let mut sessions = Sessions::new();
            for job in receive {
                job(&mut sessions);
            }
        });
        Self(send)
    }

    fn call(
        &self,
        job: impl FnOnce(&mut Sessions) -> Result<CallToolResult, ErrorData> + Send + 'static,
    ) -> Result<CallToolResult, ErrorData> {
        let (send, receive) = mpsc::channel();
        self.0
            .send(Box::new(move |sessions| {
                let _ = send.send(job(sessions));
            }))
            .map_err(io_error)?;
        receive.recv().map_err(io_error)?
    }

    pub fn start(&self, path: PathBuf, input: Start) -> Result<CallToolResult, ErrorData> {
        self.call(move |sessions| {
            if sessions.len() >= MAX_SESSIONS {
                return Err(invalid(
                    "Four play sessions are open; close one with play_end",
                ));
            }
            let id = Uuid::new_v4();
            let mut session = Session::load(
                &path,
                input.path,
                input.max_ticks.unwrap_or(3600),
                input.seed,
            )?;
            let result = session.observe(id)?;
            sessions.insert(id, session);
            Ok(result)
        })
    }

    pub fn observe(&self, input: SessionId) -> Result<CallToolResult, ErrorData> {
        self.call(move |sessions| session(sessions, input.session_id)?.observe(input.session_id))
    }

    pub fn step(&self, input: Step) -> Result<CallToolResult, ErrorData> {
        self.call(move |sessions| {
            let current = session(sessions, input.session_id)?;
            if input.expected_tick.is_some_and(|tick| tick != current.tick) {
                return Err(invalid(format!(
                    "Tick changed; current tick is {}",
                    current.tick
                )));
            }
            current.step(Controls {
                actions: input.actions,
                ticks: input.ticks,
                look: input.look,
            })?;
            current.observe(input.session_id)
        })
    }

    pub fn end(&self, input: SessionId) -> Result<CallToolResult, ErrorData> {
        self.call(move |sessions| {
            let closed = sessions
                .remove(&input.session_id)
                .ok_or_else(|| invalid("Unknown play session"))?;
            Ok(CallToolResult::structured(
                json!({"tick": closed.tick, "status": closed.status()}),
            ))
        })
    }

    pub fn save_replay(&self, id: Uuid, path: PathBuf) -> Result<CallToolResult, ErrorData> {
        self.call(move |sessions| {
            let current = session(sessions, id)?;
            current.replay.save(&path)?;
            Ok(CallToolResult::structured(
                json!({"saved": true, "tick": current.tick}),
            ))
        })
    }

    pub fn test(
        &self,
        path: PathBuf,
        input: PlayTest,
        output: Option<PathBuf>,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(move |_| {
            if input.steps.len() > MAX_TICKS as usize {
                return Err(invalid("Too many playtest steps"));
            }
            for step in &input.steps {
                step.validate()?;
            }
            let mut current = Session::load(
                &path,
                input.path,
                input.max_ticks.unwrap_or(3600),
                input.seed,
            )?;
            for step in input.steps {
                if current.status() != Status::Running {
                    break;
                }
                current.step(step)?;
            }
            let status = current.status();
            let expected = input.expect.unwrap_or(Status::Success);
            let last_observation = current.observation()?;
            if let Some(file) = output {
                current.replay.save(&file)?;
            }
            Ok(CallToolResult::structured(json!({
                "passed": status == expected,
                "expected": expected,
                "status": status,
                "tick": current.tick,
                "replay_path": input.replay_path,
                "last_observation": last_observation,
            })))
        })
    }

    pub fn replay(&self, path: PathBuf, expected: Replay) -> Result<CallToolResult, ErrorData> {
        self.call(move |_| {
            let source = PlaySource::load(&path)?;
            if source.digest != expected.source_digest {
                return Ok(CallToolResult::structured(
                    json!({"matched": false, "reason": "source_changed"}),
                ));
            }
            let mut current =
                Session::from_source(source, expected.scene, expected.max_ticks, expected.seed)?;
            if let Some(result) = compare_marks(&expected.marks, &current.replay.marks, 0) {
                return Ok(result);
            }
            for step in expected.steps {
                let offset = current.replay.marks.len();
                if current.status() != Status::Running {
                    return Err(invalid("Replay contains steps after a terminal state"));
                }
                current.step(step)?;
                if let Some(result) = compare_marks(&expected.marks, &current.replay.marks, offset)
                {
                    return Ok(result);
                }
            }
            if current.replay.marks.len() < expected.marks.len() {
                let tick = current.replay.marks.len();
                return Ok(difference(tick, expected.marks.get(tick), None));
            }
            Ok(CallToolResult::structured(json!({
                "matched": true,
                "status": current.status(),
                "tick": current.tick,
            })))
        })
    }
}

fn compare_marks(expected: &[Mark], actual: &[Mark], offset: usize) -> Option<CallToolResult> {
    actual
        .iter()
        .enumerate()
        .skip(offset)
        .find_map(|(tick, mark)| {
            (expected.get(tick) != Some(mark))
                .then(|| difference(tick, expected.get(tick), Some(mark)))
        })
}

fn difference(tick: usize, expected: Option<&Mark>, actual: Option<&Mark>) -> CallToolResult {
    CallToolResult::structured(json!({
        "matched": false,
        "reason": "state_changed",
        "first_different_tick": tick,
        "expected": expected,
        "actual": actual,
    }))
}

fn session(sessions: &mut Sessions, id: Uuid) -> Result<&mut Session, ErrorData> {
    sessions
        .get_mut(&id)
        .ok_or_else(|| invalid("Unknown play session"))
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Start {
    /// Workspace-relative path to a saved .ron scene with a character camera.
    pub path: String,
    /// Tick limit from 1 to 36000. Defaults to 3600.
    max_ticks: Option<u32>,
    /// Seed used to assign reproducible IDs to runtime prefab instances.
    #[serde(default)]
    seed: u32,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SessionId {
    #[schemars(with = "String")]
    session_id: Uuid,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Step {
    #[schemars(with = "String")]
    session_id: Uuid,
    /// Hold these actions throughout the step. Omitted actions are released.
    #[serde(default)]
    actions: Vec<Action>,
    /// Advance 1 to 120 ticks at 60 Hz. Defaults to 1.
    ticks: Option<u32>,
    /// Relative yaw and pitch in degrees, applied once. Positive turns right or up.
    look: Option<[f32; 2]>,
    /// If present, reject this step unless the session is still on this tick.
    expected_tick: Option<u32>,
}

#[derive(Clone, Copy, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum Action {
    Forward,
    Backward,
    Left,
    Right,
    Jump,
    Interact,
}

#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct Controls {
    /// Actions held throughout these ticks. Omitted actions are released.
    #[serde(default)]
    actions: Vec<Action>,
    /// Advance 1 to 120 ticks. Defaults to 1.
    ticks: Option<u32>,
    /// Relative yaw and pitch in degrees, applied once.
    look: Option<[f32; 2]>,
}

impl Controls {
    pub fn validate(&self) -> Result<u32, ErrorData> {
        let ticks = self.ticks.unwrap_or(1);
        if !(1..=MAX_STEP_TICKS).contains(&ticks) || self.actions.len() > MOVEMENT_ACTIONS.len() {
            return Err(invalid("Use 1 to 120 ticks and up to six actions"));
        }
        if self.look.is_some_and(|angles| {
            !angles
                .into_iter()
                .all(|angle| (-180.0..=180.0).contains(&angle))
        }) {
            return Err(invalid("Look angles must be between -180 and 180 degrees"));
        }
        Ok(ticks)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Status {
    Running,
    Success,
    Failure,
    TickLimit,
    SceneTransition,
}

impl Action {
    fn input(self) -> InputAction {
        match self {
            Self::Forward => InputAction::MoveForward,
            Self::Backward => InputAction::MoveBackward,
            Self::Left => InputAction::MoveLeft,
            Self::Right => InputAction::MoveRight,
            Self::Jump => InputAction::Jump,
            Self::Interact => InputAction::Interact,
        }
    }
}

struct PlaySource {
    scene: Scene,
    scripts: HashMap<Uuid, String>,
    digest: String,
}

impl PlaySource {
    fn load(path: &Path) -> Result<Self, ErrorData> {
        let source = fs::read_to_string(path).map_err(io_error)?;
        let mut signature = Fingerprint::new();
        signature.add(source.as_bytes());
        let scene = Scene::decode(&source).map_err(io_error)?;
        let ids: HashSet<_> = scene
            .referenced_objects()
            .filter_map(|object| object.script)
            .collect();
        let mut scripts = HashMap::new();
        for asset in scene.assets.iter().filter(|asset| ids.contains(&asset.id)) {
            if !scene.script_sources.contains_key(&asset.id) {
                let bytes = futty_assets::read_saved(path, asset).map_err(io_error)?;
                let source = futty_assets::script_source(&bytes).map_err(io_error)?;
                signature.add(asset.id.as_bytes());
                signature.add(&bytes);
                scripts.insert(asset.id, source.to_owned());
            }
        }
        Ok(Self {
            scene,
            scripts,
            digest: signature.finish(),
        })
    }
}

struct Session {
    scene: Scene,
    runtime: Runtime,
    camera_index: usize,
    tick: u32,
    held: [bool; InputAction::ALL.len()],
    requested_scene: Option<String>,
    replay: Replay,
}

impl Session {
    fn load(path: &Path, scene_path: String, max_ticks: u32, seed: u32) -> Result<Self, ErrorData> {
        if !(1..=MAX_TICKS).contains(&max_ticks) {
            return Err(invalid("max_ticks must be between 1 and 36000"));
        }
        Self::from_source(PlaySource::load(path)?, scene_path, max_ticks, seed)
    }

    fn from_source(
        source: PlaySource,
        scene_path: String,
        max_ticks: u32,
        seed: u32,
    ) -> Result<Self, ErrorData> {
        let PlaySource {
            scene,
            scripts,
            digest,
        } = source;
        let camera_index = scene
            .camera_index(None)
            .filter(|&index| {
                scene.entities[index].character.is_some() && scene.training_range.is_none()
            })
            .ok_or_else(|| {
                invalid(
                    "Play requires a main camera with a character controller in a standard scene",
                )
            })?;
        let mut runtime = Runtime::new(&scene).map_err(io_error)?;
        runtime.start_seeded_scripts(&scene, &scripts, seed);
        runtime.prepare_play();
        let requested_scene = runtime.take_scene_request();
        let mut session = Self {
            scene,
            runtime,
            camera_index,
            tick: 0,
            held: [false; InputAction::ALL.len()],
            requested_scene,
            replay: Replay::new(scene_path, digest, seed, max_ticks),
        };
        session.record_tick();
        Ok(session)
    }

    fn status(&self) -> Status {
        match self.runtime.game_outcome() {
            Some(GameOutcome::Success) => Status::Success,
            Some(GameOutcome::Failure) => Status::Failure,
            None if self.requested_scene.is_some() => Status::SceneTransition,
            None if self.tick >= self.replay.max_ticks => Status::TickLimit,
            None => Status::Running,
        }
    }

    fn step(&mut self, input: Controls) -> Result<(), ErrorData> {
        if self.status() != Status::Running {
            return Err(invalid("This play session has ended"));
        }
        let ticks = input.validate()?;
        if let Some([yaw, pitch]) = input.look
            && !self
                .runtime
                .look_character(self.scene.entities[self.camera_index].uuid, yaw, pitch)
        {
            return Err(io_error("The character camera is missing"));
        }
        let mut held = [false; InputAction::ALL.len()];
        for action in &input.actions {
            held[action.input() as usize] = true;
        }
        for index in 0..ticks.min(self.replay.max_ticks - self.tick) {
            let actions = std::array::from_fn(|action| ActionState {
                held: held[action],
                pressed: index == 0 && held[action] && !self.held[action],
                released: index == 0 && !held[action] && self.held[action],
            });
            self.runtime.game_input(GameInput {
                actions,
                ..Default::default()
            });
            self.runtime.tick(FIXED_STEP_SECONDS);
            self.tick += 1;
            self.requested_scene = self.runtime.take_scene_request();
            self.record_tick();
            if self.status() != Status::Running {
                break;
            }
        }
        self.held = held;
        self.replay.steps.push(input);
        Ok(())
    }

    fn hud(&self) -> BTreeMap<&str, Value> {
        self.scene
            .hud
            .iter()
            .filter(|widget| {
                matches!(widget.kind, HudKind::Text | HudKind::Gauge) && !widget.binding.is_empty()
            })
            .filter_map(|widget| {
                self.runtime
                    .hud_value(&widget.binding)
                    .map(|value| (widget.binding.as_str(), hud_value(value)))
            })
            .collect()
    }

    fn record_tick(&mut self) {
        let mut state: Vec<_> = self.runtime.extract().collect();
        state.sort_unstable_by_key(|entry| entry.0);
        let player = self.scene.entities[self.camera_index].uuid;
        let mut player_position = self.scene.entities[self.camera_index]
            .transform
            .translation
            .to_array();
        let mut signature = Fingerprint::new();
        for (id, transform, color) in state {
            if id == player {
                player_position = transform.translation.to_array();
            }
            signature.add(id.as_bytes());
            for value in transform
                .translation
                .to_array()
                .into_iter()
                .chain(transform.rotation.to_array())
                .chain(transform.scale.to_array())
                .chain(color)
            {
                signature.add(&value.to_bits().to_le_bytes());
            }
        }
        signature.add(json!(self.hud()).to_string().as_bytes());
        if let Some(target) = &self.requested_scene {
            signature.add(target.as_bytes());
        }
        for log in self.runtime.script_log() {
            signature.add(log.as_bytes());
        }
        self.replay.marks.push(Mark {
            digest: signature.finish(),
            player_position,
            status: self.status(),
        });
    }

    fn observe(&mut self, id: Uuid) -> Result<CallToolResult, ErrorData> {
        let mut observation = self.observation()?;
        observation["session_id"] = json!(id);
        Ok(CallToolResult::structured(observation))
    }

    fn observation(&mut self) -> Result<Value, ErrorData> {
        let state: HashMap<_, _> = self
            .runtime
            .extract()
            .map(|(id, transform, _)| (id, transform))
            .collect();
        let spawned = self.runtime.spawned_objects();
        let objects = if spawned.is_empty() {
            Cow::Borrowed(self.scene.entities.as_slice())
        } else {
            Cow::Owned(self.scene.entities.iter().chain(spawned).cloned().collect())
        };
        let hierarchy = Hierarchy::new(&objects).map_err(io_error)?;
        let local: Vec<_> = objects
            .iter()
            .map(|object| state.get(&object.uuid).copied().unwrap_or(object.transform))
            .collect();
        let worlds = hierarchy.world_matrices(|index| local[index].matrix());
        let camera = self
            .scene
            .camera_view(self.camera_index, &hierarchy, &worlds, &local)
            .ok_or_else(|| io_error("The character camera has an invalid pose"))?;
        let projection = camera.view_projection(16.0 / 9.0);
        let visible_objects = {
            let mut candidates: Vec<_> = objects
                .iter()
                .zip(worlds)
                .filter_map(|(object, world)| {
                    if !object.has_renderer() {
                        return None;
                    }
                    let position = world.w_axis.truncate();
                    let clip = projection * position.extend(1.0);
                    let from_eye = position - camera.eye;
                    let distance = from_eye.length();
                    (clip.w > 0.0
                        && clip.x.abs() <= clip.w
                        && clip.y.abs() <= clip.w
                        && clip.z >= 0.0
                        && clip.z <= clip.w
                        && distance > 0.001)
                        .then_some((distance, position, object))
                })
                .collect();
            candidates
                .sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.2.uuid.cmp(&b.2.uuid)));
            candidates
                .into_iter()
                .filter(|&(distance, position, object)| {
                    self.runtime
                        .raycast(camera.eye, position - camera.eye, distance, u32::MAX)
                        .is_none_or(|hit| hit.entity == object.uuid)
                })
                .take(MAX_VISIBLE)
                .map(|(distance, position, object)| {
                    json!({
                        "id": object.uuid,
                        "name": object.name,
                        "tags": object.tags,
                        "position": position.to_array(),
                        "distance": distance,
                    })
                })
                .collect::<Vec<_>>()
        };
        let status = self.status();
        Ok(json!({
            "scene": self.scene.name,
            "tick": self.tick,
            "remaining_ticks": self.replay.max_ticks - self.tick,
            "status": status,
            "requested_scene": self.requested_scene,
            "player": {
                "id": self.scene.entities[self.camera_index].uuid,
                "position": camera.eye.to_array(),
                "forward": camera.forward().to_array(),
            },
            "hud": self.hud(),
            "visible_objects": visible_objects,
            "actions": if status == Status::Running { &MOVEMENT_ACTIONS[..] } else { &[] },
        }))
    }
}

fn hud_value(value: ScriptValue) -> Value {
    match value {
        ScriptValue::Number(value) => json!(value),
        ScriptValue::Bool(value) => json!(value),
        ScriptValue::Text(value) => json!(value),
    }
}
