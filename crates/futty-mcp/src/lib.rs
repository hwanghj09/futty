mod assets;
mod authoring;
mod capture;
mod components;
mod diff;
mod ghost;
mod interface;
mod layout;
mod play;
mod replay;
mod report;
mod review;
mod scripts;
mod surgery;
mod templates;

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
};

use assets::{CreateMaterial, ImportAsset, ImportAssetBatch, ReimportAsset, UpdateMaterial};
use authoring::{Batch, ObjectQuery, Undo, UndoRecord, invalid};
use components::ComponentQuery;
use futty_core::{AssetKind, Camera, CameraLens};
use futty_runtime::{
    CharacterFeel, HudWidget, InputAction, InputProfile, Material, Object, Scene, SceneDimension,
    SceneIssue,
    project::{Project, SceneList, is_project},
};
use glam::Vec3;
use interface::{BindHud, CreateHud, SetInput, UpdateHud};
use play::{PlayRunner, SessionId, Start, Step};
use replay::{PlayTest, Replay, ReplayFile, SaveReplay};
use report::{Context as ReportContext, Stage};
use rmcp::{
    ErrorData, ServerHandler, ServiceExt,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
};
use scripts::{AttachScript, CreateScript, ScriptQuery, UpdateScript, ValidateScript};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Clone)]
struct Futty {
    root: PathBuf,
    editor: Option<PathBuf>,
    undo: Arc<Mutex<Option<UndoRecord>>>,
    surgery_preview: Arc<Mutex<Option<surgery::Preview>>>,
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
struct FeelTarget {
    path: String,
    #[schemars(with = "String")]
    object_id: Uuid,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct FeelChange {
    path: String,
    #[schemars(with = "String")]
    object_id: Uuid,
    speed: Option<f32>,
    jump_speed: Option<f32>,
    /// Use the revision from feel_get or feel_preview. Required when applying.
    expected_revision: Option<String>,
}

impl From<FeelChange> for Batch {
    fn from(input: FeelChange) -> Self {
        Self::tune_character(
            input.path,
            input.expected_revision,
            input.object_id,
            input.speed,
            input.jump_speed,
        )
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct NewScene {
    /// New .ron scene path relative to the workspace. Its parent directory must exist.
    path: String,
    name: String,
    /// ThreeD creates the usual cube and sky. TwoD starts with an XY orthographic camera.
    #[serde(default)]
    #[schemars(schema_with = "scene_dimension_schema")]
    dimension: SceneDimension,
}

fn scene_dimension_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({ "type": "string", "enum": ["TwoD", "ThreeD"] })
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreatePrefab {
    path: String,
    #[schemars(with = "String")]
    object_id: Uuid,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpdatePrefab {
    path: String,
    #[schemars(with = "String")]
    prefab_id: Uuid,
    name: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlacePrefab {
    path: String,
    #[schemars(with = "String")]
    prefab_id: Uuid,
    position: [f32; 3],
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SceneTransition {
    /// Workspace-relative Futty project manifest.
    path: String,
    /// Registered source and destination scene IDs.
    from: String,
    to: String,
    /// Script host in the source scene. Must have no script assigned.
    #[schemars(with = "String")]
    object_id: Uuid,
    /// Existing HUD button binding. Omit to use the Interact action.
    button: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct NewProject {
    /// New workspace-relative .futty.project.ron path. Its parent must exist.
    path: String,
    /// Start scene ID from the scenes map.
    start: String,
    /// Scene IDs mapped to existing .ron paths relative to the project folder.
    scenes: BTreeMap<String, String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ProjectFile {
    /// Existing workspace-relative .futty.project.ron file.
    path: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct BuildProject {
    /// Existing workspace-relative .futty.project.ron manifest.
    path: String,
    /// New workspace-relative output directory. Its parent must exist.
    output_path: String,
    /// Workspace-relative Windows x64 Futty executable. Defaults to dist/futty-editor.exe.
    template_path: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetProjectStart {
    /// Existing workspace-relative .futty.project.ron file.
    path: String,
    /// Registered scene ID.
    start: String,
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
    /// Set an orthographic camera's vertical half-size in world units.
    orthographic_size: Option<f32>,
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
        description = "Return the JSON Schema for Futty game production failure reports.",
        annotations(read_only_hint = true)
    )]
    fn game_failure_schema(&self) -> CallToolResult {
        report::schema()
    }

    #[tool(
        description = "Review a saved Futty project through validation and a registered scene playtest. Optionally capture a PNG and package a Windows game after the test passes. Returns stage results and stops on the first failure."
    )]
    async fn game_review(
        &self,
        Parameters(input): Parameters<review::Review>,
    ) -> Result<CallToolResult, ErrorData> {
        let server = self.clone();
        tokio::task::spawn_blocking(move || review::run(&server, input))
            .await
            .map_err(io_error)?
    }

    #[tool(
        description = "Render a saved scene from a camera into a new PNG using a graphics adapter."
    )]
    async fn scene_capture(
        &self,
        Parameters(input): Parameters<capture::Capture>,
    ) -> Result<CallToolResult, ErrorData> {
        let server = self.clone();
        let context = ReportContext::output(&input.path, &input.output_path);
        tokio::task::spawn_blocking(move || server.capture_scene(input))
            .await
            .map_err(io_error)
            .and_then(|result| result)
            .map_err(|error| report::execution(error, Stage::CameraCapture, context))
    }

    #[tool(description = "Create a prefab from an object and its descendants in a saved scene.")]
    fn prefab_create(
        &self,
        Parameters(input): Parameters<CreatePrefab>,
    ) -> Result<CallToolResult, ErrorData> {
        self.edit_scene(&input.path, |scene| {
            let id = scene.create_prefab(input.object_id).map_err(invalid)?;
            Ok(CallToolResult::structured(
                json!({"prefab_id":id,"name":scene.prefabs[&id].name}),
            ))
        })
    }

    #[tool(
        description = "Place a linked prefab instance, returning the instance root UUID and undo token."
    )]
    fn prefab_instantiate(
        &self,
        Parameters(input): Parameters<PlacePrefab>,
    ) -> Result<CallToolResult, ErrorData> {
        self.scene_apply(Parameters(Batch::place_prefab(input)))
    }

    #[tool(
        description = "Refresh a prefab from its linked source and optionally rename it, updating its instances."
    )]
    fn prefab_update(
        &self,
        Parameters(input): Parameters<UpdatePrefab>,
    ) -> Result<CallToolResult, ErrorData> {
        self.edit_scene(&input.path, |scene| {
            let before = scene.clone();
            let prefab = scene
                .prefabs
                .get_mut(&input.prefab_id)
                .ok_or_else(|| invalid(format!("Unknown prefab {}", input.prefab_id)))?;
            if let Some(name) = input.name {
                prefab.name = name;
            }
            scene.sync_prefabs(&before).map_err(invalid)?;
            Ok(CallToolResult::structured(
                json!({"prefab_id":input.prefab_id,"name":scene.prefabs[&input.prefab_id].name}),
            ))
        })
    }

