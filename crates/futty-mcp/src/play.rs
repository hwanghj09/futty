use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

use futty_runtime::{
    ActionState, CharacterMode, FIXED_STEP_SECONDS, GameInput, GameOutcome, Hierarchy, HudKind,
    InputAction, Object, Runtime, Scene, SceneDimension, ScriptValue,
};
use glam::Vec3;
use rmcp::{ErrorData, model::CallToolResult, schemars};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    authoring::invalid,
    ghost::{self, Persona},
    io_error,
    replay::{Fingerprint, MAX_TICKS, Mark, PlayTest, Replay},
    report::{self, Code, Context, Evidence, Stage},
};

const MAX_SESSIONS: usize = 4;
const MAX_STEP_TICKS: u32 = 120;
const MAX_VISIBLE: usize = 32;
const GHOST_STEP_TICKS: u32 = 6;
const PLAY_ACTIONS: [&str; 9] = [
    "forward", "backward", "left", "right", "jump", "interact", "fire", "reload", "retry",
];

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
            current.step(
                Controls {
                    actions: input.actions,
                    ticks: input.ticks,
                    look: input.look,
                    players: input.players,
                    tint: current.pending_tint,
                },
                |_| {},
            )?;
            current.pending_tint = None;
            current.observe(input.session_id)
        })
    }

    pub fn queue_tint(&self, input: HotReload) -> Result<CallToolResult, ErrorData> {
        self.call(move |sessions| {
            let current = session(sessions, input.session_id)?;
            if current.status() != Status::Running {
                return Err(invalid("This play session has ended"));
            }
            if current.tick != input.expected_tick {
                return Err(invalid(format!(
                    "Tick changed; current tick is {}",
                    current.tick
                )));
            }
            if current.pending_tint.is_some() {
                return Err(invalid(
                    "Advance Play to apply the queued tint before queuing another",
                ));
            }
            current.validate_tint(input.tint)?;
            current.pending_tint = Some(input.tint);
            Ok(CallToolResult::structured(json!({
                "object_id": input.tint.object_id,
                "color": input.tint.color,
                "applies_at_tick": current.tick + 1,
            })))
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
            if input.expect_hud.len() > 64
                || input.expect_collisions.len() > 64
                || input.expect_hud.values().any(|value| !value.is_number() && !value.is_boolean() && !value.is_string())
                || input.expect_scene.as_deref().is_some_and(|id| !futty_runtime::valid_scene_id(id))
                || input.expect_collisions.iter().any(|pair| pair.first.is_nil() || pair.second.is_nil() || pair.first == pair.second)
            {
                return Err(invalid("Invalid playtest expectations"));
            }
            for step in &input.steps {
                step.validate()?;
            }
            let mut context = Context::new(&input.path);
            context.seed = Some(input.seed);
            let mut current = Session::load(
                &path,
                input.path,
                input.max_ticks.unwrap_or(3600),
                input.seed,
            )?;
            let mut collision_ticks = vec![None; input.expect_collisions.len()];
            for step in input.steps {
                if current.status() != Status::Running {
                    break;
                }
                current.step(step, |state| {
                    for (pair, tick) in input.expect_collisions.iter().zip(&mut collision_ticks) {
                        if tick.is_none() && state.runtime.recent_physics_events().iter().any(|event| {
                            event.started
                                && pair.sensor.is_none_or(|sensor| sensor == event.sensor)
                                && ((pair.first == event.first && pair.second == event.second)
                                    || (pair.first == event.second && pair.second == event.first))
                        }) {
                            *tick = Some(state.tick);
                        }
                    }
                })?;
            }
            let status = current.status();
            let expected = input.expect.unwrap_or(Status::Success);
            let last_observation = current.observation()?;
            let mut checks = Vec::new();
            for (binding, expected) in input.expect_hud {
                let actual = &last_observation["hud"][&binding];
                let passed = if let (Some(actual), Some(expected)) = (actual.as_f64(), expected.as_f64()) {
                    actual == expected
                } else {
                    actual == &expected
                };
                checks.push(json!({"kind":"hud","binding":binding,"expected":expected,"actual":actual,"passed":passed}));
            }
            for (pair, tick) in input.expect_collisions.into_iter().zip(collision_ticks) {
                checks.push(json!({"kind":"collision","first":pair.first,"second":pair.second,"sensor":pair.sensor,"first_tick":tick,"passed":tick.is_some()}));
            }
            if let Some(scene) = input.expect_scene {
                checks.push(json!({"kind":"scene","expected":scene,"actual":current.requested_scene,"passed":current.requested_scene.as_deref() == Some(scene.as_str())}));
            }
            let passed = status == expected && checks.iter().all(|check| check["passed"] == true);
            context.tick = Some(current.tick);
            if let Some(file) = output
                && let Err(error) = current.replay.save(&file)
            {
                context.output_path = input.replay_path;
                return Err(report::execution(error, Stage::Playtest, context));
            }
            context.replay_path = input.replay_path.clone();
            let mut result = json!({
                "passed": passed,
                "checks": checks,
                "expected": expected,
                "status": status,
                "tick": current.tick,
                "replay_path": input.replay_path,
                "last_observation": last_observation,
            });
            if !passed {
                let (code, message) = if status != expected {
                    (Code::UnexpectedStatus, "Playtest ended with a different status")
                } else {
                    (Code::FailedChecks, "Playtest expectations did not match")
                };
                let failed_checks = checks.into_iter().filter(|check| check["passed"] == false).collect();
                result["failure_report"] = report::failure(
                    Stage::Playtest, code, message, context,
                    Some(Evidence::Playtest { expected, actual: status, failed_checks }),
                );
            }
            Ok(CallToolResult::structured(result))
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
                current.step(step, |_| {})?;
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

    pub fn ghost_batch(
        &self,
        path: PathBuf,
        input: ghost::Batch,
        output: PathBuf,
        max_ticks: u32,
    ) -> Result<CallToolResult, ErrorData> {
        self.call(move |_| {
            if output.symlink_metadata().is_ok() {
                return Err(invalid("Choose a new Ghost output directory"));
            }
            let source = PlaySource::load(&path)?;
            let parent = output.parent().ok_or_else(|| invalid("Ghost requires a parent directory"))?;
            let stage = tempfile::Builder::new().prefix(".futty-ghost-").tempdir_in(parent).map_err(io_error)?;
            let output_path = input.output_path.trim_end_matches(['/', '\\']);
            let tag = input.target_tag.as_deref().unwrap_or("coin");
            let mut runs = Vec::new();
            let mut summary = Vec::new();
            for &persona in &input.personas {
                let mut successes = 0;
                for &seed in &input.seeds {
                    let mut current = Session::from_source(source.clone(), input.path.clone(), max_ticks, seed)?;
                    let evidence = current.ghost_run(persona, tag)?;
                    let filename = format!("{}-{seed}.json", persona.label());
                    current.replay.save(&stage.path().join(&filename))?;
                    let status = current.status();
                    successes += usize::from(status == Status::Success);
                    let terminal = current.replay.marks.last().expect("Recorded initial tick");
                    runs.push(json!({
                        "persona":persona,
                        "seed":seed,
                        "status":status,
                        "tick":current.tick,
                        "reason":match status {
                            Status::Success => "game_reported_success",
                            Status::Failure => "game_reported_failure",
                            Status::TickLimit => "tick_budget_exhausted",
                            Status::SceneTransition => "scene_transition_requested",
                            Status::Running => unreachable!("Ghost runs until a terminal status"),
                        },
                        "first_failure_tick":matches!(status, Status::Failure | Status::TickLimit).then_some(current.tick),
                        "first_stall_tick":evidence,
                        "player_position":terminal.player_position,
                        "requested_scene":current.requested_scene,
                        "hud":current.hud(),
                        "decision_count":current.replay.steps.len(),
                        "replay_path":format!("{output_path}/{filename}"),
                    }));
                }
                summary.push(json!({"persona":persona,"runs":input.seeds.len(),"successes":successes}));
            }
            let result = json!({
                "format":1,
                "engine_version":env!("CARGO_PKG_VERSION"),
                "scene":input.path,
                "source_digest":source.digest,
                "target_tag":tag,
                "max_ticks":max_ticks,
                "report_path":format!("{output_path}/report.json"),
                "summary":summary,
                "runs":runs,
            });
            let report = fs::File::create(stage.path().join("report.json")).map_err(io_error)?;
            serde_json::to_writer_pretty(&report, &result).map_err(io_error)?;
            report.sync_all().map_err(io_error)?;
            drop(report);
            if output.symlink_metadata().is_ok() {
                return Err(invalid("Ghost output directory already exists"));
            }
            fs::rename(stage.path(), &output).map_err(io_error)?;
            Ok(CallToolResult::structured(result))
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
pub(crate) struct HotReload {
    #[schemars(with = "String")]
    session_id: Uuid,
    /// Last observed simulation tick.
    expected_tick: u32,
    tint: PlayTint,
}

#[derive(Clone, Copy, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlayTint {
    #[schemars(with = "String")]
    object_id: Uuid,
    /// Normalized RGB channels from 0 to 1.
    color: [f32; 3],
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
    #[serde(default)]
    players: Vec<PlayerControls>,
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
    Fire,
    Reload,
    Retry,
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
    /// Independent inputs for up to four characters in the same simulation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    players: Vec<PlayerControls>,
    /// Apply a visual tint before this step's first fixed tick.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tint: Option<PlayTint>,
}

#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlayerControls {
    #[schemars(with = "String")]
    player_id: Uuid,
    #[serde(default)]
    actions: Vec<Action>,
    look: Option<[f32; 2]>,
}

fn valid_look(look: Option<[f32; 2]>) -> bool {
    look.is_none_or(|angles| {
        angles
            .into_iter()
            .all(|angle| (-180.0..=180.0).contains(&angle))
    })
}

type HeldActions = [bool; InputAction::ALL.len()];

fn held_actions(actions: &[Action]) -> HeldActions {
    let mut held = [false; InputAction::ALL.len()];
    for action in actions {
        held[action.input() as usize] = true;
    }
    held
}

fn step_input(held: HeldActions, previous: HeldActions, first: bool) -> GameInput {
    GameInput {
        actions: std::array::from_fn(|index| ActionState {
            held: held[index],
            pressed: first && held[index] && !previous[index],
            released: first && !held[index] && previous[index],
        }),
        ..Default::default()
    }
}

impl Controls {
    pub fn validate(&self) -> Result<u32, ErrorData> {
        let ticks = self.ticks.unwrap_or(1);
        if !(1..=MAX_STEP_TICKS).contains(&ticks) || self.actions.len() > PLAY_ACTIONS.len() {
            return Err(invalid("Use 1 to 120 ticks and up to nine actions"));
        }
        if !valid_look(self.look) {
            return Err(invalid("Look angles must be between -180 and 180 degrees"));
        }
        if !self.players.is_empty() {
            let mut ids = HashSet::new();
            if self.players.len() > 4
                || !self.actions.is_empty()
                || self.look.is_some()
                || self.players.iter().any(|player| {
                    player.player_id.is_nil()
                        || !ids.insert(player.player_id)
                        || player.actions.len() > PLAY_ACTIONS.len()
                        || !valid_look(player.look)
                })
            {
                return Err(invalid(
                    "Provide up to four distinct player inputs without top-level actions or look",
                ));
            }
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
            Self::Fire => InputAction::Fire,
            Self::Reload => InputAction::Reload,
            Self::Retry => InputAction::Retry,
        }
    }
}

#[derive(Clone)]
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
    player_index: usize,
    tick: u32,
    held: HeldActions,
    player_held: BTreeMap<Uuid, HeldActions>,
    requested_scene: Option<String>,
    replay: Replay,
    pending_tint: Option<PlayTint>,
}

impl Session {
    fn ghost_run(&mut self, persona: Persona, tag: &str) -> Result<Option<u32>, ErrorData> {
        const STALL_TICKS: u32 = 30;
        const STALL_DISTANCE_SQUARED: f32 = 0.02 * 0.02;
        let mode = self.scene.entities[self.player_index]
            .character
            .expect("playable scene")
            .mode;
        let mut movement_start = None;
        let mut first_stall_tick = None;
        while self.status() == Status::Running {
            let actions = self.ghost_actions(persona, tag)?;
            let moving = actions.iter().any(|action| {
                matches!(
                    action,
                    Action::Forward | Action::Backward | Action::Left | Action::Right
                )
            });
            self.step(
                Controls {
                    actions,
                    ticks: Some(GHOST_STEP_TICKS),
                    look: None,
                    players: Vec::new(),
                    tint: None,
                },
                |state| {
                    let marks = &state.replay.marks;
                    if !moving {
                        movement_start = None;
                        return;
                    }
                    let (start_tick, start) = movement_start.get_or_insert_with(|| {
                        (
                            state.tick,
                            Vec3::from(marks[marks.len() - 2].player_position),
                        )
                    });
                    let position = Vec3::from(marks[marks.len() - 1].player_position);
                    let mut delta = position - *start;
                    if mode == CharacterMode::Spatial {
                        delta.y = 0.0;
                    } else if mode == CharacterMode::SideScroller {
                        delta.y = 0.0;
                        delta.z = 0.0;
                    }
                    if delta.length_squared() >= STALL_DISTANCE_SQUARED {
                        movement_start = Some((state.tick + 1, position));
                    } else if state.tick + 1 - *start_tick >= STALL_TICKS {
                        first_stall_tick.get_or_insert(*start_tick);
                    }
                },
            )?;
        }
        Ok(first_stall_tick)
    }

    fn ghost_actions(&mut self, persona: Persona, tag: &str) -> Result<Vec<Action>, ErrorData> {
        let seed = self.replay.seed;
        if persona == Persona::Beginner && self.tick < 8 + seed % 12 {
            return Ok(Vec::new());
        }
        let observation = self.observation()?;
        let controller = self.scene.entities[self.player_index]
            .character
            .expect("playable scene");
        let mode = controller.mode;
        if persona != Persona::Beginner
            && !observation["player"]["interaction"].is_null()
            && !self.held[InputAction::Interact as usize]
        {
            return Ok(vec![Action::Interact]);
        }
        if persona == Persona::Explorer && self.tick < 36 + seed % 12 {
            return Ok(vec![if seed.is_multiple_of(2) {
                Action::Left
            } else {
                Action::Right
            }]);
        }

        let position = Vec3::from(
            self.replay
                .marks
                .last()
                .expect("Recorded initial tick")
                .player_position,
        );
        let target = observation["visible_objects"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|object| {
                object["tags"]
                    .as_array()
                    .is_some_and(|tags| tags.iter().any(|value| value.as_str() == Some(tag)))
            })
            .filter_map(|object| {
                serde_json::from_value::<[f32; 3]>(object["position"].clone()).ok()
            })
            .map(|target| Vec3::from(target) - position)
            .min_by(|a, b| a.length_squared().total_cmp(&b.length_squared()));
        let mut actions = Vec::with_capacity(3);
        if mode == CharacterMode::SideScroller {
            let sign = if target.is_some_and(|delta| delta.x < 0.0) {
                -1.0
            } else {
                1.0
            };
            actions.push(if sign < 0.0 {
                Action::Left
            } else {
                Action::Right
            });
            if persona == Persona::GoalDriven && !self.held[InputAction::Jump as usize] {
                let feet = position - Vec3::Y * (controller.origin_height - 0.16);
                let lead = controller.radius
                    + controller.speed * FIXED_STEP_SECONDS * GHOST_STEP_TICKS as f32
                    + 0.12;
                let ground = |point| {
                    self.runtime
                        .raycast(point, Vec3::NEG_Y, 0.32, controller.collision_mask)
                        .is_some()
                };
                if ground(feet) && !ground(feet + Vec3::X * lead * sign) {
                    actions.push(Action::Jump);
                }
            }
        } else if let Some(delta) = target {
            let (side, ahead) = if mode == CharacterMode::TopDown2d {
                (delta.x, delta.y)
            } else {
                let forward =
                    serde_json::from_value::<[f32; 3]>(observation["player"]["forward"].clone())
                        .map(Vec3::from)
                        .unwrap_or(Vec3::NEG_Z);
                let forward = Vec3::new(forward.x, 0.0, forward.z)
                    .try_normalize()
                    .unwrap_or(Vec3::NEG_Z);
                (delta.dot(forward.cross(Vec3::Y)), delta.dot(forward))
            };
            for (distance, positive, negative) in [
                (side, Action::Right, Action::Left),
                (ahead, Action::Forward, Action::Backward),
            ] {
                if distance.abs() > 0.08 {
                    actions.push(if distance > 0.0 { positive } else { negative });
                }
            }
        } else {
            actions.push(Action::Forward);
        }
        Ok(actions)
    }

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
            .filter(|_| scene.training_range.is_none())
            .ok_or_else(|| invalid("Play requires a camera in a standard scene"))?;
        let player_index = scene
            .player_index()
            .ok_or_else(|| invalid("Play requires an object with a character controller"))?;
        let mut runtime = Runtime::new(&scene).map_err(io_error)?;
        runtime.start_seeded_scripts(&scene, &scripts, seed);
        runtime.prepare_play();
        let requested_scene = runtime.take_scene_request();
        let mut session = Self {
            scene,
            runtime,
            camera_index,
            player_index,
            tick: 0,
            held: [false; InputAction::ALL.len()],
            player_held: BTreeMap::new(),
            requested_scene,
            replay: Replay::new(scene_path, digest, seed, max_ticks),
            pending_tint: None,
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

    fn validate_tint(&self, tint: PlayTint) -> Result<(), ErrorData> {
        if !self
            .scene
            .entities
            .iter()
            .any(|object| object.uuid == tint.object_id && object.has_renderer())
        {
            return Err(invalid("Tint requires a saved object with a renderer"));
        }
        self.runtime
            .validate_play_tint(tint.object_id, tint.color)
            .map_err(invalid)
    }

    fn step(
        &mut self,
        input: Controls,
        mut after_tick: impl FnMut(&Self),
    ) -> Result<(), ErrorData> {
        if self.status() != Status::Running {
            return Err(invalid("This play session has ended"));
        }
        let ticks = input.validate()?;
        if let Some(tint) = input.tint {
            self.validate_tint(tint)?;
        }
        if input.players.iter().any(|player| {
            !self.scene.entities.iter().any(|object| {
                object.uuid == player.player_id
                    && object.character.is_some_and(|controller| {
                        player.look.is_none() || controller.mode.is_spatial()
                    })
            })
        }) {
            return Err(invalid(
                "Each player_id must identify a character; only spatial characters accept look",
            ));
        }
        for player in &input.players {
            if let Some([yaw, pitch]) = player.look {
                self.runtime.look_character(player.player_id, yaw, pitch);
            }
        }
        if let Some([yaw, pitch]) = input.look
            && !self
                .runtime
                .look_character(self.scene.entities[self.player_index].uuid, yaw, pitch)
        {
            return Err(invalid("Only a spatial character accepts look"));
        }
        if let Some(tint) = input.tint {
            self.runtime
                .set_play_tint(tint.object_id, tint.color)
                .map_err(invalid)?;
        }
        let held = held_actions(&input.actions);
        let player_held: BTreeMap<_, _> = input
            .players
            .iter()
            .map(|player| (player.player_id, held_actions(&player.actions)))
            .collect();
        let neutral = [false; InputAction::ALL.len()];
        for index in 0..ticks.min(self.replay.max_ticks - self.tick) {
            if player_held.is_empty() {
                self.runtime
                    .game_input(step_input(held, self.held, index == 0));
            } else {
                self.runtime.game_input(GameInput::default());
                for (&id, &actions) in &player_held {
                    let prior = self.player_held.get(&id).copied().unwrap_or(neutral);
                    self.runtime
                        .game_input_for(id, step_input(actions, prior, index == 0));
                }
                if index == 0 {
                    for (&id, &prior) in &self.player_held {
                        if !player_held.contains_key(&id) {
                            self.runtime
                                .game_input_for(id, step_input(neutral, prior, true));
                        }
                    }
                }
            }
            self.runtime.tick(FIXED_STEP_SECONDS);
            self.tick += 1;
            self.requested_scene = self.runtime.take_scene_request();
            self.record_tick();
            after_tick(self);
            if self.status() != Status::Running {
                break;
            }
        }
        self.held = held;
        self.player_held = player_held;
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
        let player = self.scene.entities[self.player_index].uuid;
        let mut player_position = self.scene.entities[self.player_index]
            .transform
            .translation
            .to_array();
        let mut signature = Fingerprint::new();
        for (id, transform, color) in state {
            if !self.runtime.is_active(id) {
                signature.add(b"inactive");
            }
            if id == player {
                player_position = transform.translation.to_array();
            }
            signature.add(id.as_bytes());
            if let Some(health) = self.runtime.health_state(id) {
                signature.add(&health.current.to_bits().to_le_bytes());
                signature.add(&health.max.to_bits().to_le_bytes());
            }
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
        if !self.runtime.game_state().is_empty() {
            signature.add(json!(self.runtime.game_state()).to_string().as_bytes());
        }
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
            .map(|(id, transform, color)| (id, (transform, color)))
            .collect();
        let sprites: HashMap<_, _> = self.runtime.animated_sprites().collect();
        let objects = self
            .scene
            .entities
            .iter()
            .chain(self.runtime.spawned_objects());
        let hierarchy = Hierarchy::from_objects(objects.clone()).map_err(io_error)?;
        let local: Vec<_> = objects
            .clone()
            .map(|object| {
                state
                    .get(&object.uuid)
                    .map_or(object.transform, |&(pose, _)| pose)
            })
            .collect();
        let worlds = hierarchy.world_matrices(|index| local[index].matrix());
        let camera = self
            .scene
            .camera_view(self.camera_index, &hierarchy, &worlds, &local)
            .ok_or_else(|| io_error("The character camera has an invalid pose"))?;
        let projection = camera.view_projection(16.0 / 9.0);
        let planar = self.scene.dimension == SceneDimension::TwoD;
        let visible_objects = {
            let mut candidates: Vec<_> = objects
                .zip(worlds)
                .filter_map(|(object, world)| {
                    if !object.has_renderer() || !self.runtime.is_active(object.uuid) {
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
            let nearest = |a: &(f32, Vec3, &Object), b: &(f32, Vec3, &Object)| {
                a.0.total_cmp(&b.0).then_with(|| a.2.uuid.cmp(&b.2.uuid))
            };
            if planar && candidates.len() > MAX_VISIBLE {
                candidates.select_nth_unstable_by(MAX_VISIBLE, nearest);
                candidates.truncate(MAX_VISIBLE);
            }
            candidates.sort_unstable_by(nearest);
            candidates
                .into_iter()
                .filter(|&(distance, position, object)| {
                    planar
                        || self
                            .runtime
                            .raycast(camera.eye, position - camera.eye, distance, u32::MAX)
                            .is_none_or(|hit| hit.entity == object.uuid)
                })
                .take(MAX_VISIBLE)
                .map(|(distance, position, object)| {
                    let mut entry = json!({
                        "id": object.uuid,
                        "name": object.name,
                        "tags": object.tags,
                        "position": position.to_array(),
                        "distance": distance,
                        "health": self.runtime.health_state(object.uuid),
                        "color": state.get(&object.uuid).map_or(object.color, |&(_, color)| color),
                    });
                    if let Some((sheet, time)) = sprites.get(&object.uuid).copied().or_else(|| {
                        object
                            .sprite
                            .and_then(|sprite| sprite.sheet)
                            .map(|sheet| (sheet, 0.0))
                    }) {
                        entry["sprite_frame"] = json!(sheet.frame_index(time));
                    }
                    entry
                })
                .collect::<Vec<_>>()
        };
        let primary = self.scene.entities[self.player_index].uuid;
        let player_pose = local[self.player_index];
        let focus_for = |id| {
            self.runtime
                .interaction_focus(id)
                .map(|(target, prompt)| json!({"object_id":target,"prompt":prompt}))
        };
        let focus = focus_for(primary);
        let players: Vec<_> = self.scene.entities.iter().filter(|object| object.character.is_some())
            .map(|object| {
                let pose = state.get(&object.uuid).map_or(object.transform, |&(pose, _)| pose);
                let interaction = if object.uuid == primary { focus.clone() } else { focus_for(object.uuid) };
                json!({
                    "id":object.uuid,"active":self.runtime.is_active(object.uuid),"position":pose.translation.to_array(),
                    "forward":(pose.rotation * Vec3::NEG_Z).to_array(),"interaction":interaction,
                    "health":self.runtime.health_state(object.uuid),
                })
            }).collect();
        let status = self.status();
        let game_state: BTreeMap<_, _> = self
            .runtime
            .game_state()
            .iter()
            .map(|(key, value)| (key, hud_value(value.clone())))
            .collect();
        Ok(json!({
            "scene": self.scene.name,
            "tick": self.tick,
            "remaining_ticks": self.replay.max_ticks - self.tick,
            "status": status,
            "requested_scene": self.requested_scene,
            "player": {
                "id": primary,
                "position": player_pose.translation.to_array(),
                "forward": (player_pose.rotation * Vec3::NEG_Z).to_array(),
                "interaction": focus,
                "health": self.runtime.health_state(primary),
            },
            "players": players,
            "hud": self.hud(),
            "game_state": game_state,
            "visible_objects": visible_objects,
            "actions": if status == Status::Running { &PLAY_ACTIONS[..] } else { &[] },
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
