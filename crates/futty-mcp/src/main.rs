mod authoring;
mod play;
mod replay;

use std::{
    fs::{self, File, OpenOptions},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

use authoring::{Batch, ObjectQuery, Undo, UndoRecord};
use futty_core::{Camera, CameraLens};
use futty_runtime::{Object, Scene};
use glam::Vec3;
use play::{PlayRunner, SessionId, Start, Step};
use replay::{PlayTest, Replay, ReplayFile, SaveReplay};
use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Clone)]
struct Futty {
    root: PathBuf,
    undo: Arc<Mutex<Option<UndoRecord>>>,
    play: PlayRunner,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SceneFile {
    /// Existing .ron scene path relative to the configured Futty workspace.
    path: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct NewScene {
    /// New .ron scene path relative to the workspace. Its parent directory must exist.
    path: String,
    name: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Background {
    Sky,
    Solid,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetBackground {
    /// Existing .ron scene path relative to the workspace.
    path: String,
    background: Background,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct AddCamera {
    /// Existing .ron scene path relative to the workspace.
    path: String,
    name: String,
    /// World-space camera position [x, y, z].
    position: [f32; 3],
    /// World-space point the camera should face [x, y, z].
    target: [f32; 3],
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetMainCamera {
    /// Existing .ron scene path relative to the workspace.
    path: String,
    /// Camera UUID from scene_inspect or scene_add_camera.
    camera_id: String,
}

#[tool_handler]
impl ServerHandler for Futty {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("futty-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions("Use workspace-relative .ron paths with forward slashes. Mutating tools edit saved scene files. Save and close an open scene in Futty before editing it through MCP, then reopen it.")
    }
}

#[tool_router]
impl Futty {
    #[tool(
        description = "Run a saved scene headlessly from an ordered list of actions and compare its final result to the expected status. Can save a replay."
    )]
    fn play_test(
        &self,
        Parameters(input): Parameters<PlayTest>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, true)?;
        let replay = input
            .replay_path
            .as_deref()
            .map(|path| self.workspace_file(path, "json", false))
            .transpose()?;
        self.play.test(path, input, replay)
    }

    #[tool(
        description = "Save a live or finished play session to a new workspace-relative .json replay file. Existing files are preserved."
    )]
    fn play_save_replay(
        &self,
        Parameters(input): Parameters<SaveReplay>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.workspace_file(&input.path, "json", false)?;
        self.play.save_replay(input.session_id, path)
    }

    #[tool(
        description = "Replay a saved .json recording and report the first different tick, if any. Scene inputs are verified and files are unchanged.",
        annotations(read_only_hint = true)
    )]
    fn play_replay(
        &self,
        Parameters(input): Parameters<ReplayFile>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.workspace_file(&input.path, "json", true)?;
        let replay = Replay::load(&path)?;
        let scene = self.scene_path(&replay.scene, true)?;
        self.play.replay(scene, replay)
    }

    #[tool(
        description = "Start a headless player session from a saved scene with a character camera. Returns the initial player observation."
    )]
    fn play_start(
        &self,
        Parameters(input): Parameters<Start>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, true)?;
        self.play.start(path, input)
    }

    #[tool(
        description = "Read the current player pose, visible objects, authored HUD values, tick and result without advancing time.",
        annotations(read_only_hint = true)
    )]
    fn play_observe(
        &self,
        Parameters(input): Parameters<SessionId>,
    ) -> Result<CallToolResult, ErrorData> {
        self.play.observe(input)
    }

    #[tool(
        description = "Apply a character camera look and hold the chosen movement or interact actions for 1 to 120 fixed ticks. Returns the next observation."
    )]
    fn play_step(&self, Parameters(input): Parameters<Step>) -> Result<CallToolResult, ErrorData> {
        self.play.step(input)
    }

    #[tool(
        description = "Close a play session and return its last tick and status. Saved scene files are unchanged."
    )]
    fn play_end(
        &self,
        Parameters(input): Parameters<SessionId>,
    ) -> Result<CallToolResult, ErrorData> {
        self.play.end(input)
    }

    #[tool(
        description = "Create a new Futty scene with the default cube and sky. Refuses to overwrite an existing file."
    )]
    fn scene_create(
        &self,
        Parameters(input): Parameters<NewScene>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, false)?;
        let mut scene = Scene::default();
        scene.name = input.name;
        scene.save_new(&path).map_err(io_error)?;
        Ok(scene_info(&scene))
    }

    #[tool(
        description = "Read a saved Futty scene's background, camera and object summaries, registered assets and prefabs.",
        annotations(read_only_hint = true)
    )]
    fn scene_inspect(
        &self,
        Parameters(input): Parameters<SceneFile>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, true)?;
        let scene = Scene::load(&path).map_err(io_error)?;
        Ok(scene_info(&scene))
    }

    #[tool(
        description = "List saved scene objects with UUIDs, transforms, components and asset references, optionally filtered by tag or UUID.",
        annotations(read_only_hint = true)
    )]
    fn scene_objects(
        &self,
        Parameters(input): Parameters<ObjectQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, true)?;
        let scene = Scene::load(&path).map_err(io_error)?;
        Ok(input.list(&scene))
    }

    #[tool(
        description = "Apply up to 64 ordered edits to a saved scene atomically. New objects can be referenced by key within the batch. Returns an undo token for the latest successful changed batch in this server session."
    )]
    fn scene_apply(
        &self,
        Parameters(input): Parameters<Batch>,
    ) -> Result<CallToolResult, ErrorData> {
        let _lock = self.lock_edits().map_err(io_error)?;
        let path = self.scene_path(&input.path, true)?;
        let before = Scene::load(&path).map_err(io_error)?;
        let mut after = before.clone();
        let mut objects = input.apply(&mut after)?;
        after.sync_prefabs(&before).map_err(authoring::invalid)?;
        objects.retain(|_, id| after.entities.iter().any(|object| object.uuid == *id));
        if before == after {
            return Ok(CallToolResult::structured(
                json!({"changed": false, "objects": objects}),
            ));
        }
        let mut undo = self.undo.lock().map_err(io_error)?;
        after.save(&path).map_err(io_error)?;
        let token = Uuid::new_v4();
        *undo = Some(UndoRecord {
            path,
            token,
            before,
            after,
        });
        Ok(CallToolResult::structured(json!({
            "changed": true,
            "objects": objects,
            "undo_token": token,
        })))
    }

    #[tool(
        description = "Undo the latest scene_apply batch for this server session if the scene has not changed since that batch."
    )]
    fn scene_undo(&self, Parameters(input): Parameters<Undo>) -> Result<CallToolResult, ErrorData> {
        let _lock = self.lock_edits().map_err(io_error)?;
        let path = self.scene_path(&input.path, true)?;
        let mut undo = self.undo.lock().map_err(io_error)?;
        let record = undo
            .as_ref()
            .filter(|record| record.path == path && record.token.to_string() == input.undo_token)
            .ok_or_else(|| authoring::invalid("No matching latest batch in this MCP session"))?;
        let current = Scene::load(&path).map_err(io_error)?;
        if current != record.after {
            return Err(authoring::invalid("The scene changed after the batch"));
        }
        record.before.save(&path).map_err(io_error)?;
        *undo = None;
        Ok(CallToolResult::structured(json!({"undone": true})))
    }

    #[tool(description = "Set sky or solid as the background of a saved Futty scene.")]
    fn scene_set_background(
        &self,
        Parameters(input): Parameters<SetBackground>,
    ) -> Result<CallToolResult, ErrorData> {
        self.edit_scene(&input.path, |scene| {
            scene.sky = matches!(input.background, Background::Sky);
            Ok(scene_info(scene))
        })
    }

    #[tool(
        description = "Add a perspective camera to a saved Futty scene and aim it at a world-space point. The first camera becomes the main camera."
    )]
    fn scene_add_camera(
        &self,
        Parameters(input): Parameters<AddCamera>,
    ) -> Result<CallToolResult, ErrorData> {
        let object = camera_object(input.name, input.position, input.target)?;
        self.edit_scene(&input.path, |scene| {
            let id = object.uuid;
            let main = scene.camera_index(None).is_none();
            if main {
                scene.main_camera = Some(id);
            }
            scene.entities.push(object);
            Ok(CallToolResult::structured(
                json!({"camera_id": id, "main": main}),
            ))
        })
    }

    #[tool(
        description = "Choose which existing camera is used to play or export a saved Futty scene."
    )]
    fn scene_set_main_camera(
        &self,
        Parameters(input): Parameters<SetMainCamera>,
    ) -> Result<CallToolResult, ErrorData> {
        let id = Uuid::parse_str(&input.camera_id)
            .map_err(|error| ErrorData::invalid_params(error.to_string(), None))?;
        self.edit_scene(&input.path, |scene| {
            if !scene
                .entities
                .iter()
                .any(|object| object.uuid == id && object.camera.is_some())
            {
                return Err(ErrorData::invalid_params(
                    "No camera with this UUID exists in the scene",
                    None,
                ));
            }
            scene.main_camera = Some(id);
            Ok(CallToolResult::structured(json!({"main_camera": id})))
        })
    }
}