    #[tool(
        description = "Link two registered project scenes with a Rhai transition triggered by Interact or an existing HUD button."
    )]
    fn scene_transition_set(
        &self,
        Parameters(input): Parameters<SceneTransition>,
    ) -> Result<CallToolResult, ErrorData> {
        let _lock = self.lock_edits().map_err(io_error)?;
        let project = Project::load(&self.project_path(&input.path, true)?).map_err(io_error)?;
        let from = project.resolve(&input.from).map_err(invalid)?;
        let to = project.resolve(&input.to).map_err(invalid)?;
        Scene::load(&to).map_err(io_error)?;
        let before = Scene::load(&from).map_err(io_error)?;
        let mut scene = before.clone();
        let trigger = if let Some(button) = &input.button {
            if button.is_empty()
                || !scene.hud.iter().any(|widget| {
                    widget.kind == futty_runtime::HudKind::Button && widget.binding == *button
                })
            {
                return Err(invalid(
                    "The source scene must contain a HUD button with that binding",
                ));
            }
            format!("hud_pressed({})", json!(button))
        } else {
            "input_pressed(\"Interact\")".to_owned()
        };
        let index = scene
            .entities
            .iter()
            .position(|object| object.uuid == input.object_id)
            .ok_or_else(|| invalid("Unknown script host in the source scene"))?;
        if scene.entities[index].prefab.is_some() {
            return Err(invalid("Choose an object outside a linked prefab instance"));
        }
        let source = format!(
            "fn on_update(entity, dt) {{ if {trigger} {{ load_scene({}); }} }}",
            json!(input.to)
        );
        let script_id = if let Some(id) = scene.entities[index].script {
            if scene.script_sources.get(&id) != Some(&source) {
                return Err(invalid("The script host already has a different script"));
            }
            id
        } else {
            scripts::check(&source, &input.path, None)?;
            let id = scripts::register(&mut scene, "Scene transition".into(), source);
            scripts::assign(&mut scene.entities[index], Some(id));
            scene.sync_prefabs(&before).map_err(invalid)?;
            scene.save(&from).map_err(io_error)?;
            id
        };
        Ok(CallToolResult::structured(
            json!({"from":input.from,"to":input.to,"script_id":script_id}),
        ))
    }

    #[tool(
        description = "Run a saved scene headlessly and check status, final HUD values, collision starts and requested scene. Can save a replay."
    )]
    fn play_test(
        &self,
        Parameters(input): Parameters<PlayTest>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut context = ReportContext::new(&input.path);
        context.seed = Some(input.seed);
        let run = || {
            let path = self.scene_path(&input.path, true)?;
            let replay = input
                .replay_path
                .as_deref()
                .map(|path| self.workspace_file(path, "json", false))
                .transpose()?;
            self.play.test(path, input, replay)
        };
        run().map_err(|error| report::execution(error, Stage::Playtest, context))
    }

    #[tool(
        description = "Run deterministic Futty Ghost personas across several seeds. Writes a JSON report and one replay per run to a new directory."
    )]
    async fn ghost_batch_run(
        &self,
        Parameters(input): Parameters<ghost::Batch>,
    ) -> Result<CallToolResult, ErrorData> {
        let ticks = input.validate()?;
        let scene = self.scene_path(&input.path, true)?;
        let output = self.workspace_path(&input.output_path, false)?;
        let play = self.play.clone();
        tokio::task::spawn_blocking(move || play.ghost_batch(scene, input, output, ticks))
            .await
            .map_err(io_error)?
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
        description = "Create a new Futty scene. Defaults to ThreeD with a cube and sky. TwoD starts with an orthographic XY camera. Refuses to overwrite an existing file."
    )]
    fn scene_create(
        &self,
        Parameters(input): Parameters<NewScene>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, false)?;
        let mut scene = Scene::new(input.dimension);
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
        description = "Check a saved scene's format, hierarchy, settings and saved asset files without editing. Returns structured issues.",
        annotations(read_only_hint = true)
    )]
    fn scene_validate(
        &self,
        Parameters(input): Parameters<SceneFile>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, true).map_err(|error| {
            report::execution(
                error,
                Stage::SceneValidation,
                ReportContext::new(&input.path),
            )
        })?;
        let issues = scene_issues(&path);
        let result = json!({
            "path": input.path, "valid": issues.is_empty(), "issues": issues,
        });
        Ok(report::validation(
            result,
            Stage::SceneValidation,
            &input.path,
            issues,
            vec![],
        ))
    }

    #[tool(
        description = "Create an editor-compatible Futty project from saved scenes. Refuses to overwrite existing files."
    )]
    fn project_create(
        &self,
        Parameters(input): Parameters<NewProject>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.project_path(&input.path, false)?;
        let scenes = SceneList {
            start: input.start,
            scenes: input.scenes,
        };
        let project = Project::create_with_scenes(&path, scenes).map_err(io_error)?;
        Ok(project_info(&project, &input.path))
    }

    #[tool(
        description = "List built-in Futty game project templates.",
        annotations(read_only_hint = true)
    )]
    fn project_template_list(&self) -> CallToolResult {
        let templates: Vec<_> = templates::TEMPLATES
            .iter()
            .map(|template| {
                json!({
                    "id":template.id, "name":template.name, "description":template.description,
                })
            })
            .collect();
        CallToolResult::structured(json!({"templates":templates}))
    }

    #[tool(
        description = "Create an editor-ready Futty project from a built-in template in a new folder. Returns a ready-to-run playtest."
    )]
    fn game_create_from_template(
        &self,
        Parameters(input): Parameters<templates::CreateGame>,
    ) -> Result<CallToolResult, ErrorData> {
        let _lock = self.lock_edits().map_err(io_error)?;
        let output = self.workspace_path(&input.output_path, false)?;
        let template = templates::create(&input.template, &output)?;
        let folder = input.output_path.trim_end_matches(['/', '\\']);
        let scene = format!("{folder}/{}", template.scene_file());
        let steps: Vec<_> = template
            .steps
            .iter()
            .map(|(actions, ticks)| {
                json!({
                    "actions":actions, "ticks":ticks,
                })
            })
            .collect();
        Ok(CallToolResult::structured(json!({
            "template":input.template,
            "project_path":format!("{folder}/{}", template.project_file()),
            "scene_path":scene,
            "playtest":{
                "path":scene,
                "steps":steps,
                "expect":"success",
                "expect_hud":{(template.hud_expectation.0):template.hud_expectation.1},
            },
        })))
    }

    #[tool(
        description = "Read a Futty project's registered scene paths and current start scene.",
        annotations(read_only_hint = true)
    )]
    fn project_inspect(
        &self,
        Parameters(input): Parameters<ProjectFile>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.project_path(&input.path, true)?;
        let project = Project::load(&path).map_err(io_error)?;
        Ok(project_info(&project, &input.path))
    }

    #[tool(
        description = "Validate the project manifest and every registered scene, including saved asset contents and Rhai scripts.",
        annotations(read_only_hint = true)
    )]
    fn project_validate(
        &self,
        Parameters(input): Parameters<ProjectFile>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.project_path(&input.path, true).map_err(|error| {
            report::execution(
                error,
                Stage::ProjectValidation,
                ReportContext::new(&input.path),
            )
        })?;
        let project = match Project::load(&path) {
            Ok(project) => project,
            Err(error) => {
                let issues = vec![json!({"field": "manifest", "message": error.to_string()})];
                let result = json!({
                    "path": input.path, "valid": false, "scenes": [],
                    "issues": issues,
                });
                return Ok(report::validation(
                    result,
                    Stage::ProjectValidation,
                    &input.path,
                    issues,
                    vec![],
                ));
            }
        };
        let scenes: Vec<_> = project
            .list()
            .scenes
            .iter()
            .map(|(id, relative)| {
                let issues = match project.resolve(id) {
                    Ok(path) => scene_issues(&path),
                    Err(error) => vec![json!({"field":"path", "message":error.to_string()})],
                };
                json!({"scene_id":id,"path":relative,"valid":issues.is_empty(),"issues":issues})
            })
            .collect();
        let invalid_scenes: Vec<_> = scenes
            .iter()
            .filter(|scene| scene["valid"] == false)
            .cloned()
            .collect();
        let result = json!({
            "path": input.path, "start":project.list().start, "issues": [],
            "valid": invalid_scenes.is_empty(), "scenes": scenes,
        });
        Ok(report::validation(
            result,
            Stage::ProjectValidation,
            &input.path,
            vec![],
            invalid_scenes,
        ))
    }

    #[tool(
        description = "Package a saved multi-scene Futty project and a Windows x64 player into a new folder without opening the editor."
    )]
    async fn project_build(
        &self,
        Parameters(input): Parameters<BuildProject>,
    ) -> Result<CallToolResult, ErrorData> {
        let server = self.clone();
        let context = ReportContext::output(&input.path, &input.output_path);
        tokio::task::spawn_blocking(move || server.build_project(input))
            .await
            .map_err(io_error)
            .and_then(|result| result)
            .map_err(|error| report::execution(error, Stage::WindowsBuild, context))
    }

    #[tool(
        description = "Set an existing Futty project's start scene to a registered, loadable scene."
    )]
    fn project_set_start_scene(
        &self,
        Parameters(input): Parameters<SetProjectStart>,
    ) -> Result<CallToolResult, ErrorData> {
        let _lock = self.lock_edits().map_err(io_error)?;
        let path = self.project_path(&input.path, true)?;
        let mut project = Project::load(&path).map_err(io_error)?;
        project.set_start(&input.start).map_err(io_error)?;
        project.save(true).map_err(io_error)?;
        Ok(project_info(&project, &input.path))
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
        description = "List a saved scene's registered assets and shared materials.",
        annotations(read_only_hint = true)
    )]
    fn asset_list(
        &self,
        Parameters(input): Parameters<SceneFile>,
    ) -> Result<CallToolResult, ErrorData> {
        let scene = Scene::load(&self.scene_path(&input.path, true)?).map_err(io_error)?;
        Ok(CallToolResult::structured(
            json!({"assets": scene.assets, "materials": scene.materials}),
        ))
    }

    #[tool(
        description = "Validate and copy a workspace GLB, PNG, WAV or Ogg file into a saved scene's assets."
    )]
    fn asset_import(
        &self,
        Parameters(input): Parameters<ImportAsset>,
    ) -> Result<CallToolResult, ErrorData> {
        let _lock = self.lock_edits().map_err(io_error)?;
        let path = self.scene_path(&input.path, true)?;
        let mut scene = Scene::load(&path).map_err(io_error)?;
        let (source, kind) = self.asset_source(&input.source)?;
        let asset = assets::new_asset(&input.source, kind);
        let bytes = futty_assets::read(&source).map_err(io_error)?;
        assets::validate_asset(&scene, &asset, &bytes)?;
        let saved = futty_assets::save_new(&path, &asset, &bytes).map_err(io_error)?;
        scene.assets.push(asset.clone());
        if let Err(error) = scene.save(&path) {
            if let Err(cleanup) = fs::remove_file(&saved) {
                return Err(io_error(format!(
                    "{error}; could not remove the unregistered asset: {cleanup}"
                )));
            }
            return Err(io_error(error));
        }
        Ok(CallToolResult::structured(json!({"asset":asset})))
    }

    #[tool(
        description = "Import or replace up to 32 assets in a saved scene, preserving registered UUIDs and optionally binding them to objects, sprites, shared materials, or HUD images."
    )]
    fn asset_import_batch(
        &self,
        Parameters(input): Parameters<ImportAssetBatch>,
    ) -> Result<CallToolResult, ErrorData> {
        assets::import_batch(self, input)
    }

    #[tool(
        description = "Replace a saved GLB, PNG, WAV or Ogg with a validated file of the same kind, preserving its UUID."
    )]
    fn asset_reimport(
        &self,
        Parameters(input): Parameters<ReimportAsset>,
    ) -> Result<CallToolResult, ErrorData> {
        let _lock = self.lock_edits().map_err(io_error)?;
        let path = self.scene_path(&input.path, true)?;
        let scene = Scene::load(&path).map_err(io_error)?;
        let asset = scene
            .assets
            .iter()
            .find(|asset| asset.id == input.asset_id)
            .ok_or_else(|| invalid(format!("Unknown asset {}", input.asset_id)))?;
        let (source, kind) = self.asset_source(&input.source)?;
        if kind != asset.kind {
            return Err(invalid(
                "The replacement must match the registered asset kind",
            ));
        }
        let expected = assets::read_previous(&path, asset)?;
        let bytes = futty_assets::read(&source).map_err(io_error)?;
        assets::validate_asset(&scene, asset, &bytes)?;
        futty_assets::replace_saved(&path, asset, expected.as_deref(), &bytes).map_err(io_error)?;
        Ok(CallToolResult::structured(json!({"asset":asset})))
    }

    #[tool(
        description = "Create a shared material in a saved scene. Omitted settings use engine defaults."
    )]
    fn material_create(
        &self,
        Parameters(input): Parameters<CreateMaterial>,
    ) -> Result<CallToolResult, ErrorData> {
        self.edit_scene(&input.path, |scene| {
            let mut material = Material::default();
            input.settings.apply(&mut material);
            let id = Uuid::new_v4();
            let result =
                CallToolResult::structured(json!({"material_id": id,"material": material}));
            scene.materials.insert(id, material);
            Ok(result)
        })
    }

    #[tool(
        description = "Update selected settings of a shared material without changing its UUID."
    )]
    fn material_update(
        &self,
        Parameters(input): Parameters<UpdateMaterial>,
    ) -> Result<CallToolResult, ErrorData> {
        self.edit_scene(&input.path, |scene| {
            let material = scene
                .materials
                .get_mut(&input.material_id)
                .ok_or_else(|| invalid(format!("Unknown material {}", input.material_id)))?;
            input.settings.apply(material);
            Ok(CallToolResult::structured(
                json!({"material_id": input.material_id,"material": material}),
            ))
        })
    }

    #[tool(
        description = "Compile inline Rhai source or a workspace .rhai file and report fields or an issue with line and column.",
        annotations(read_only_hint = true)
    )]
    fn script_validate(
        &self,
        Parameters(input): Parameters<ValidateScript>,
    ) -> Result<CallToolResult, ErrorData> {
        let source = match (input.source, &input.path) {
            (Some(source), None) => source,
            (None, Some(path)) => {
                let path = self.workspace_file(path, "rhai", true)?;
                let bytes = futty_assets::read(&path).map_err(io_error)?;
                futty_assets::script_source(&bytes)
                    .map_err(invalid)?
                    .to_owned()
            }
            _ => return Err(invalid("Provide either source or path")),
        };
        let result = match futty_runtime::inspect_script_diagnostic(&source) {
            Ok(fields) => json!({"valid":true,"path":input.path,"fields":fields,"issues":[]}),
            Err(issue) => json!({"valid":false,"path":input.path,"issues":[issue]}),
        };
        Ok(CallToolResult::structured(result))
    }

    #[tool(
        description = "Read a registered Rhai script by UUID, including embedded source or its saved source file.",
        annotations(read_only_hint = true)
    )]
    fn script_get(
        &self,
        Parameters(input): Parameters<ScriptQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, true)?;
        let scene = Scene::load(&path).map_err(io_error)?;
        let asset = scripts::get_asset(&scene, input.script_id)?;
        let embedded = scene.script_sources.get(&asset.id);
        let bytes;
        let source = if let Some(source) = embedded {
            source.as_str()
        } else {
            bytes = futty_assets::read_saved(&path, asset).map_err(io_error)?;
            futty_assets::script_source(&bytes).map_err(invalid)?
        };
        Ok(CallToolResult::structured(json!({
            "script_id": asset.id, "name": asset.name, "source": source, "embedded": embedded.is_some(),
        })))
    }

    #[tool(description = "Validate and register a new embedded Rhai script in a saved scene.")]
    fn script_create(
        &self,
        Parameters(input): Parameters<CreateScript>,
    ) -> Result<CallToolResult, ErrorData> {
        let name = input.name.trim();
        if name.is_empty() || name.len() > 256 {
            return Err(invalid("Script name must contain 1 to 256 bytes"));
        }
        self.edit_scene(&input.path, |scene| {
            let fields = scripts::check(&input.source, &input.path, None)?;
            let id = scripts::register(scene, name.into(), input.source);
            Ok(CallToolResult::structured(
                json!({"script_id":id,"name":name,"fields":fields}),
            ))
        })
    }

    #[tool(
        description = "Validate and save new embedded source for a registered Rhai script, preserving the UUID and object bindings."
    )]
    fn script_update(
        &self,
        Parameters(input): Parameters<UpdateScript>,
    ) -> Result<CallToolResult, ErrorData> {
        self.edit_scene(&input.path, |scene| {
            scripts::get_asset(scene, input.script_id)?;
            let fields = scripts::check(&input.source, &input.path, Some(input.script_id))?;
            scene.script_sources.insert(input.script_id, input.source);
            Ok(CallToolResult::structured(
                json!({"script_id":input.script_id,"fields":fields}),
            ))
        })
    }

    #[tool(
        description = "Attach a registered Rhai script to an object or detach it. Returns an undo token for a changed scene."
    )]
    fn script_attach(
        &self,
        Parameters(input): Parameters<AttachScript>,
    ) -> Result<CallToolResult, ErrorData> {
        self.scene_apply(Parameters(Batch::attach_script(input)))
    }

    #[tool(
        description = "Read saved scene input overrides and built-in action defaults.",
        annotations(read_only_hint = true)
    )]
    fn input_get(
        &self,
        Parameters(input): Parameters<SceneFile>,
    ) -> Result<CallToolResult, ErrorData> {
        let scene = Scene::load(&self.scene_path(&input.path, true)?).map_err(io_error)?;
        let defaults = InputAction::ALL
            .into_iter()
            .map(|action| (action.name(), action.default_binding()))
            .collect::<BTreeMap<_, _>>();
        Ok(CallToolResult::structured(json!({
            "path": input.path, "input": scene.input, "defaults": defaults, "default_sensitivity": 1.0,
        })))
    }

    #[tool(
        description = "Replace the scene's input overrides. Unspecified actions and sensitivity use the player's settings."
    )]
    fn input_set(
        &self,
        Parameters(input): Parameters<SetInput>,
    ) -> Result<CallToolResult, ErrorData> {
        let profile = InputProfile {
            bindings: input.bindings,
            sensitivity: input.sensitivity,
            capture_mouse: input.capture_mouse,
        };
        profile.validate().map_err(invalid)?;
        self.edit_scene(&input.path, |scene| {
            scene.input = profile;
            Ok(CallToolResult::structured(
                json!({"path": input.path, "input": scene.input}),
            ))
        })
    }

    #[tool(
        description = "List HUD widgets in drawing order for a saved scene.",
        annotations(read_only_hint = true)
    )]
    fn hud_list(
        &self,
        Parameters(input): Parameters<SceneFile>,
    ) -> Result<CallToolResult, ErrorData> {
        let scene = Scene::load(&self.scene_path(&input.path, true)?).map_err(io_error)?;
        Ok(CallToolResult::structured(json!({"widgets": scene.hud})))
    }

    #[tool(description = "Add a Text, Image, Button or Gauge widget to a saved scene.")]
    fn hud_create(
        &self,
        Parameters(input): Parameters<CreateHud>,
    ) -> Result<CallToolResult, ErrorData> {
        self.edit_scene(&input.path, |scene| {
            let mut widget = HudWidget::new(input.kind);
            input.settings.apply(&mut widget);
            let result = CallToolResult::structured(json!({"widget": widget}));
            scene.hud.push(widget);
            Ok(result)
        })
    }

    #[tool(
        description = "Update selected widget settings while retaining the widget UUID and draw order."
    )]
    fn hud_update(
        &self,
        Parameters(input): Parameters<UpdateHud>,
    ) -> Result<CallToolResult, ErrorData> {
        self.edit_hud(&input.path, input.widget_id, |widget| {
            input.settings.apply(widget)
        })
    }

    #[tool(description = "Bind a HUD widget to a Rhai value key or button action.")]
    fn hud_bind(
        &self,
        Parameters(input): Parameters<BindHud>,
    ) -> Result<CallToolResult, ErrorData> {
        self.edit_hud(&input.path, input.widget_id, |widget| {
            widget.binding = input.binding
        })
    }

    #[tool(
        description = "Read an object's light, body, character, interaction or health component. Returns null when absent.",
        annotations(read_only_hint = true)
    )]
    fn component_get(
        &self,
        Parameters(input): Parameters<ComponentQuery>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, true)?;
        let scene = Scene::load(&path).map_err(io_error)?;
        let object = scene
            .entities
            .iter()
            .find(|object| object.uuid == input.object_id)
            .ok_or_else(|| authoring::invalid(format!("Unknown object {}", input.object_id)))?;
        Ok(CallToolResult::structured(json!({
            "object_id": object.uuid,
            "value": input.component.read(object),
        })))
    }

    #[tool(
        description = "Read a character's move and jump speed, limits, presets, and saved scene revision.",
        annotations(read_only_hint = true)
    )]
    fn feel_get(
        &self,
        Parameters(input): Parameters<FeelTarget>,
    ) -> Result<CallToolResult, ErrorData> {
        let (scene, revision) = authoring::read_revision(&self.scene_path(&input.path, true)?)?;
        let controller = scene
            .entities
            .iter()
            .find(|object| object.uuid == input.object_id)
            .and_then(|object| object.character)
            .ok_or_else(|| invalid("Object must already have a character controller"))?;
        let presets: BTreeMap<_, _> = CharacterFeel::PRESETS.into_iter().collect();
        let range = |limits: std::ops::RangeInclusive<f32>| [*limits.start(), *limits.end()];
        Ok(CallToolResult::structured(json!({
            "object_id": input.object_id,
            "revision": revision,
            "mode": controller.mode,
            "values": CharacterFeel::read(controller),
            "parameters": {
                "speed": {"unit": "world_units_per_second", "range": range(CharacterFeel::SPEED_RANGE), "default": CharacterFeel::DEFAULT.speed},
                "jump_speed": {"unit": "world_units_per_second", "range": range(CharacterFeel::JUMP_RANGE), "default": CharacterFeel::DEFAULT.jump_speed, "enabled": CharacterFeel::supports_jump(controller.mode, scene.dimension)},
            },
            "presets": presets,
        })))
    }

    #[tool(
        description = "Validate proposed character feel values and show the scene diff without saving.",
        annotations(read_only_hint = true)
    )]
    fn feel_preview(
        &self,
        Parameters(input): Parameters<FeelChange>,
    ) -> Result<CallToolResult, ErrorData> {
        self.scene_preview_ops(Parameters(input.into()))
    }

    #[tool(
        description = "Apply move or jump speed to a saved character using the revision from feel_get or feel_preview. Returns a scene_undo token if changed."
    )]
    fn feel_apply(
        &self,
        Parameters(input): Parameters<FeelChange>,
    ) -> Result<CallToolResult, ErrorData> {
        if input.expected_revision.is_none() {
            return Err(invalid(
                "Provide expected_revision from feel_get or feel_preview",
            ));
        }
        self.scene_apply(Parameters(input.into()))
    }

    #[tool(
        description = "Preview an ordered scene_apply batch without saving. Returns validation issues, the scene revision, and the proposed changes.",
        annotations(read_only_hint = true)
    )]
    fn scene_preview_ops(
        &self,
        Parameters(input): Parameters<Batch>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, true)?;
        let relative = input.path.clone();
        let (before, revision) = input.read(&path)?;
        let result = match input.prepare(&before) {
            Ok((after, objects)) => json!({
                "valid":true, "path":relative, "revision":revision, "changed":before != after,
                "preview_objects":objects, "changes":diff::changes(&before, &after),
            }),
            Err(error) => json!({
                "valid":false, "path":relative, "revision":revision,
                "issues":[error.data.as_ref().and_then(|data| data.get("issue")).cloned()
                    .unwrap_or_else(|| json!({"message":error.message}))],
            }),
        };
        Ok(CallToolResult::structured(result))
    }

    #[tool(
        description = "Compare two saved scenes by UUID without writing either file.",
        annotations(read_only_hint = true)
    )]
    fn scene_diff(
        &self,
        Parameters(input): Parameters<diff::Compare>,
    ) -> Result<CallToolResult, ErrorData> {
        let before = Scene::load(&self.scene_path(&input.before_path, true)?).map_err(io_error)?;
        let after = Scene::load(&self.scene_path(&input.after_path, true)?).map_err(io_error)?;
        Ok(CallToolResult::structured(json!({
            "before_path":input.before_path,"after_path":input.after_path,
            "changed":before != after,"changes":diff::changes(&before,&after),
        })))
    }

    #[tool(
        description = "Apply up to 64 ordered edits to a saved scene atomically, including setting or removing components. New objects can be referenced by key within the batch. Returns an undo token for the latest successful changed batch in this server session."
    )]
    fn scene_apply(
        &self,
        Parameters(input): Parameters<Batch>,
    ) -> Result<CallToolResult, ErrorData> {
        let _lock = self.lock_edits().map_err(io_error)?;
        let path = self.scene_path(&input.path, true)?;
        let (before, _) = input.read(&path)?;
        let (after, objects) = input.prepare(&before)?;
        let token = self.save_undoable(path, before, after)?;
        Ok(CallToolResult::structured(match token {
            Some(token) => json!({"changed":true,"objects":objects,"undo_token":token}),
            None => json!({"changed":false,"objects":objects}),
        }))
    }

    #[tool(
        description = "Preview an isolated color or Rhai script change on one saved object. Returns affected IDs, the diff, and a token for surgery_apply.",
        annotations(read_only_hint = true)
    )]
    fn surgery_preview(
        &self,
        Parameters(input): Parameters<surgery::Change>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, true)?;
        let (before, revision) = authoring::read_revision(&path)?;
        let (after, mut result) = input.prepare(&before)?;
        let token = Uuid::new_v4();
        result["preview_token"] = json!(token);
        result["revision"] = json!(revision);
        *self.surgery_preview.lock().map_err(io_error)? = Some(surgery::Preview {
            path,
            revision,
            token,
            after,
        });
        Ok(CallToolResult::structured(result))
    }

    #[tool(
        description = "Commit the latest surgery preview in this MCP session if its saved scene is still at the previewed revision."
    )]
    fn surgery_apply(
        &self,
        Parameters(input): Parameters<surgery::Apply>,
    ) -> Result<CallToolResult, ErrorData> {
        let _lock = self.lock_edits().map_err(io_error)?;
        let path = self.scene_path(&input.path, true)?;
        let mut pending = self.surgery_preview.lock().map_err(io_error)?;
        let preview = pending
            .as_ref()
            .filter(|preview| preview.path == path && preview.token == input.preview_token)
            .ok_or_else(|| invalid("No matching latest Surgery preview in this MCP session"))?;
        let (before, revision) = authoring::read_revision(&path)?;
        if revision != preview.revision {
            return Err(invalid(
                "Scene changed since the Surgery preview; preview again",
            ));
        }
        let token = self.save_undoable(path, before, preview.after.clone())?;
        *pending = None;
        Ok(CallToolResult::structured(match token {
            Some(token) => json!({"changed":true,"undo_token":token}),
            None => json!({"changed":false}),
        }))
    }

    #[tool(
        description = "Roll back the latest committed Surgery change using its undo token if the scene has not changed."
    )]
    fn surgery_rollback(&self, input: Parameters<Undo>) -> Result<CallToolResult, ErrorData> {
        self.scene_undo(input)
    }

    #[tool(
        description = "Queue a tint on one saved object in a running headless Play session. The next play_step applies it at a fixed tick boundary and records it for replay."
    )]
    fn surgery_hot_reload(
        &self,
        Parameters(input): Parameters<play::HotReload>,
    ) -> Result<CallToolResult, ErrorData> {
        self.play.queue_tint(input)
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
        let mut object = camera_object(input.name, input.position, input.target)?;
        set_orthographic(&mut object, input.orthographic_size)?;
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
            editor: None,
            undo: Arc::default(),
            surgery_preview: Arc::default(),
            play: PlayRunner::new(),
        }
    }

    fn save_undoable(
        &self,
        path: PathBuf,
        before: Scene,
        after: Scene,
    ) -> Result<Option<Uuid>, ErrorData> {
        if before == after {
            return Ok(None);
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
        Ok(Some(token))
    }

    fn scene_path(&self, value: &str, existing: bool) -> Result<PathBuf, ErrorData> {
        self.workspace_file(value, "ron", existing)
    }

    fn asset_source(&self, value: &str) -> Result<(PathBuf, AssetKind), ErrorData> {
        let kind = futty_assets::kind(Path::new(value))
            .filter(|kind| *kind != AssetKind::Script)
            .ok_or_else(|| invalid("Choose a workspace-relative GLB, PNG, WAV or Ogg file"))?;
        let extension = Path::new(value)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or(kind.extension());
        Ok((self.workspace_file(value, extension, true)?, kind))
    }

    fn project_path(&self, value: &str, existing: bool) -> Result<PathBuf, ErrorData> {
        let path = self.workspace_file(value, "ron", existing)?;
        if !is_project(Path::new(value)) || !is_project(&path) {
            return Err(authoring::invalid("Expected a .futty.project.ron file"));
        }
        Ok(path)
    }

    fn workspace_file(
        &self,
        value: &str,
        extension: &str,
        existing: bool,
    ) -> Result<PathBuf, ErrorData> {
        if Path::new(value)
            .extension()
            .and_then(|extension| extension.to_str())
            != Some(extension)
        {
            return Err(authoring::invalid(format!(
                "Use a workspace-relative .{extension} path"
            )));
        }
        let path = self.workspace_path(value, existing)?;
        if existing
            && (!path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some(extension))
        {
            return Err(invalid(format!("Expected a .{extension} file")));
        }
        Ok(path)
    }

    fn workspace_path(&self, value: &str, existing: bool) -> Result<PathBuf, ErrorData> {
        let relative = Path::new(value);
        let name = relative
            .file_name()
            .filter(|_| {
                relative
                    .components()
                    .all(|part| matches!(part, Component::Normal(_)))
            })
            .ok_or_else(|| {
                invalid("Use a non-empty workspace-relative path without parent traversal")
            })?;
        let joined = self.root.join(relative);
        let path = if existing {
            joined.canonicalize().map_err(io_error)?
        } else {
            let parent = joined
                .parent()
                .unwrap_or(&self.root)
                .canonicalize()
                .map_err(io_error)?;
            if !parent.is_dir() {
                return Err(invalid("The output parent must be a directory"));
            }
            parent.join(name)
        };
        if !path.starts_with(&self.root) {
            return Err(ErrorData::invalid_params(
                "The path is outside the configured workspace",
                None,
            ));
        }
        Ok(path)
    }

    fn capture_scene(&self, input: capture::Capture) -> Result<CallToolResult, ErrorData> {
        let path = self.scene_path(&input.path, true)?;
        let output = self.workspace_file(&input.output_path, "png", false)?;
        let ([width, height], camera_id) = capture::save(&path, &output, &input)?;
        Ok(CallToolResult::structured(json!({
            "output_path":input.output_path,"camera_id":camera_id,"width":width,"height":height,
        })))
    }

    fn build_project(&self, input: BuildProject) -> Result<CallToolResult, ErrorData> {
        let _lock = self.lock_edits().map_err(io_error)?;
        self.package_project(input)
    }

    fn package_project(&self, input: BuildProject) -> Result<CallToolResult, ErrorData> {
        let project = self.project_path(&input.path, true)?;
        let output = self.workspace_path(&input.output_path, false)?;
        let template = match (input.template_path, &self.editor) {
            (Some(path), _) => self.workspace_file(&path, "exe", true)?,
            (None, Some(editor)) => editor.clone(),
            (None, None) => self.workspace_file("dist/futty-editor.exe", "exe", true)?,
        };
        futty_build::validate_windows_template(&template).map_err(invalid)?;
        futty_build::export(&project, &output, Camera::default(), &template).map_err(io_error)?;
        let folder = input.output_path.trim_end_matches(['/', '\\']);
        Ok(CallToolResult::structured(json!({
            "output_path":folder,
            "executable":format!("{folder}/Game.exe"),
            "manifest":format!("{folder}/{}", futty_build::MANIFEST),
        })))
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

    fn edit_hud(
        &self,
        path: &str,
        id: Uuid,
        edit: impl FnOnce(&mut HudWidget),
    ) -> Result<CallToolResult, ErrorData> {
        self.edit_scene(path, |scene| {
            let widget = scene
                .hud
                .iter_mut()
                .find(|widget| widget.id == id)
                .ok_or_else(|| invalid(format!("Unknown HUD widget {id}")))?;
            edit(widget);
            Ok(CallToolResult::structured(json!({"widget": widget})))
        })
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

fn scene_issues(path: &Path) -> Vec<serde_json::Value> {
    let scene = match Scene::load(path) {
        Ok(scene) => scene,
        Err(error) => {
            return vec![match error.downcast_ref::<SceneIssue>() {
                Some(issue) => json!(issue),
                None => json!({"field": "scene", "message": error.to_string()}),
            }];
        }
    };
    scene
        .assets
        .iter()
        .filter_map(|asset| {
            let issue = if let Some(source) = scene.script_sources.get(&asset.id) {
                futty_runtime::inspect_script_diagnostic(source)
                    .err()
                    .map(|issue| json!(issue))
            } else {
                match futty_assets::read_saved(path, asset) {
                    Ok(bytes) if asset.kind == AssetKind::Script => {
                        match futty_assets::script_source(&bytes) {
                            Ok(source) => futty_runtime::inspect_script_diagnostic(source)
                                .err()
                                .map(|issue| json!(issue)),
                            Err(error) => Some(json!({"message":error.to_string()})),
                        }
                    }
                    Ok(bytes) => assets::validate_asset(&scene, asset, &bytes)
                        .err()
                        .map(|error| json!({"message":error.message})),
                    Err(error) => Some(json!({"message":error.to_string()})),
                }
            };
            issue.map(|mut issue| {
                issue["field"] = json!(if asset.kind == AssetKind::Script {
                    "script"
                } else {
                    "assets"
                });
                issue["asset_id"] = json!(asset.id);
                issue
            })
        })
        .collect()
}

fn project_info(project: &Project, path: &str) -> CallToolResult {
    CallToolResult::structured(json!({
        "path": path,
        "name": project.name(),
        "start": project.list().start,
        "scenes": project.list().scenes,
    }))
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
        "dimension": scene.dimension,
        "background": if scene.sky { "sky" } else { "solid" },
        "object_count": scene.entities.len(),
        "local_coop": scene.local_coop,
        "local_player_two": scene.local_player_two().map(|index| scene.entities[index].uuid),
        "main_camera": scene.camera_index(None).map(|index| scene.entities[index].uuid),
        "cameras": cameras,
        "assets": scene.assets,
        "prefabs": scene.prefabs.iter().map(|(&id, prefab)| json!({"id": id, "name": prefab.name})).collect::<Vec<_>>(),
    }))
}

fn set_orthographic(object: &mut Object, size: Option<f32>) -> Result<(), ErrorData> {
    if let Some(size) = size {
        let lens = CameraLens {
            projection: futty_core::Projection::Orthographic,
            size,
            ..CameraLens::default()
        };
        if !lens.valid() {
            return Err(invalid(
                "Orthographic half-size must be between 0.05 and 500",
            ));
        }
        object.camera = Some(lens);
    }
    Ok(())
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
pub async fn run(
    workspace: &Path,
    editor: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = workspace.canonicalize()?;
    if !root.is_dir() {
        return Err("Workspace must be a directory".into());
    }
    let mut server = Futty::new(root);
    server.editor = editor;
    server.serve(stdio()).await?.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ghost_compares_three_personas_across_three_seeds_and_replays_each_run() {
        let dir = tempfile::tempdir().unwrap();
        let server = Futty::new(dir.path().canonicalize().unwrap());
        let template = server
            .game_create_from_template(Parameters(templates::CreateGame {
                template: "coin_runner_2d".into(),
                output_path: "coin-runner".into(),
            }))
            .unwrap()
            .structured_content
            .unwrap();
        let batch = || {
            Parameters(
                serde_json::from_value(json!({
                    "path":template["scene_path"],
                    "output_path":"ghost-report",
                    "seeds":[11, 22, 33],
                    "max_ticks":360,
                }))
                .unwrap(),
            )
        };
        let result = server
            .ghost_batch_run(batch())
            .await
            .unwrap()
            .structured_content
            .unwrap();
        let runs = result["runs"].as_array().unwrap();
        assert_eq!(runs.len(), 9);
        assert_eq!(result["summary"][0]["successes"], 0, "{result:#}");
        assert_eq!(result["summary"][2]["successes"], 3, "{result:#}");
        for run in runs {
            let replay = server
                .play_replay(Parameters(ReplayFile {
                    path: run["replay_path"].as_str().unwrap().into(),
                }))
                .unwrap()
                .structured_content
                .unwrap();
            assert_eq!(replay["matched"], true, "{replay}");
            assert_eq!(replay["tick"], run["tick"]);
            if matches!(run["status"].as_str(), Some("failure" | "tick_limit")) {
                assert_eq!(run["first_failure_tick"], run["tick"]);
            }
        }
        assert!(server.root.join("ghost-report/report.json").is_file());
        assert!(server.ghost_batch_run(batch()).await.is_err());
    }

    #[tokio::test]
    async fn ghost_reports_when_a_wall_stalls_movement_before_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let server = Futty::new(dir.path().canonicalize().unwrap());
        let mut scene = Scene::new(SceneDimension::TwoD);
        let mut actor = Object::cube("Player".into(), Vec3::ZERO);
        actor.character = Some(futty_runtime::CharacterController {
            mode: futty_runtime::CharacterMode::TopDown2d,
            ..Default::default()
        });
        let mut wall = Object::cube("Wall".into(), Vec3::Y * 2.0);
        wall.transform.scale.x = 8.0;
        wall.body = Some(futty_runtime::Body {
            motion: futty_runtime::Motion::Fixed,
            ..Default::default()
        });
        scene.entities.extend([actor, wall]);
        scene.save_new(&server.root.join("wall.ron")).unwrap();
        let result = server
            .ghost_batch_run(Parameters(
                serde_json::from_value(json!({
                    "path":"wall.ron", "output_path":"results", "seeds":[42],
                    "personas":["goal_driven"], "max_ticks":60,
                }))
                .unwrap(),
            ))
            .await
            .unwrap()
            .structured_content
            .unwrap();
        let run = &result["runs"][0];
        assert_eq!(run["status"], "tick_limit");
        assert_eq!(run["first_failure_tick"], 60);
        assert!(
            run["first_stall_tick"]
                .as_u64()
                .is_some_and(|tick| (1..=30).contains(&tick)),
            "{run}"
        );
    }

    #[test]
    fn surgery_hot_reload_preserves_play_state_and_is_recorded_in_replay() {
        let dir = tempfile::tempdir().unwrap();
        let server = Futty::new(dir.path().canonicalize().unwrap());
        let path = "Escape.ron";
        let mut scene = Scene::decode(include_str!(
            "../../../examples/agent-escape/Beacon Bot Escape.futty.scene.ron"
        ))
        .unwrap();
        let player = scene.player_index().unwrap();
        scene.entities[player].health = Some(futty_runtime::Health::default());
        let script = scene.entities[player].script.unwrap();
        let code = scene.script_sources.get_mut(&script).unwrap();
        *code = code.replacen(
            "fn on_start(entity) {",
            "fn on_start(entity) { damage(entity, 7.0); game_set(\"stage\", 2);",
            1,
        );
        scene.save_new(&server.root.join(path)).unwrap();
        let source = fs::read(server.root.join(path)).unwrap();
        let start = || {
            server
                .play_start(Parameters(
                    serde_json::from_value(json!({
                        "path":path, "seed":42,
                    }))
                    .unwrap(),
                ))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let step = |session: &serde_json::Value, actions, ticks| {
            server
                .play_step(Parameters(
                    serde_json::from_value(json!({
                        "session_id":session, "actions":actions, "ticks":ticks,
                    }))
                    .unwrap(),
                ))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let edited = start();
        let control = start();
        let edited_id = &edited["session_id"];
        let control_id = &control["session_id"];
        let before = step(edited_id, ["forward"], 10);
        assert_eq!(before["player"]["health"]["current"], 93.0);
        assert_eq!(before["game_state"]["stage"], 2.0);
        assert_eq!(
            before["player"],
            step(control_id, ["forward"], 10)["player"]
        );
        let object = &before["visible_objects"][0];
        assert!(object["id"].is_string());
        let color = json!([1, 0, 1]);
        let queue = |tick, color: serde_json::Value| {
            server.surgery_hot_reload(Parameters(
                serde_json::from_value(json!({
                    "session_id":edited_id, "expected_tick":tick,
                    "tint":{"object_id":object["id"], "color":color},
                }))
                .unwrap(),
            ))
        };
        assert!(queue(9, color.clone()).is_err());
        assert!(queue(10, json!([1, 0, 2])).is_err());
        let queued = queue(10, color.clone())
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(queued["applies_at_tick"], 11);
        assert!(queue(10, color.clone()).is_err());
        let observe = server
            .play_observe(Parameters(
                serde_json::from_value(json!({
                    "session_id":edited_id,
                }))
                .unwrap(),
            ))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(observe, before);

        let after = step(edited_id, ["forward"], 1);
        let control = step(control_id, ["forward"], 1);
        for field in ["tick", "player", "hud", "game_state", "status"] {
            assert_eq!(after[field], control[field], "{field}");
        }
        let visible = after["visible_objects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == object["id"])
            .unwrap();
        assert_eq!(visible["color"], json!([1.0, 0.0, 1.0]));
        assert_eq!(fs::read(server.root.join(path)).unwrap(), source);

        let replay = "Play.json";
        server
            .play_save_replay(Parameters(
                serde_json::from_value(json!({
                    "session_id":edited_id, "path":replay,
                }))
                .unwrap(),
            ))
            .unwrap();
        let result = server
            .play_replay(Parameters(ReplayFile {
                path: replay.into(),
            }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(result["matched"], true, "{result}");
        assert_eq!(result["tick"], 11);
    }

    #[test]
    fn surgery_isolates_a_door_replacement_and_preserves_the_escape_playtest() {
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/agent-escape");
        let dir = tempfile::tempdir().unwrap();
        let server = Futty::new(dir.path().canonicalize().unwrap());
        let relative = "Escape.ron";
        let path = server.root.join(relative);
        let mut scene = Scene::load(&example.join("Beacon Bot Escape.futty.scene.ron")).unwrap();
        let door = scene
            .entities
            .iter()
            .position(|object| object.name == "Exit door")
            .unwrap();
        let id = scene.entities[door].uuid;
        let old_script = scene.entities[door].script.unwrap();
        let deck = scene
            .entities
            .iter_mut()
            .find(|object| object.name == "Deck")
            .unwrap();
        deck.script = Some(old_script);
        scene.save_new(&path).unwrap();
        let original = fs::read(&path).unwrap();
        let play = || {
            let mut input: PlayTest =
                serde_json::from_slice(&fs::read(example.join("playtest.json")).unwrap()).unwrap();
            input.path = relative.into();
            let result = server
                .play_test(Parameters(input))
                .unwrap()
                .structured_content
                .unwrap();
            assert_eq!(result["passed"], true, "{result}");
            (
                result["tick"].clone(),
                result["last_observation"]["player"].clone(),
            )
        };
        let baseline = play();
        let preview = |source: &str| {
            server.surgery_preview(Parameters(
                serde_json::from_value(json!({
                    "path":relative, "object_id":id, "color":[1.0,0.0,0.0],
                    "script":{"name":"Revised door", "source":source},
                }))
                .unwrap(),
            ))
        };
        let source = format!(
            "{}\nfn revision() {{ 2 }}",
            scene.script_sources[&old_script]
        );
        assert!(preview("fn broken(").is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        let result = preview(&source).unwrap().structured_content.unwrap();
        assert_eq!(result["affected_object_ids"], json!([id]));
        assert_eq!(result["other_objects_using_prior_script"], 1);
        assert_eq!(fs::read(&path).unwrap(), original);
        let apply = |preview: &serde_json::Value| {
            server.surgery_apply(Parameters(
                serde_json::from_value(json!({
                    "path":relative, "preview_token":preview["preview_token"],
                }))
                .unwrap(),
            ))
        };
        let mut externally_saved = original.clone();
        externally_saved.push(b'\n');
        fs::write(&path, &externally_saved).unwrap();
        assert!(apply(&result).is_err());
        assert_eq!(fs::read(&path).unwrap(), externally_saved);

        let result = preview(&source).unwrap().structured_content.unwrap();
        let applied = apply(&result).unwrap().structured_content.unwrap();
        assert!(apply(&result).is_err());
        let after = Scene::load(&path).unwrap();
        let renamed = after.entities[door].script.unwrap();
        assert_eq!(json!(renamed), result["new_script_id"]);
        let mut expected = scene.clone();
        expected.entities[door].color = [1.0, 0.0, 0.0];
        expected.entities[door].script = Some(renamed);
        expected.assets.push(after.assets.last().unwrap().clone());
        expected.script_sources.insert(renamed, source);
        assert!(after == expected);
        assert_eq!(play(), baseline);

        server
            .surgery_rollback(Parameters(Undo {
                path: relative.into(),
                undo_token: applied["undo_token"].as_str().unwrap().into(),
            }))
            .unwrap();
        assert!(Scene::load(&path).unwrap() == scene);
        assert_eq!(play(), baseline);
    }

    #[test]
    fn feel_preview_and_apply_share_validation_and_reject_stale_changes() {
        let dir = tempfile::tempdir().unwrap();
        let server = Futty::new(dir.path().canonicalize().unwrap());
        let path = "Feel.ron";
        let file = server.root.join(path);
        let mut scene = Scene::default();
        let object_id = scene.entities[0].uuid;
        let controller = futty_runtime::CharacterController::default();
        scene.entities[0].character = Some(controller);
        scene.save_new(&file).unwrap();
        let saved = fs::read(&file).unwrap();
        let get = server
            .feel_get(Parameters(FeelTarget {
                path: path.into(),
                object_id,
            }))
            .unwrap()
            .structured_content
            .unwrap();
        let revision = get["revision"].as_str().unwrap();
        let agile = CharacterFeel::PRESETS[2].1;
        let change = |speed, expected_revision| {
            Parameters(FeelChange {
                path: path.into(),
                object_id,
                speed: Some(speed),
                jump_speed: Some(agile.jump_speed),
                expected_revision,
            })
        };
        let preview = server
            .feel_preview(change(agile.speed, None))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(preview["revision"], revision);
        assert_eq!(preview["changed"], true);
        let invalid = server
            .feel_preview(change(51.0, None))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(invalid["valid"], false);
        assert_eq!(fs::read(&file).unwrap(), saved);
        assert!(server.feel_apply(change(agile.speed, None)).is_err());
        let applied = server
            .feel_apply(change(agile.speed, Some(revision.into())))
            .unwrap()
            .structured_content
            .unwrap();
        let after = Scene::load(&file).unwrap();
        let mut expected = controller;
        agile.apply(&mut expected).unwrap();
        assert_eq!(after.entities[0].character, Some(expected));
        assert!(
            server
                .feel_apply(change(3.0, Some(revision.into())))
                .is_err()
        );
        server
            .scene_undo(Parameters(Undo {
                path: path.into(),
                undo_token: applied["undo_token"].as_str().unwrap().into(),
            }))
            .unwrap();
        assert_eq!(
            Scene::load(&file).unwrap().entities[0].character,
            Some(controller)
        );
    }

    #[test]
    fn scene_preview_and_diff_show_changes_without_writing_and_apply_checks_revision() {
        let dir = tempfile::tempdir().unwrap();
        let server = Futty::new(dir.path().canonicalize().unwrap());
        let path = "Main.ron";
        let original = "Original.ron";
        let scene = Scene::default();
        let id = scene.entities[0].uuid;
        scene.save_new(&server.root.join(path)).unwrap();
        fs::copy(server.root.join(path), server.root.join(original)).unwrap();
        let bytes = fs::read(server.root.join(path)).unwrap();
        let preview = |edits| {
            server
                .scene_preview_ops(Parameters(
                    serde_json::from_value(json!({"path":path,"edits":edits})).unwrap(),
                ))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let edits = json!([
            {"op":"set_component","object":id,"component":{"kind":"health","max":20}},
            {"op":"add_object","key":"marker","name":"Marker"}
        ]);
        let planned = preview(edits.clone());
        assert_eq!(planned["valid"], true);
        assert_eq!(planned["changes"]["objects"]["added_count"], 1);
        assert_eq!(
            planned["changes"]["objects"]["updated"][0]["fields"]["health"]["after"]["max"],
            20.0
        );
        assert!(planned["preview_objects"]["marker"].is_string());
        assert!(server.undo.lock().unwrap().is_none());
        let invalid = preview(json!([
            {"op":"set_component","object":id,"component":{"kind":"health","max":-1}}
        ]));
        assert_eq!(invalid["valid"], false, "{invalid}");
        assert_eq!(invalid["issues"][0]["field"], "health");
        let bad_edit = preview(json!([{"op":"remove_leaf","object":"missing"}]));
        assert_eq!(bad_edit["issues"][0]["edit"], 1);
        assert_eq!(fs::read(server.root.join(path)).unwrap(), bytes);
        let apply = || {
            server.scene_apply(Parameters(
                serde_json::from_value(
                    json!({"path":path,"edits":edits,"expected_revision":planned["revision"]}),
                )
                .unwrap(),
            ))
        };
        assert_eq!(
            apply().unwrap().structured_content.unwrap()["changed"],
            true
        );
        let saved = fs::read(server.root.join(path)).unwrap();
        let rejected = apply().unwrap_err();
        assert_eq!(
            rejected.data.unwrap()["expected_revision"],
            planned["revision"]
        );
        assert_eq!(fs::read(server.root.join(path)).unwrap(), saved);
        let diff = server
            .scene_diff(Parameters(diff::Compare {
                before_path: original.into(),
                after_path: path.into(),
            }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(diff["changes"]["objects"]["added_count"], 1);
        assert_eq!(
            diff["changes"]["objects"]["updated"],
            planned["changes"]["objects"]["updated"]
        );
        assert_eq!(fs::read(server.root.join(original)).unwrap(), bytes);
    }

    #[tokio::test]
    async fn capture_and_build_errors_carry_the_versioned_report_without_losing_mcp_codes() {
        let dir = tempfile::tempdir().unwrap();
        let server = Futty::new(dir.path().canonicalize().unwrap());
        let schema = server.game_failure_schema().structured_content.unwrap();
        assert_eq!(schema["properties"]["schema_version"]["const"], 1);
        Scene::default()
            .save_new(&server.root.join("Main.ron"))
            .unwrap();
        let capture = server
            .scene_capture(Parameters(capture::Capture {
                path: "Main.ron".into(),
                output_path: "Preview.png".into(),
                camera_id: None,
                width: Some(0),
                height: None,
            }))
            .await
            .unwrap_err();
        assert_eq!(capture.code, invalid("").code);
        let data = capture.data.unwrap();
        assert_eq!(data["failure_report"]["stage"], "camera_capture");
        assert_eq!(data["failure_report"]["code"], "execution_failed");
        assert_eq!(
            data["failure_report"]["context"]["output_path"],
            "Preview.png"
        );
        assert!(!server.root.join("Preview.png").exists());

        let build = server
            .project_build(Parameters(BuildProject {
                path: "Missing.futty.project.ron".into(),
                output_path: "Windows".into(),
                template_path: None,
            }))
            .await
            .unwrap_err();
        let data = build.data.unwrap();
        assert_eq!(data["failure_report"]["stage"], "windows_build");
        assert_eq!(
            data["failure_report"]["context"]["path"],
            "Missing.futty.project.ron"
        );
        assert_eq!(data["failure_report"]["context"]["output_path"], "Windows");
        assert!(!server.root.join("Windows").exists());
    }

    #[test]
    fn health_component_applies_damage_heal_death_and_checkpoint_restore() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let server = Futty::new(root.clone());
        let path = "Health.ron";
        Scene::default().save_new(&root.join(path)).unwrap();
        let script = r#"
            fn on_start(entity) { this.deaths = 0; set_hud("deaths", 0); save_checkpoint(entity); }
            fn on_update(entity, dt) {
                if input_pressed("Interact") { damage(entity, 40); }
                if input_pressed("Jump") { heal(entity, 20); }
                if input_pressed("MoveForward") { damage(entity, 150); }
                if input_pressed("MoveRight") { restore_checkpoint(entity); }
            }
            fn on_damage(entity, source, amount) { set_hud("damage", amount); }
            fn on_heal(entity, source, amount) { set_hud("heal", amount); }
            fn on_death(entity, source) {
                this.deaths += 1;
                set_hud("deaths", this.deaths);
                let health = get_health(entity);
                set_hud("health", health.fraction);
            }
        "#;
        let mut edits = vec![
            json!({"op":"add_camera","key":"player","name":"Player","position":[0,2,4],"target":[0,2,0],"character":true}),
            json!({"op":"set_component","object":"player","component":{"kind":"health","max":100}}),
            json!({"op":"set_script","object":"player","name":"Health rules","source":script}),
        ];
        for (index, binding) in ["damage", "heal", "deaths", "health"]
            .into_iter()
            .enumerate()
        {
            edits.push(json!({"op":"add_hud_text","label":binding,"binding":binding,"offset":[12,index*30],"size":[100,26]}));
        }
        server
            .scene_apply(Parameters(
                serde_json::from_value(json!({"path":path,"edits":edits})).unwrap(),
            ))
            .unwrap();
        let initial = server
            .play_start(Parameters(
                serde_json::from_value(json!({"path":path})).unwrap(),
            ))
            .unwrap()
            .structured_content
            .unwrap();
        let step = |actions| {
            server
                .play_step(Parameters(
                    serde_json::from_value(json!({
                        "session_id":initial["session_id"],"actions":actions,"ticks":3
                    }))
                    .unwrap(),
                ))
                .unwrap()
                .structured_content
                .unwrap()
        };
        assert_eq!(initial["player"]["health"]["current"], 100.0);
        let damaged = step(["interact"]);
        assert_eq!(damaged["player"]["health"]["current"], 60.0);
        assert_eq!(damaged["hud"]["damage"], 40.0);
        let healed = step(["jump"]);
        assert_eq!(healed["player"]["health"]["current"], 80.0);
        assert_eq!(healed["hud"]["heal"], 20.0);
        let dead = step(["forward"]);
        assert_eq!(dead["player"]["health"]["current"], 0.0);
        assert_eq!(dead["hud"]["damage"], 80.0);
        assert_eq!(dead["hud"]["health"], 0.0);
        assert_eq!(dead["hud"]["deaths"], 1.0);
        assert_eq!(step(["jump"])["player"]["health"]["current"], 0.0);
        assert_eq!(step(["forward"])["hud"]["deaths"], 1.0);
        assert_eq!(step(["right"])["player"]["health"]["current"], 100.0);
    }

    #[test]
    fn scene_create_persists_two_d_and_defaults_omitted_dimension_to_three_d() {
        let dir = tempfile::tempdir().unwrap();
        let server = Futty::new(dir.path().canonicalize().unwrap());
        let create = |request| {
            server
                .scene_create(Parameters(serde_json::from_value(request).unwrap()))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let scene = create(json!({"path":"Flat.ron","name":"Flat","dimension":"TwoD"}));
        assert_eq!(scene["dimension"], "TwoD");
        assert_eq!(scene["background"], "solid");
        let path = server.root.join("Flat.ron");
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("dimension: TwoD")
        );
        let loaded = Scene::load(&path).unwrap();
        assert_eq!(loaded.dimension, SceneDimension::TwoD);
        let camera = &loaded.entities[loaded.camera_index(None).unwrap()];
        assert_eq!(
            camera.camera.unwrap().projection,
            futty_core::Projection::Orthographic
        );
        assert_eq!(camera.transform.translation, Vec3::Z * 10.0);
        assert_eq!(camera.transform.rotation * Vec3::NEG_Z, Vec3::NEG_Z);
        let info = server
            .scene_inspect(Parameters(SceneFile {
                path: "Flat.ron".into(),
            }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(info["dimension"], "TwoD");
        let space = create(json!({"path":"Space.ron","name":"Space"}));
        assert_eq!(space["dimension"], "ThreeD");
        assert_eq!(space["background"], "sky");
        assert!(space["main_camera"].is_null());
    }

    #[test]
    fn planar_collision_example_blocks_the_shortcut_and_accepts_the_route() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap();
        let server = Futty::new(repo);
        let mut request: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/planar-collision/playtest.json"
        ))
        .unwrap();
        let play = |request| {
            server
                .play_test(Parameters(serde_json::from_value(request).unwrap()))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let solved = play(request.clone());
        assert_eq!(solved["passed"], true, "{solved}");
        request["steps"] = json!([{"actions":["right"],"ticks":90}]);
        request["expect"] = json!("running");
        let blocked = play(request);
        assert_eq!(blocked["passed"], true, "{blocked}");
        let position = &blocked["last_observation"]["player"]["position"];
        assert!((-0.8..-0.58).contains(&position[0].as_f64().unwrap()));
        assert_eq!(position[2].as_f64(), Some(2.0));
    }

    #[test]
    fn orthographic_camera_tests_a_separate_character_in_the_xy_plane() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let server = Futty::new(root.clone());
        let path = "Platform.ron";
        Scene::default().save_new(&root.join(path)).unwrap();
        let edits = json!([
            {"op":"add_camera","key":"view","name":"Side camera","position":[0,2,10],"target":[0,2,0],"orthographic_size":4},
            {"op":"add_object","key":"floor","name":"Floor","properties":{"position":[0,-0.25,1],"scale":[12,0.5,2],"collider":"fixed"}},
            {"op":"add_object","key":"player","name":"Runner","properties":{"position":[0,1.65,1]}},
            {"op":"set_component","object":"player","component":{"kind":"character","mode":"side_scroller"}}
        ]);
        let apply = |edits| {
            server
                .scene_apply(Parameters(
                    serde_json::from_value(json!({"path":path,"edits":edits})).unwrap(),
                ))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let objects = apply(edits)["objects"].clone();
        let play = |steps| {
            server
                .play_test(Parameters(
                    serde_json::from_value(json!({"path":path,"steps":steps,"expect":"running"}))
                        .unwrap(),
                ))
                .unwrap()
                .structured_content
                .unwrap()["last_observation"]["player"]
                .clone()
        };
        let platformer =
            play(json!([{"ticks":18},{"actions":["right","forward","jump"],"ticks":18}]));
        assert_eq!(platformer["id"], objects["player"]);
        let position = platformer["position"].as_array().unwrap();
        assert!(
            position[0].as_f64().unwrap() > 1.0 && position[1].as_f64().unwrap() > 2.0,
            "{platformer}"
        );
        assert_eq!(position[2], 1.0);
        apply(
            json!([{"op":"set_component","object":objects["player"],"component":{"kind":"character","mode":"top_down_2d"}}]),
        );
        let top = play(json!([{"actions":["forward","jump"],"ticks":30}]));
        let position = top["position"].as_array().unwrap();
        assert_eq!(position[0], 0.0);
        assert!((position[1].as_f64().unwrap() - 3.85).abs() < 0.01, "{top}");
        assert_eq!(position[2], 1.0);
        let scene = Scene::load(&root.join(path)).unwrap();
        assert_eq!(
            scene.entities[scene.camera_index(None).unwrap()]
                .camera
                .unwrap()
                .projection,
            futty_core::Projection::Orthographic
        );
    }

    #[test]
    fn maze_batch_repeats_wall_layout_and_supports_a_playable_route() {
        let dir = tempfile::tempdir().unwrap();
        let server = Futty::new(dir.path().canonicalize().unwrap());
        let create = |path: &str| {
            let mut scene = Scene::default();
            scene.entities.clear();
            scene.save_new(&server.root.join(path)).unwrap();
        };
        let apply = |path: &str, seed: u32, rooms: [u8; 2]| {
            server.scene_apply(Parameters(
                serde_json::from_value(json!({"path":path,"edits":[
                    {"op":"generate_maze2d","start_key":"player","goal_key":"exit",
                        "layout":{"seed":seed,"rooms":rooms,"origin":[0,0,1]}},
                    {"op":"add_camera","key":"view","name":"Overhead","position":[3,3,16],
                        "target":[3,3,1],"orthographic_size":8},
                    {"op":"set_camera_follow","object":"view","tag":"maze_player"},
                    {"op":"set_script","object":"exit","name":"Reach exit",
                        "source":r#"fn on_update(entity, dt) {
                            if !find_nearby(entity, "maze_player", 0.5).is_empty() {
                                finish_game("success");
                            }
                        }"#}
                ]}))
                .unwrap(),
            ))
        };
        let walls = |path: &str| {
            Scene::load(&server.root.join(path))
                .unwrap()
                .entities
                .into_iter()
                .filter(|object| object.name.starts_with("Maze wall "))
                .map(|object| (object.name, object.transform, object.body))
                .collect::<Vec<_>>()
        };
        let path = "Maze.ron";
        create(path);
        create("Same.ron");
        create("Different.ron");
        apply(path, 0, [2, 2]).unwrap();
        apply("Same.ron", 0, [2, 2]).unwrap();
        apply("Different.ron", 1, [2, 2]).unwrap();
        assert_eq!(walls(path), walls("Same.ron"));
        assert_ne!(walls(path), walls("Different.ron"));
        let saved = std::fs::read(server.root.join(path)).unwrap();
        assert!(apply(path, 0, [9, 2]).is_err());
        assert_eq!(std::fs::read(server.root.join(path)).unwrap(), saved);
        let play = |steps, expect| {
            server
                .play_test(Parameters(
                    serde_json::from_value(json!({"path":path,"steps":steps,"expect":expect}))
                        .unwrap(),
                ))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let blocked = play(json!([{"actions":["right"],"ticks":82}]), "running");
        assert!(
            blocked["last_observation"]["player"]["position"][0]
                .as_f64()
                .unwrap()
                < 1.5
        );
        let completed = play(
            json!([{"actions":["forward"],"ticks":82},{"actions":["right"],"ticks":82}]),
            "success",
        );
        assert_eq!(completed["passed"], true, "{completed}");
    }

    #[test]
    fn two_players_move_and_interact_independently() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let server = Futty::new(root.clone());
        let path = "Players.ron";
        let mut scene = Scene::default();
        scene.entities.clear();
        let press = scripts::register(
            &mut scene,
            "Press".into(),
            r#"
            fn fields() { #{ key: "presses" } }
            fn on_start(entity) { this.count = 0; set_hud(param("key"), 0); }
            fn on_interact(entity, actor) {
                this.count += 1;
                set_hud(param("key"), this.count);
            }
        "#
            .into(),
        );
        let mut players = Vec::new();
        let mut targets = Vec::new();
        for (key, x) in [("left", -3.0), ("right", 3.0)] {
            let mut camera = camera_object(key.into(), [x, 1.65, 4.0], [x, 1.65, 0.0]).unwrap();
            camera.character = Some(Default::default());
            players.push(camera.uuid);
            let mut target = Object::cube(format!("{key} button"), Vec3::new(x, 1.65, 2.5));
            target.spin = false;
            target.interaction = Some(futty_runtime::Interaction {
                prompt: format!("Use {key}"),
                range: 2.0,
            });
            target.script = Some(press);
            target
                .script_fields
                .insert("key".into(), futty_runtime::ScriptValue::Text(key.into()));
            targets.push(target.uuid);
            let mut hud = HudWidget::new(futty_runtime::HudKind::Text);
            hud.binding = key.into();
            scene.hud.push(hud);
            scene.entities.extend([camera, target]);
        }
        scene.main_camera = Some(players[0]);
        scene.save_new(&root.join(path)).unwrap();
        let initial = server
            .play_start(Parameters(
                serde_json::from_value(json!({"path":path})).unwrap(),
            ))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(
            initial["players"][0]["interaction"]["object_id"],
            targets[0].to_string()
        );
        assert_eq!(
            initial["players"][1]["interaction"]["object_id"],
            targets[1].to_string()
        );
        let step = |players| {
            server
                .play_step(Parameters(
                    serde_json::from_value(json!({
                        "session_id":initial["session_id"], "ticks":3, "players":players
                    }))
                    .unwrap(),
                ))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let first = step(json!([
            {"player_id":players[0],"actions":["forward","interact"]},
            {"player_id":players[1],"actions":[]}
        ]));
        assert_eq!(first["hud"], json!({"left":1.0,"right":0.0}));
        assert!(first["players"][0]["position"][2].as_f64().unwrap() < 4.0);
        assert_eq!(first["players"][1]["position"][2], 4.0);
        let second = step(json!([
            {"player_id":players[0],"actions":["interact"]},
            {"player_id":players[1],"actions":["interact"]}
        ]));
        assert_eq!(second["hud"], json!({"left":1.0,"right":1.0}));
        let third = step(json!([{"player_id":players[1],"look":[180.0,0.0]}]));
        assert!(third["players"][1]["interaction"].is_null());
        let replay_path = "Players.replay.json";
        server
            .play_save_replay(Parameters(
                serde_json::from_value(json!({
                    "session_id":initial["session_id"], "path":replay_path
                }))
                .unwrap(),
            ))
            .unwrap();
        let replayed = server
            .play_replay(Parameters(ReplayFile {
                path: replay_path.into(),
            }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(replayed["matched"], true);
    }

    #[test]
    fn coin_template_creates_a_playable_project_without_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let server = Futty::new(root.clone());
        let create = |template: &str, folder: &str| {
            server.game_create_from_template(Parameters(templates::CreateGame {
                template: template.into(),
                output_path: folder.into(),
            }))
        };
        assert!(create("unknown", "Game").is_err());
        assert!(create(templates::COIN_COLLECTION, "../Game").is_err());
        assert!(!root.join("Game").exists());
        let generated = create(templates::COIN_COLLECTION, "Game")
            .unwrap()
            .structured_content
            .unwrap();
        let manifest = generated["project_path"].as_str().unwrap();
        let source = root.join(generated["scene_path"].as_str().unwrap());
        let before = fs::read(&source).unwrap();
        assert!(create(templates::COIN_COLLECTION, "Game").is_err());
        assert_eq!(fs::read(&source).unwrap(), before);
        let valid = server
            .project_validate(Parameters(ProjectFile {
                path: manifest.into(),
            }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(valid["valid"], true, "{valid}");
        let mut idle = generated["playtest"].clone();
        idle["steps"] = json!([{"ticks":120}]);
        idle["seed"] = json!(17);
        idle["replay_path"] = json!("Idle.json");
        let missed = server
            .play_test(Parameters(serde_json::from_value(idle).unwrap()))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(missed["passed"], false, "{missed}");
        assert_eq!(
            missed["last_observation"]["hud"]["coins"].as_f64(),
            Some(0.0)
        );
        let report = &missed["failure_report"];
        assert_eq!(report["schema_version"], 1);
        assert_eq!(report["code"], "unexpected_status");
        assert_eq!(report["context"]["tick"], 120);
        assert_eq!(report["context"]["seed"], 17);
        assert_eq!(report["context"]["replay_path"], "Idle.json");
        assert_eq!(report["evidence"]["expected"], "success");
        assert_eq!(report["evidence"]["failed_checks"][0]["binding"], "coins");
        assert!(root.join("Idle.json").is_file());
        let input = serde_json::from_value(generated["playtest"].clone()).unwrap();
        let outcome = server
            .play_test(Parameters(input))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(outcome["passed"], true, "{outcome}");
        assert!(outcome.get("failure_report").is_none());
        assert_eq!(
            outcome["last_observation"]["hud"]["objective"],
            "All coins collected"
        );
        let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/futty-editor.exe");
        let staging = root.join("Futty.exe");
        fs::copy(template, staging).unwrap();
        let built = server
            .build_project(BuildProject {
                path: manifest.into(),
                output_path: "Windows".into(),
                template_path: Some("Futty.exe".into()),
            })
            .unwrap()
            .structured_content
            .unwrap();
        let exe = root.join(built["executable"].as_str().unwrap());
        futty_build::validate_windows_template(&exe).unwrap();
        assert!(root.join("Windows").join(futty_build::MANIFEST).is_file());
    }

    #[test]
    fn power_maze_template_requires_power_before_the_exit() {
        let dir = tempfile::tempdir().unwrap();
        let server = Futty::new(dir.path().canonicalize().unwrap());
        let created = server
            .game_create_from_template(Parameters(templates::CreateGame {
                template: templates::POWER_MAZE.into(),
                output_path: "Maze".into(),
            }))
            .unwrap()
            .structured_content
            .unwrap();
        let project = created["project_path"].as_str().unwrap();
        let valid = server
            .project_validate(Parameters(ProjectFile {
                path: project.into(),
            }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(valid["valid"], true, "{valid}");
        let play = |recipe| {
            server
                .play_test(Parameters(serde_json::from_value(recipe).unwrap()))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let mut direct = created["playtest"].clone();
        direct["steps"].as_array_mut().unwrap().truncate(2);
        direct["expect"] = json!("running");
        direct["expect_hud"] = json!({"power":0,"objective":"Exit locked. Find cell."});
        let locked = play(direct);
        assert_eq!(locked["passed"], true, "{locked}");
        let success = play(created["playtest"].clone());
        assert_eq!(success["passed"], true, "{success}");
        assert_eq!(
            success["last_observation"]["hud"]["objective"],
            "Maze escaped"
        );
        assert!(
            success["last_observation"]["visible_objects"]
                .as_array()
                .unwrap()
                .iter()
                .all(|object| object["name"] != "Power cell")
        );
    }

    #[test]
    fn coin_runner_template_requires_the_jump_and_collects_three_coins() {
        let dir = tempfile::tempdir().unwrap();
        let server = Futty::new(dir.path().canonicalize().unwrap());
        let created = server
            .game_create_from_template(Parameters(templates::CreateGame {
                template: templates::COIN_RUNNER.into(),
                output_path: "Runner".into(),
            }))
            .unwrap()
            .structured_content
            .unwrap();
        let path = created["project_path"].as_str().unwrap();
        let valid = server
            .project_validate(Parameters(ProjectFile { path: path.into() }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(valid["valid"], true, "{valid}");
        let scene =
            Scene::load(&server.root.join(created["scene_path"].as_str().unwrap())).unwrap();
        assert_eq!(scene.dimension, SceneDimension::TwoD);
        assert!(
            scene
                .entities
                .iter()
                .filter_map(|object| object.sprite)
                .all(|sprite| sprite.planar.is_some())
        );
        let play = |recipe| {
            server
                .play_test(Parameters(serde_json::from_value(recipe).unwrap()))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let success = play(created["playtest"].clone());
        assert_eq!(success["passed"], true, "{success}");
        assert_eq!(
            success["last_observation"]["hud"]["coins"].as_f64(),
            Some(3.0)
        );
        let failure = play(json!({
            "path":created["scene_path"],
            "steps":[{"ticks":10},{"actions":["right"],"ticks":120},{"actions":["right"],"ticks":60}],
            "expect":"failure","expect_hud":{"coins":1}
        }));
        assert_eq!(failure["passed"], true, "{failure}");
    }

    #[test]
    #[ignore = "requires Python to generate the prototype asset pack"]
    fn generated_prototype_assets_import_through_futty() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let generator = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../scripts/generate-prototype-assets.py");
        let generated =
            std::process::Command::new(if cfg!(windows) { "python" } else { "python3" })
                .arg(generator)
                .arg("--output")
                .arg(root.join("AssetsGenerated"))
                .args(["--seed", "42"])
                .output()
                .unwrap();
        assert!(
            generated.status.success(),
            "{}",
            String::from_utf8_lossy(&generated.stderr)
        );
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("AssetsGenerated/asset-manifest.json")).unwrap(),
        )
        .unwrap();
        let server = Futty::new(root.clone());
        let path = "Prototype.futty.scene.ron";
        let scene_path = root.join(path);
        Scene::default().save_new(&scene_path).unwrap();
        let entries = manifest["assets"].as_array().unwrap();
        assert_eq!(entries.len(), 8);
        for entry in entries {
            let relative = entry["path"].as_str().unwrap();
            let source = format!("AssetsGenerated/{relative}");
            let result = server
                .asset_import(Parameters(ImportAsset {
                    path: path.into(),
                    source: source.clone(),
                }))
                .unwrap_or_else(|error| panic!("{relative}: {}", error.message));
            let result = result.structured_content.unwrap();
            let id = result["asset"]["id"].as_str().unwrap();
            let scene = Scene::load(&scene_path).unwrap();
            let asset = scene
                .assets
                .iter()
                .find(|asset| asset.id.to_string() == id)
                .unwrap();
            let bytes = futty_assets::read_saved(&scene_path, asset).unwrap();
            assert_eq!(bytes, fs::read(root.join(source)).unwrap());
            if asset.kind == AssetKind::Model {
                let rig = futty_assets::model(&bytes).unwrap().rig.unwrap();
                let clips: Vec<_> = rig.clips().iter().map(|clip| clip.name.as_str()).collect();
                assert_eq!(clips, ["Idle", "Action"]);
            }
        }
        let result = server
            .scene_validate(Parameters(SceneFile { path: path.into() }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(result["valid"], true, "{result}");
    }

    #[test]
    fn project_build_publishes_a_portable_package_and_rejects_existing_output() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let server = Futty::new(root.clone());
        let manifest = "Game.futty.project.ron";
        for name in ["Lobby.ron", "Level.ron"] {
            Scene::default().save_new(&root.join(name)).unwrap();
        }
        server
            .project_create(Parameters(NewProject {
                path: manifest.into(),
                start: "lobby".into(),
                scenes: [
                    ("lobby".into(), "Lobby.ron".into()),
                    ("level".into(), "Level.ron".into()),
                ]
                .into(),
            }))
            .unwrap();
        let template = root.join("Template.exe");
        let build = |output_path: &str| {
            server.build_project(BuildProject {
                path: manifest.into(),
                output_path: output_path.into(),
                template_path: Some("Template.exe".into()),
            })
        };
        fs::write(&template, b"not a windows executable").unwrap();
        assert!(build("Build").is_err());
        assert!(!root.join("Build").exists());
        let mut exe = [0_u8; 70];
        exe[..2].copy_from_slice(b"MZ");
        exe[60..64].copy_from_slice(&64_u32.to_le_bytes());
        exe[64..].copy_from_slice(b"PE\0\0\x64\x86");
        fs::write(&template, exe).unwrap();
        assert!(build("../Build").is_err());
        let result = build("Build").unwrap().structured_content.unwrap();
        assert_eq!(result["executable"], "Build/Game.exe");
        assert_eq!(fs::read(root.join("Build/Game.exe")).unwrap(), exe);
        assert!(root.join("Build/Scenes/0/game.futty.scene.ron").is_file());
        assert!(root.join("Build/Scenes/1/game.futty.scene.ron").is_file());
        assert!(root.join("Build").join(futty_build::MANIFEST).is_file());
        assert!(build("Build").is_err());
    }

    #[test]
    fn validates_project_assets_and_checks_playtest_events() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let server = Futty::new(root.clone());
        let path = "Main.ron";
        let manifest = "Check.futty.project.ron";
        let mut scene = Scene::default();
        let mut floor = Object::cube("Floor".into(), Vec3::Y * -0.25);
        floor.transform.scale = Vec3::new(6.0, 0.5, 6.0);
        floor.body = Some(futty_runtime::Body {
            motion: futty_runtime::Motion::Fixed,
            ..Default::default()
        });
        floor.spin = false;
        let mut falling = Object::cube("Falling".into(), Vec3::Y * 3.0);
        falling.body = Some(futty_runtime::Body::default());
        let mut sensor = Object::cube("Sensor".into(), Vec3::Y * 1.5);
        sensor.body = Some(futty_runtime::Body {
            motion: futty_runtime::Motion::Fixed,
            size: Vec3::new(2.0, 0.2, 2.0),
            sensor: true,
            ..Default::default()
        });
        let (first, second) = (falling.uuid, sensor.uuid);
        let mut camera = camera_object("Player".into(), [0.0, 2.0, 4.0], [0.0, 2.0, 0.0]).unwrap();
        camera.character = Some(Default::default());
        camera.script = Some(scripts::register(
            &mut scene,
            "Score".into(),
            "fn on_start(entity) { set_hud(\"score\", 3); }".into(),
        ));
        let script = camera.script.unwrap();
        let mut hud = HudWidget::new(futty_runtime::HudKind::Text);
        hud.binding = "score".into();
        scene.hud.push(hud);
        scene.entities.extend([floor, sensor, falling, camera]);
        scene.save_new(&root.join(path)).unwrap();
        server
            .project_create(Parameters(NewProject {
                path: manifest.into(),
                start: "main".into(),
                scenes: [("main".into(), path.into())].into(),
            }))
            .unwrap();
        let validate = || {
            server
                .project_validate(Parameters(ProjectFile {
                    path: manifest.into(),
                }))
                .unwrap()
                .structured_content
                .unwrap()
        };
        assert_eq!(validate()["valid"], true);
        let play = |score| {
            server.play_test(Parameters(serde_json::from_value(json!({
            "path":path, "steps":[{"actions":[],"ticks":120}], "expect":"running",
            "expect_hud":{"score":score}, "expect_collisions":[{"first":first,"second":second,"sensor":true}]
        })).unwrap())).unwrap().structured_content.unwrap()
        };
        let result = play(3);
        assert_eq!(result["passed"], true, "{result}");
        assert!(result["checks"][1]["first_tick"].is_u64());
        let failed = play(5);
        assert_eq!(failed["passed"], false);
        assert_eq!(failed["failure_report"]["code"], "failed_checks");
        assert_eq!(
            failed["failure_report"]["evidence"]["failed_checks"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        scene
            .script_sources
            .insert(script, "fn on_start(entity) {\n let x = ;\n}".into());
        scene.save(&root.join(path)).unwrap();
        let invalid = validate();
        assert_eq!(invalid["valid"], false);
        assert_eq!(invalid["scenes"][0]["issues"][0]["line"], 2);
        let report = &invalid["failure_report"];
        assert_eq!(report["stage"], "project_validation");
        assert_eq!(report["code"], "validation_failed");
        assert_eq!(
            report["evidence"]["invalid_scenes"][0]["issues"][0]["line"],
            2
        );
    }

    #[test]
    fn validates_scenes_and_persists_editor_project_start() {
        let folder = tempfile::tempdir().unwrap();
        let root = folder.path().canonicalize().unwrap();
        let server = Futty::new(root.clone());
        let scene = Scene::default();
        scene.save_new(&root.join("first.ron")).unwrap();
        scene.save_new(&root.join("second.ron")).unwrap();
        let valid = server
            .scene_validate(Parameters(SceneFile {
                path: "first.ron".into(),
            }))
            .unwrap();
        assert_eq!(valid.structured_content.unwrap()["valid"], true);

        let broken = root.join("broken.ron");
        let source = fs::read_to_string(root.join("first.ron")).unwrap();
        let parent = Uuid::new_v4();
        let changed = source.replacen("parent: None", &format!("parent: Some(\"{parent}\")"), 1);
        assert_ne!(source, changed);
        fs::write(&broken, &changed).unwrap();
        let invalid = server
            .scene_validate(Parameters(SceneFile {
                path: "broken.ron".into(),
            }))
            .unwrap();
        let invalid = invalid.structured_content.unwrap();
        let issue = &invalid["issues"][0];
        assert_eq!(issue["field"], "parent");
        assert_eq!(issue["object_id"], json!(scene.entities[0].uuid));
        assert_eq!(invalid["failure_report"]["stage"], "scene_validation");
        assert_eq!(invalid["failure_report"]["evidence"]["issues"][0], *issue);
        assert_eq!(fs::read_to_string(broken).unwrap(), changed);

        let path = "Test.futty.project.ron";
        let create = || {
            Parameters(NewProject {
                path: path.into(),
                start: "first".into(),
                scenes: [
                    ("first".into(), "first.ron".into()),
                    ("second".into(), "second.ron".into()),
                ]
                .into(),
            })
        };
        server.project_create(create()).unwrap();
        assert!(server.project_create(create()).is_err());
        let set = |start: &str| {
            Parameters(SetProjectStart {
                path: path.into(),
                start: start.into(),
            })
        };
        server.project_set_start_scene(set("second")).unwrap();
        assert!(server.project_set_start_scene(set("unknown")).is_err());
        let inspect = server
            .project_inspect(Parameters(ProjectFile { path: path.into() }))
            .unwrap();
        assert_eq!(inspect.structured_content.unwrap()["start"], "second");
        assert_eq!(
            Project::load(&root.join(path))
                .unwrap()
                .resolve("second")
                .unwrap(),
            root.join("second.ron")
        );
    }

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
    fn prefab_and_transition_tools_use_saved_scenes() {
        let folder = tempfile::tempdir().unwrap();
        let server = Futty::new(folder.path().canonicalize().unwrap());
        let from = "Lobby.ron";
        let to = "Results.ron";
        let lobby = Scene::default();
        let host = lobby.entities[0].uuid;
        lobby.save_new(&server.root.join(from)).unwrap();
        Scene::default().save_new(&server.root.join(to)).unwrap();
        let prefab_id = serde_json::from_value(
            server
                .prefab_create(Parameters(CreatePrefab {
                    path: from.into(),
                    object_id: host,
                }))
                .unwrap()
                .structured_content
                .unwrap()["prefab_id"]
                .clone(),
        )
        .unwrap();
        let placed = server
            .prefab_instantiate(Parameters(PlacePrefab {
                path: from.into(),
                prefab_id,
                position: [5.0, 0.0, 0.0],
            }))
            .unwrap()
            .structured_content
            .unwrap();
        assert!(placed["objects"]["instance"].is_string());
        assert!(placed["undo_token"].is_string());
        server
            .prefab_update(Parameters(UpdatePrefab {
                path: from.into(),
                prefab_id,
                name: Some("Reused cube".into()),
            }))
            .unwrap();
        let manifest = "Game.futty.project.ron";
        server
            .project_create(Parameters(NewProject {
                path: manifest.into(),
                start: "lobby".into(),
                scenes: [("lobby".into(), from.into()), ("results".into(), to.into())].into(),
            }))
            .unwrap();
        let transition = |target: &str| {
            server.scene_transition_set(Parameters(SceneTransition {
                path: manifest.into(),
                from: "lobby".into(),
                to: target.into(),
                object_id: host,
                button: None,
            }))
        };
        assert!(transition("unknown").is_err());
        let link = transition("results").unwrap().structured_content.unwrap();
        assert_eq!(
            transition("results").unwrap().structured_content.unwrap(),
            link
        );
        let saved = Scene::load(&server.root.join(from)).unwrap();
        assert_eq!(saved.prefabs[&prefab_id].name, "Reused cube");
        assert_eq!(saved.script_sources.len(), 1);
        let batch = serde_json::from_value(json!({"path":from,"edits":[
            {"op":"add_camera","key":"camera","name":"Player","position":[0,2,4],"target":[0,2,0],"character":true}
        ]})).unwrap();
        server.scene_apply(Parameters(batch)).unwrap();
        let run = serde_json::from_value(
            json!({"path":from,"steps":[{"actions":["interact"],"ticks":2}],"expect":"scene_transition","expect_scene":"results"}),
        )
        .unwrap();
        let result = server
            .play_test(Parameters(run))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(result["passed"], true);
        assert_eq!(result["last_observation"]["requested_scene"], "results");
    }

    #[test]
    fn saved_input_and_hud_reject_invalid_edits() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let server = Futty::new(root.clone());
        let path = "main.ron";
        let file = root.join(path);
        Scene::default().save_new(&file).unwrap();
        let set = |binding: &str| {
            server.input_set(Parameters(SetInput {
                path: path.into(),
                bindings: [("Jump".into(), binding.into())].into(),
                sensitivity: Some(1.5),
                capture_mouse: false,
            }))
        };
        set("J").unwrap();
        let input = server
            .input_get(Parameters(SceneFile { path: path.into() }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(input["input"]["bindings"]["Jump"], "J");
        assert_eq!(input["defaults"]["Jump"], "Space");
        let before = fs::read(&file).unwrap();
        assert!(set("Escape").is_err());
        assert_eq!(fs::read(&file).unwrap(), before);
        let create = |kind, settings| {
            let input =
                serde_json::from_value(json!({"path":path,"kind":kind,"settings":settings}))
                    .unwrap();
            server
                .hud_create(Parameters(input))
                .unwrap()
                .structured_content
                .unwrap()
        };
        let gauge = create("Gauge", json!({"label":"Health","anchor":"BottomLeft"}));
        let button = create("Button", json!({"label":"Retry","binding":"retry"}));
        let widget_id = serde_json::from_value(gauge["widget"]["id"].clone()).unwrap();
        let bind = |binding: &str| {
            server.hud_bind(Parameters(BindHud {
                path: path.into(),
                widget_id,
                binding: binding.into(),
            }))
        };
        bind("player.health").unwrap();
        assert!(bind("invalid binding").is_err());
        let update = serde_json::from_value(json!({
            "path":path,
            "widget_id":widget_id,
            "settings":{"size":[120,20]}
        }))
        .unwrap();
        server.hud_update(Parameters(update)).unwrap();
        let list = server
            .hud_list(Parameters(SceneFile { path: path.into() }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(list["widgets"][0]["binding"], "player.health");
        assert_eq!(list["widgets"][0]["size"], json!([120.0, 20.0]));
        assert_eq!(list["widgets"][1], button["widget"]);
    }

    #[test]
    fn script_tools_report_positions_and_keep_bindings_on_update() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let server = Futty::new(root.clone());
        let path = "main.ron";
        let scene = Scene::default();
        let object_id = scene.entities[0].uuid;
        scene.save_new(&root.join(path)).unwrap();
        let broken = "fn on_start(entity) {\n let x = ;\n}";
        fs::write(root.join("broken.rhai"), broken).unwrap();
        let diagnosis = server
            .script_validate(Parameters(ValidateScript {
                source: None,
                path: Some("broken.rhai".into()),
            }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(diagnosis["valid"], false);
        assert_eq!(diagnosis["path"], "broken.rhai");
        assert_eq!(diagnosis["issues"][0]["line"], 2);
        assert!(diagnosis["issues"][0]["column"].is_u64());

        let initial = "fn fields() { #{speed: 4.0} }";
        let created = server
            .script_create(Parameters(CreateScript {
                path: path.into(),
                name: "Movement".into(),
                source: initial.into(),
            }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(created["fields"]["speed"]["Number"], 4.0);
        let script_id = serde_json::from_value::<Uuid>(created["script_id"].clone()).unwrap();
        let attach = |id| {
            server.script_attach(Parameters(AttachScript {
                path: path.into(),
                object_id,
                script_id: id,
            }))
        };
        attach(Some(script_id)).unwrap();
        let update = |source: &str| {
            server.script_update(Parameters(UpdateScript {
                path: path.into(),
                script_id,
                source: source.into(),
            }))
        };
        let before = fs::read(root.join(path)).unwrap();
        let error = update(broken).unwrap_err();
        assert_eq!(error.data.unwrap()["issue"]["line"], 2);
        assert_eq!(fs::read(root.join(path)).unwrap(), before);
        let replacement = "fn fields() { #{speed: 8.0} }";
        update(replacement).unwrap();
        let stored = server
            .script_get(Parameters(ScriptQuery {
                path: path.into(),
                script_id,
            }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(stored["source"], replacement);
        assert_eq!(stored["embedded"], true);
        let load = || Scene::load(&root.join(path)).unwrap();
        assert_eq!(load().entities[0].script, Some(script_id));
        let detach = attach(None).unwrap().structured_content.unwrap();
        assert_eq!(load().entities[0].script, None);
        server
            .scene_undo(Parameters(
                serde_json::from_value::<Undo>(
                    json!({"path":path,"undo_token":detach["undo_token"]}),
                )
                .unwrap(),
            ))
            .unwrap();
        assert_eq!(load().entities[0].script, Some(script_id));
    }

    #[test]
    fn batch_assets_bind_and_preserve_ids_after_replacement() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let server = Futty::new(root.clone());
        let path = "Main.ron";
        let scene_path = root.join(path);
        let mut scene = Scene::default();
        let sprite = scene.entities[0].uuid;
        let mut model = Object::cube("Model".into(), Vec3::X);
        model.spin = false;
        let model_id = model.uuid;
        scene.entities.push(model);
        let material = Uuid::new_v4();
        scene.materials.insert(material, Material::default());
        let widget = HudWidget::new(futty_runtime::HudKind::Image);
        let image = widget.id;
        scene.hud.push(widget);
        scene.save_new(&scene_path).unwrap();
        let original = include_bytes!("../../../examples/sprite-playground/Signal.png");
        let replacement = include_bytes!("../../../examples/asset-preview/Workshop Grid.png");
        fs::write(root.join("sprite.png"), original).unwrap();
        fs::write(root.join("updated.png"), replacement).unwrap();
        fs::write(root.join("invalid.wav"), b"not a WAV").unwrap();
        fs::write(
            root.join("bell.wav"),
            include_bytes!("../../../examples/audio-playground/Chime.wav"),
        )
        .unwrap();
        fs::write(
            root.join("crate.glb"),
            include_bytes!("../../../examples/asset-preview/Workshop Crate.glb"),
        )
        .unwrap();
        let batch = |assets| {
            server.asset_import_batch(Parameters(
                serde_json::from_value(json!({"path":path,"assets":assets})).unwrap(),
            ))
        };
        let result = batch(json!([
            {"key":"texture","source":"sprite.png","bind":[{"target":"sprite","id":sprite,"size":[2,1]},
                {"target":"material","id":material},{"target":"hud_image","id":image}]},
            {"key":"sound","source":"bell.wav","bind":[{"target":"object","id":sprite}]},
            {"key":"mesh","source":"crate.glb","bind":[{"target":"object","id":model_id}]}
        ])).unwrap().structured_content.unwrap();
        let id = |key: &str| serde_json::from_value::<Uuid>(result["assets"][key].clone()).unwrap();
        let texture = id("texture");
        let scene = Scene::load(&scene_path).unwrap();
        assert_eq!(scene.entities[0].sprite.unwrap().texture, texture);
        assert_eq!(scene.entities[0].audio_source.unwrap().clip, id("sound"));
        assert_eq!(scene.entities[1].mesh, Some(id("mesh")));
        assert_eq!(scene.materials[&material].texture, Some(texture));
        assert_eq!(scene.hud[0].image, Some(texture));
        let asset = scene
            .assets
            .iter()
            .find(|asset| asset.id == texture)
            .unwrap();
        let saved_scene = fs::read(&scene_path).unwrap();
        let failed = batch(json!([
            {"key":"updated","source":"updated.png","asset_id":texture},
            {"key":"broken","source":"invalid.wav"}
        ]));
        assert!(failed.is_err());
        assert_eq!(
            futty_assets::read_saved(&scene_path, asset).unwrap(),
            original
        );
        assert!(
            batch(json!([{"key":"bad_binding","source":"sprite.png",
            "bind":[{"target":"hud_image","id":sprite}]}]))
            .is_err()
        );
        assert_eq!(fs::read(&scene_path).unwrap(), saved_scene);
        let replace = || {
            batch(json!([{"key":"updated","source":"updated.png","asset_id":texture}]))
                .unwrap()
                .structured_content
                .unwrap()
        };
        assert_eq!(replace()["assets"]["updated"], result["assets"]["texture"]);
        assert_eq!(replace()["assets"]["updated"], result["assets"]["texture"]);
        assert_eq!(fs::read(&scene_path).unwrap(), saved_scene);
        assert_eq!(
            futty_assets::read_saved(&scene_path, asset).unwrap(),
            replacement
        );
    }

    #[test]
    fn asset_reimport_preserves_material_and_object_references() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let server = Futty::new(root.clone());
        let path = "main.ron";
        let scene_path = root.join(path);
        let scene = Scene::default();
        let object_id = scene.entities[0].uuid;
        scene.save_new(&scene_path).unwrap();
        let original = include_bytes!("../../../examples/sprite-playground/Signal.png");
        let replacement = include_bytes!("../../../examples/asset-preview/Workshop Grid.png");
        fs::write(root.join("source.png"), original).unwrap();
        fs::write(root.join("replacement.png"), replacement).unwrap();
        fs::write(root.join("broken.png"), b"not a texture").unwrap();
        fs::write(
            root.join("clip.wav"),
            include_bytes!("../../../examples/audio-playground/Chime.wav"),
        )
        .unwrap();
        fs::write(
            root.join("model.glb"),
            include_bytes!("../../../examples/asset-preview/Workshop Crate.glb"),
        )
        .unwrap();
        let imported = |source: &str| {
            server.asset_import(Parameters(ImportAsset {
                path: path.into(),
                source: source.into(),
            }))
        };
        assert!(imported("broken.png").is_err());
        assert!(imported("../source.png").is_err());
        let asset_id = |source| {
            serde_json::from_value::<Uuid>(
                imported(source).unwrap().structured_content.unwrap()["asset"]["id"].clone(),
            )
            .unwrap()
        };
        let texture = asset_id("source.png");
        let clip = asset_id("clip.wav");
        let model = asset_id("model.glb");
        let material = server
            .material_create(Parameters(
                serde_json::from_value::<CreateMaterial>(
                    json!({"path":path,"settings":{"name":"Grid","texture":texture}}),
                )
                .unwrap(),
            ))
            .unwrap()
            .structured_content
            .unwrap()["material_id"]
            .clone();
        server
            .scene_apply(Parameters(
                serde_json::from_value::<Batch>(json!({
                    "path":path,
                    "edits":[
                        {"op":"attach_asset","object":object_id,"asset_id":model},
                        {"op":"attach_asset","object":object_id,"asset_id":clip},
                        {"op":"set_material","object":object_id,"material_id":material}
                    ]
                }))
                .unwrap(),
            ))
            .unwrap();
        let before = fs::read(&scene_path).unwrap();
        let reimport = |source: &str| {
            server.asset_reimport(Parameters(ReimportAsset {
                path: path.into(),
                source: source.into(),
                asset_id: texture,
            }))
        };
        let saved = Scene::load(&scene_path).unwrap();
        let asset = saved
            .assets
            .iter()
            .find(|asset| asset.id == texture)
            .unwrap();
        assert!(reimport("broken.png").is_err());
        assert!(reimport("clip.wav").is_err());
        assert_eq!(
            futty_assets::read_saved(&scene_path, asset).unwrap(),
            original
        );
        reimport("replacement.png").unwrap();
        assert_eq!(fs::read(&scene_path).unwrap(), before);
        assert_eq!(
            futty_assets::read_saved(&scene_path, asset).unwrap(),
            replacement
        );
        let list = server
            .asset_list(Parameters(SceneFile { path: path.into() }))
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(
            list["materials"][material.as_str().unwrap()]["texture"],
            texture.to_string()
        );
        assert_eq!(saved.entities[0].mesh, Some(model));
        assert_eq!(saved.entities[0].audio_source.unwrap().clip, clip);
        let updated = server
            .material_update(Parameters(
                serde_json::from_value::<UpdateMaterial>(json!({
                    "path":path,"material_id":material,"settings":{"texture":null,"roughness":0.25}
                }))
                .unwrap(),
            ))
            .unwrap()
            .structured_content
            .unwrap();
        assert!(updated["material"]["texture"].is_null());
        assert_eq!(updated["material"]["name"], "Grid");
    }

    #[test]
    fn component_edits_keep_settings_and_undo_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let server = Futty::new(temp.path().canonicalize().unwrap());
        let path = "main.ron";
        Scene::default().save_new(&server.root.join(path)).unwrap();
        let apply = |edits| {
            server.scene_apply(Parameters(
                serde_json::from_value::<Batch>(json!({"path":path,"edits":edits})).unwrap(),
            ))
        };
        let created = apply(json!([
            {"op":"add_object","key":"lamp","name":"Lamp"},
            {"op":"add_object","key":"floor","name":"Floor"},
            {"op":"add_object","key":"player","name":"Player"},
            {"op":"set_component","object":"lamp","component":{"kind":"light","light_type":"Directional","intensity":2.0}},
            {"op":"set_component","object":"floor","component":{"kind":"body","size":[3,1,4],"collision_layer":4}},
            {"op":"set_component","object":"player","component":{"kind":"character","speed":6.0}}
        ])).unwrap().structured_content.unwrap();
        let lamp = &created["objects"]["lamp"];
        let floor = &created["objects"]["floor"];
        let player = &created["objects"]["player"];
        let read = |id, kind| {
            server
                .component_get(Parameters(
                    serde_json::from_value::<ComponentQuery>(
                        json!({"path":path,"object_id":id,"component":kind}),
                    )
                    .unwrap(),
                ))
                .unwrap()
                .structured_content
                .unwrap()["value"]
                .clone()
        };
        assert_eq!(read(floor, "body")["layers"]["layer"], 4);
        assert_eq!(read(player, "character")["speed"], 6.0);
        let result = apply(json!([
            {"op":"set_component","object":floor,"component":{"kind":"body","friction":0.5}},
            {"op":"remove_component","object":lamp,"component":"light"}
        ]))
        .unwrap()
        .structured_content
        .unwrap();
        assert!(read(lamp, "light").is_null());
        assert_eq!(read(floor, "body")["size"], json!([3.0, 1.0, 4.0]));
        assert_eq!(read(floor, "body")["friction"], 0.5);
        let saved = fs::read(server.root.join(path)).unwrap();
        assert!(
            apply(json!([
                {"op":"remove_component","object":floor,"component":"body"},
                {"op":"set_component","object":player,"component":{"kind":"body"}}
            ]))
            .is_err()
        );
        assert_eq!(fs::read(server.root.join(path)).unwrap(), saved);
        server
            .scene_undo(Parameters(
                serde_json::from_value::<Undo>(
                    json!({"path":path,"undo_token":result["undo_token"]}),
                )
                .unwrap(),
            ))
            .unwrap();
        assert_eq!(read(lamp, "light")["intensity"], 2.0);
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
                dimension: SceneDimension::ThreeD,
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
                dimension: SceneDimension::ThreeD,
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
                orthographic_size: None,
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