impl Futty {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            undo: Arc::default(),
            play: PlayRunner::new(),
        }
    }

    fn scene_path(&self, value: &str, existing: bool) -> Result<PathBuf, ErrorData> {
        self.workspace_file(value, "ron", existing)
    }

    fn workspace_file(
        &self,
        value: &str,
        extension: &str,
        existing: bool,
    ) -> Result<PathBuf, ErrorData> {
        let relative = Path::new(value);
        if relative
            .extension()
            .and_then(|extension| extension.to_str())
            != Some(extension)
            || !relative
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err(authoring::invalid(format!(
                "Use a workspace-relative .{extension} path without parent traversal"
            )));
        }

        let joined = self.root.join(relative);
        let path = if existing {
            let file = joined.canonicalize().map_err(io_error)?;
            if !file.is_file()
                || file.extension().and_then(|extension| extension.to_str()) != Some(extension)
            {
                return Err(authoring::invalid(format!("Expected a .{extension} file")));
            }
            file
        } else {
            let parent = joined
                .parent()
                .unwrap_or(&self.root)
                .canonicalize()
                .map_err(io_error)?;
            parent.join(
                relative
                    .file_name()
                    .expect("a path with an extension has a name"),
            )
        };
        if !path.starts_with(&self.root) {
            return Err(ErrorData::invalid_params(
                "The path is outside the configured workspace",
                None,
            ));
        }
        Ok(path)
    }

    fn edit_scene(
        &self,
        relative: &str,
        edit: impl FnOnce(&mut Scene) -> Result<CallToolResult, ErrorData>,
    ) -> Result<CallToolResult, ErrorData> {
        let _lock = self.lock_edits().map_err(io_error)?;
        let path = self.scene_path(relative, true)?;
        let mut scene = Scene::load(&path).map_err(io_error)?;
        let result = edit(&mut scene)?;
        scene.save(&path).map_err(io_error)?;
        Ok(result)
    }

    fn lock_edits(&self) -> std::io::Result<File> {
        let target = self.root.join("target");
        fs::create_dir_all(&target)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(target.join("futty-mcp.lock"))?;
        lock.lock()?;
        Ok(lock)
    }
}

fn scene_info(scene: &Scene) -> CallToolResult {
    let cameras: Vec<_> = scene
        .entities
        .iter()
        .filter_map(|object| {
            object.camera.map(|lens| {
                json!({
                    "id": object.uuid,
                    "name": object.name,
                    "parent": object.parent,
                    "local_position": object.transform.translation,
                    "local_rotation": object.transform.rotation,
                    "lens": lens,
                })
            })
        })
        .collect();
    CallToolResult::structured(json!({
        "name": scene.name,
        "background": if scene.sky { "sky" } else { "solid" },
        "object_count": scene.entities.len(),
        "main_camera": scene.camera_index(None).map(|index| scene.entities[index].uuid),
        "cameras": cameras,
        "assets": scene.assets,
        "prefabs": scene.prefabs.iter().map(|(&id, prefab)| json!({"id": id, "name": prefab.name})).collect::<Vec<_>>(),
    }))
}

fn camera_object(name: String, position: [f32; 3], target: [f32; 3]) -> Result<Object, ErrorData> {
    let eye = Vec3::from(position);
    let Some(direction) = (Vec3::from(target) - eye).try_normalize() else {
        return Err(authoring::invalid(
            "Camera position and target must define a finite direction",
        ));
    };
    let mut object = Object::cube(name, eye);
    object.spin = false;
    object.camera = Some(CameraLens::default());
    object.transform.rotation = Camera {
        eye,
        yaw: direction.x.atan2(-direction.z),
        pitch: direction.y.clamp(-1.0, 1.0).asin(),
    }
    .view()
    .rotation;
    Ok(object)
}

fn io_error(error: impl std::fmt::Display) -> ErrorData {
    ErrorData::internal_error(error.to_string(), None)
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or("Usage: futty-mcp <workspace>")?;
    let root = PathBuf::from(root).canonicalize()?;
    if !root.is_dir() {
        return Err("Workspace must be a directory".into());
    }
    Futty::new(root).serve(stdio()).await?.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_test_saves_and_replays_the_first_different_tick() {
        let directory = tempfile::tempdir().unwrap();
        let server = Futty::new(directory.path().canonicalize().unwrap());
        let example =
            include_bytes!("../../../examples/agent-escape/Beacon Bot Escape.futty.scene.ron");
        let scene_path = server.root.join("escape.ron");
        fs::write(&scene_path, example).unwrap();
        let mut input: serde_json::Value =
            serde_json::from_str(include_str!("../../../examples/agent-escape/playtest.json"))
                .unwrap();
        input["path"] = json!("escape.ron");
        input["replay_path"] = json!("escape.json");
        let run = |args: &serde_json::Value| {
            server.play_test(Parameters(
                serde_json::from_value::<PlayTest>(args.clone()).unwrap(),
            ))
        };
        let result = run(&input).unwrap().structured_content.unwrap();
        assert_eq!(result["passed"], true, "{result}");
        assert_eq!(result["status"], "success");

        let replay = |path: &str| {
            server
                .play_replay(Parameters(ReplayFile { path: path.into() }))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let matched = replay("escape.json");
        assert_eq!(matched["matched"], true, "{matched}");
        assert_eq!(matched["tick"], result["tick"]);
        assert_eq!(fs::read(&scene_path).unwrap(), example);

        let trace = server.root.join("escape.json");
        let original = fs::read(&trace).unwrap();
        assert!(run(&input).is_err());
        assert_eq!(fs::read(&trace).unwrap(), original);
        let mut divergent: serde_json::Value = serde_json::from_slice(&original).unwrap();
        divergent["marks"][17]["digest"] = json!("0000000000000000");
        divergent["marks"][19]["digest"] = json!("0000000000000000");
        fs::write(
            server.root.join("divergent.json"),
            serde_json::to_vec(&divergent).unwrap(),
        )
        .unwrap();
        let mismatch = replay("divergent.json");
        assert_eq!(mismatch["reason"], "state_changed");
        assert_eq!(mismatch["first_different_tick"], 17);

        let mut scene = Scene::load(&scene_path).unwrap();
        scene.name = "Changed map".into();
        scene.save(&scene_path).unwrap();
        assert_eq!(replay("escape.json")["reason"], "source_changed");
    }

    #[test]
    fn ai_play_observes_and_completes_escape_without_writing_the_scene() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let server = Futty::new(root);
        let path = "examples/agent-escape/Beacon Bot Escape.futty.scene.ron";
        let before = fs::read(server.root.join(path)).unwrap();
        let start = |limit| {
            server
                .play_start(Parameters(
                    serde_json::from_value::<Start>(json!({"path": path, "max_ticks": limit}))
                        .unwrap(),
                ))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let observe = |id: &serde_json::Value| {
            server
                .play_observe(Parameters(
                    serde_json::from_value::<SessionId>(json!({"session_id": id})).unwrap(),
                ))
                .map(|result| result.structured_content.unwrap())
        };
        let step = |id: &serde_json::Value, expected_tick, actions: &[&str], ticks, look| {
            server.play_step(Parameters(serde_json::from_value::<Step>(
                json!({"session_id": id, "expected_tick": expected_tick, "actions": actions, "ticks": ticks, "look": look})
            ).unwrap())).map(|result| result.structured_content.unwrap())
        };
        let end = |id: &serde_json::Value| {
            server
                .play_end(Parameters(
                    serde_json::from_value::<SessionId>(json!({"session_id": id})).unwrap(),
                ))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let sees = |observation: &serde_json::Value, name: &str| {
            observation["visible_objects"]
                .as_array()
                .unwrap()
                .iter()
                .any(|object| object["name"] == name)
        };
        let tick = |observation: &serde_json::Value| observation["tick"].as_u64().unwrap();

        let initial = start(240);
        let id = &initial["session_id"];
        assert_eq!(initial["hud"]["objective"], "Find the amber switch [E]");
        assert_eq!(initial["tick"], 0);
        assert!(sees(&initial, "Amber switch"));
        assert!(!sees(&initial, "Exit strip"));
        assert_eq!(observe(id).unwrap(), initial);
        let turned = step(id, 0, &[], 1, Some([180.0, 0.0])).unwrap();
        assert!(!sees(&turned, "Amber switch"));
        assert!(step(id, 0, &["forward"], 60, None).is_err());
        assert_eq!(observe(id).unwrap(), turned);

        let approach = step(id, 1, &["forward"], 49, Some([180.0, 0.0])).unwrap();
        assert!(approach["player"]["position"][2].as_f64().unwrap() < 2.45);
        let opened = step(id, tick(&approach), &["interact"], 4, None).unwrap();
        assert_eq!(opened["hud"]["objective"], "Walk through the open door");
        let escaped = step(id, tick(&opened), &["forward"], 120, None).unwrap();
        assert_eq!(escaped["status"], "success");
        assert_eq!(escaped["hud"]["objective"], "ESCAPE COMPLETE");
        assert!(tick(&escaped) < tick(&opened) + 120);
        assert!(step(id, tick(&escaped), &[], 1, None).is_err());
        assert_eq!(end(id)["status"], "success");
        assert!(observe(id).is_err());

        let limited = start(1);
        let limited_id = &limited["session_id"];
        let limit = step(limited_id, 0, &[], 120, None).unwrap();
        assert_eq!(limit["status"], "tick_limit");
        assert_eq!(limit["tick"], 1);
        assert_eq!(end(limited_id)["tick"], 1);
        assert_eq!(fs::read(server.root.join(path)).unwrap(), before);
    }

    #[test]
    fn authored_escape_requires_a_nearby_interaction_before_crossing_the_door() {
        use futty_runtime::{GameInput, InputAction, Runtime, ScriptValue};
        use std::collections::HashMap;

        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/agent-escape/Beacon Bot Escape.futty.scene.ron");
        let scene = Scene::load(&path).unwrap();
        let player = scene.main_camera.unwrap();
        let mut runtime = Runtime::new(&scene).unwrap();
        runtime.start_scripts(&scene, &HashMap::new());
        let step = 1.0 / 60.0;
        let position = |runtime: &mut Runtime| {
            runtime
                .extract()
                .find(|(id, ..)| *id == player)
                .unwrap()
                .1
                .translation
        };
        let interact = |runtime: &mut Runtime| {
            let mut input = GameInput::default();
            input.actions[InputAction::Interact as usize].pressed = true;
            runtime.game_input(input);
            runtime.tick(step);
            runtime.game_input(GameInput::default());
            runtime.tick(step);
        };
        runtime.tick(step);
        interact(&mut runtime);
        assert_eq!(
            runtime.hud_value("objective"),
            Some(ScriptValue::Text("Move closer to the switch".into()))
        );
        let walk = |runtime: &mut Runtime, forward, count| {
            runtime.game_input(GameInput {
                forward,
                backward: !forward,
                ..Default::default()
            });
            for _ in 0..count {
                runtime.tick(step);
            }
        };
        walk(&mut runtime, true, 130);
        let closed = position(&mut runtime);
        assert!(closed.z > -2.12, "closed door position: {closed:?}");
        walk(&mut runtime, false, 42);
        let at_switch = position(&mut runtime);
        assert!(
            (0.25..2.45).contains(&at_switch.z),
            "closed door position: {closed:?}, after backtracking: {at_switch:?}"
        );
        interact(&mut runtime);
        assert_eq!(
            runtime.hud_value("objective"),
            Some(ScriptValue::Text("Walk through the open door".into()))
        );
        walk(&mut runtime, true, 125);
        runtime.tick(step);
        assert!(position(&mut runtime).z < -4.25);
        assert_eq!(
            runtime.hud_value("objective"),
            Some(ScriptValue::Text("ESCAPE COMPLETE".into()))
        );
        let logs: Vec<_> = runtime.script_log().collect();
        assert_eq!(logs.len(), 1, "{logs:?}");
        assert!(logs[0].contains("escape_complete"), "{logs:?}");
    }

    #[test]
    fn batch_resolves_keys_and_keeps_the_file_and_undo_safe_on_failure() {
        let temp = tempfile::tempdir().unwrap();
        let server = Futty::new(temp.path().canonicalize().unwrap());
        let path = "main.futty.scene.ron";
        server
            .scene_create(Parameters(NewScene {
                path: path.into(),
                name: "Main".into(),
            }))
            .unwrap();
        let load = || Scene::load(&server.root.join(path)).unwrap();
        let original = load();
        let apply = |edits| {
            server.scene_apply(Parameters(
                serde_json::from_value::<Batch>(json!({ "path": path, "edits": edits })).unwrap(),
            ))
        };
        let result = apply(json!([
            {"op":"add_object", "key":"parent", "name":"Structure"},
            {"op":"add_object", "key":"lamp", "name":"Lamp", "shape":"Cylinder", "properties":{"tags":["lamp"]}},
            {"op":"set_parent", "object":"lamp", "parent":"parent"},
            {"op":"set_script", "object":"parent", "name":"Ready", "source":"fn on_start(entity) { log(\"ready\"); }"}
        ])).unwrap().structured_content.unwrap();
        let saved = load();
        let parent: Uuid = serde_json::from_value(result["objects"]["parent"].clone()).unwrap();
        assert_eq!(saved.entities[2].parent, Some(parent));
        assert_eq!(saved.entities[2].primitive, futty_core::Primitive::Cylinder);
        assert!(
            saved
                .script_sources
                .contains_key(&saved.entities[1].script.unwrap())
        );
        let query = |args| {
            server
                .scene_objects(Parameters(
                    serde_json::from_value::<ObjectQuery>(args).unwrap(),
                ))
                .unwrap()
                .structured_content
                .unwrap()
        };
        assert_eq!(query(json!({"path":path,"tag":"lamp"}))["total"], 1);
        assert_eq!(query(json!({"path":path,"limit":1}))["next_offset"], 1);

        let disk_before = fs::read(server.root.join(path)).unwrap();
        assert!(
            apply(json!([
                {"op":"edit_object", "object":parent, "properties":{"name":"Partial edit"}},
                {"op":"edit_object", "object":parent, "properties":{"scale":[-1,1,1]}}
            ]))
            .is_err()
        );
        assert_eq!(fs::read(server.root.join(path)).unwrap(), disk_before);
        let undo = || {
            server.scene_undo(Parameters(
                serde_json::from_value::<Undo>(
                    json!({"path":path,"undo_token":result["undo_token"]}),
                )
                .unwrap(),
            ))
        };
        let mut external = saved.clone();
        external.sky = !external.sky;
        external.save(&server.root.join(path)).unwrap();
        assert!(undo().is_err());
        assert_eq!(load(), external);
        saved.save(&server.root.join(path)).unwrap();
        undo().unwrap();
        assert_eq!(load(), original);
        assert!(undo().is_err());
    }

    #[test]
    fn saves_camera_and_background_without_overwriting_on_create() {
        let temp = tempfile::tempdir().unwrap();
        let server = Futty::new(temp.path().canonicalize().unwrap());
        let path = "main.futty.scene.ron";
        let create = |name: &str| {
            server.scene_create(Parameters(NewScene {
                path: path.into(),
                name: name.into(),
            }))
        };
        create("Main").unwrap();
        assert!(create("Overwrite").is_err());
        assert!(server.scene_path("../outside.ron", false).is_err());

        server
            .scene_set_background(Parameters(SetBackground {
                path: path.into(),
                background: Background::Solid,
            }))
            .unwrap();
        server
            .scene_add_camera(Parameters(AddCamera {
                path: path.into(),
                name: "Camera".into(),
                position: [0.0, 0.0, 3.0],
                target: [0.0, 0.0, 0.0],
            }))
            .unwrap();

        let scene = Scene::load(&server.root.join(path)).unwrap();
        assert_eq!(scene.name, "Main");
        assert!(!scene.sky);
        let camera = &scene.entities[scene.camera_index(None).unwrap()];
        assert_eq!(scene.main_camera, Some(camera.uuid));
        assert!((camera.transform.rotation * Vec3::NEG_Z).abs_diff_eq(Vec3::NEG_Z, 1e-5));
    }
}
