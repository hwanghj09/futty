#![cfg_attr(windows, windows_subsystem = "windows")]

mod adaptive_quality;
#[cfg(windows)]
mod ai_setup;
mod animation;
mod assets;
mod audio;
mod camera;
mod context_menu;
mod document;
mod external_script;
mod feel;
mod hud_view;
mod input;
mod light;
mod material;
mod physics;
mod player;
mod prefab;
mod project;
mod range_view;
mod script_editor;
mod selection;
mod sprite;
mod transform_tool;
mod viewport;
mod workspace;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use document::Document;
use eframe::{egui, wgpu};
use egui::{Color32, Key, RichText};
use futty_core::{AssetKind, Camera, LightKind, Primitive};
use futty_runtime::{AudioSource, Object, Runtime, Scene, SceneDimension};
use glam::{EulerRot, Quat, Vec3};
use rfd::{FileDialog, MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
use selection::selected_tree;
use transform_tool::{Tool, TransformGroup, TransformTool};
use uuid::Uuid;
use viewport::{AXES, Viewport};
use workspace::Panel;

const TEXT: Color32 = Color32::from_gray(226);
const MUTED: Color32 = Color32::from_gray(158);
const PANEL: Color32 = Color32::from_gray(27);
const BACKGROUND: Color32 = Color32::from_gray(17);
const FOCUS: Color32 = Color32::from_rgb(157, 185, 215);
const WARNING: Color32 = Color32::from_rgb(218, 174, 120);

fn main() -> eframe::Result {
    use std::{env, path::Path};
    let executable = env::current_exe().ok();
    let args = env::args_os().collect::<Vec<_>>();
    let mode = args.get(1).and_then(|arg| arg.to_str());
    if mode == Some("--mcp") {
        if args.len() != 3 {
            eprintln!("Usage: futty-editor --mcp WORKSPACE");
            std::process::exit(2);
        }
        let editor = if cfg!(windows) { executable } else { None };
        if let Err(error) = futty_mcp::run(Path::new(&args[2]), editor) {
            eprintln!("Futty MCP failed: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    #[cfg(windows)]
    if mode == Some("--setup-ai") {
        let workspace = args.get(2).map(Path::new);
        let client = args
            .get(3)
            .and_then(|arg| ai_setup::Client::parse(arg.to_str()?));
        let (4, Some(workspace), Some(client)) = (args.len(), workspace, client) else {
            eprintln!("Usage: futty-editor --setup-ai WORKSPACE codex|claude-code|claude-desktop");
            std::process::exit(2);
        };
        match ai_setup::install(client, workspace) {
            Ok(message) => println!("{message}"),
            Err(error) => {
                eprintln!("Futty AI setup failed: {error}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if mode == Some("--export") {
        if args.len() != 4 {
            eprintln!("Usage: futty-editor --export SCENE_OR_PROJECT OUTPUT_FOLDER");
            std::process::exit(2);
        }
        let result = executable
            .as_deref()
            .ok_or_else(|| "Cannot locate the futty executable".into())
            .and_then(|exe| {
                player::export(
                    Path::new(&args[2]),
                    Path::new(&args[3]),
                    Camera::default(),
                    exe,
                )
            });
        if let Err(error) = result {
            eprintln!("Export failed: {error}");
            std::process::exit(1);
        }
        println!("Exported {}", Path::new(&args[3]).display());
        return Ok(());
    }
    let game = executable
        .as_deref()
        .and_then(Path::parent)
        .filter(|root| mode != Some("--edit") && root.join(player::MANIFEST).is_file());
    let requested_scene = match mode {
        Some("--play" | "--edit") if args.len() == 3 => Some(Path::new(&args[2])),
        Some(flag @ ("--play" | "--edit")) => {
            eprintln!("Usage: futty-editor {flag} SCENE_OR_PROJECT");
            std::process::exit(2);
        }
        _ => None,
    };
    let scene_to_play = requested_scene.filter(|_| mode == Some("--play"));
    let scene_to_edit = requested_scene.filter(|_| mode == Some("--edit"));
    let playing_game = game.is_some() || scene_to_play.is_some();
    let title = if playing_game {
        "futty Game"
    } else {
        "futty Editor"
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 740.0])
            .with_min_inner_size(if playing_game {
                [320.0, 240.0]
            } else {
                [900.0, 560.0]
            }),
        ..Default::default()
    };
    let result = eframe::run_native(
        title,
        options,
        Box::new(move |cc| {
            if let Some(scene) = scene_to_play {
                Ok(Box::new(player::Player::load_scene(cc, scene)?))
            } else if let Some(root) = game {
                Ok(Box::new(player::Player::load(cc, root)?))
            } else {
                let mut editor = Editor::new(cc)?;
                if let Some(path) = scene_to_edit {
                    if project::is_project(path) {
                        editor.open_project(path);
                    } else {
                        editor.open_scene(path.to_path_buf());
                    }
                }
                Ok(Box::new(editor))
            }
        }),
    );
    if let Err(error) = &result {
        MessageDialog::new()
            .set_title(format!("{title} could not start"))
            .set_description(error.to_string())
            .set_level(MessageLevel::Error)
            .show();
    }
    result
}

struct Editor {
    doc: Document,
    project: Option<project::Project>,
    viewport: Viewport,
    dock: egui_stack::DockState<Panel>,
    asset_bytes: HashMap<Uuid, Vec<u8>>,
    script_editor: Option<script_editor::ScriptEditor>,
    external_scripts: HashMap<Uuid, external_script::ExternalScript>,
    next_script_sync: std::time::Instant,
    script_schemas: script_editor::SchemaCache,
    script_inspector: bool,
    inspected_asset: Option<InspectedAsset>,
    hierarchy_search: String,
    audio: futty_audio::Audio,
    play: Option<Play>,
    diagnostics: workspace::RunDiagnostics,
    move_tool: bool,
    transform_tool: Option<TransformTool>,
    transform_feedback: Option<(&'static str, String, f64)>,
    scene_rect: Option<egui::Rect>,
    game_rect: Option<egui::Rect>,
    game_aspect: f32,
    range_controls: range_view::ViewportControls,
    input_settings: input::InputSettings,
    controls_open: bool,
    hud_open: bool,
    feel: feel::FeelPanel,
    hud_selection: Option<Uuid>,
    animation_preview: Option<animation::Preview>,
    camera_preview: Option<Uuid>,
    gpu_label: String,
    gpu_error: Arc<Mutex<Option<String>>>,
    message: String,
    error: bool,
    allow_close: bool,
}

enum Play {
    Edited {
        runtime: Box<Runtime>,
        camera: Camera,
    },
    Loaded(Box<player::GameScene>),
}

impl Play {
    fn runtime(&self) -> &Runtime {
        match self {
            Self::Edited { runtime, .. } => runtime,
            Self::Loaded(world) => &world.runtime,
        }
    }
}

#[derive(Clone, Copy)]
enum Action {
    #[cfg(windows)]
    SetupAi(ai_setup::Client),
    New(SceneDimension),
    PhysicsPlayground,
    ScriptPlayground,
    AudioPlayground,
    SpritePlayground,
    Open,
    Save,
    SaveAs,
    ExportWindows,
    Project(project::ProjectAction),
    Add(Primitive),
    AddCamera,
    AddLight(LightKind),
    SetMainCamera(Uuid),
    PreviewCamera(Uuid),
    AlignCamera(Uuid),
    SelectAll,
    ReparentSelection(Option<Uuid>),
    ReparentObject(Uuid, Option<Uuid>),
    Delete,
    Duplicate,
    Undo,
    Redo,
    PlayStop,
    ReloadPlayTint,
    PreviewFeel,
    Focus,
    FrameAll,
    Import,
    ReimportModel(Uuid),
    Place(Uuid),
    PlaceSprite(Uuid),
    ApplyTexture(Uuid),
    AttachScript(Uuid),
    NewScript,
    NewMaterial,
    EditMaterial(Uuid),
    ApplyMaterial(Uuid),
    RemoveMaterial(Uuid),
    CreatePrefab,
    InspectPrefab(Uuid),
    PlacePrefab(Uuid),
    RemovePrefab(Uuid),
    EditPrefabSource(Uuid),
    UnpackPrefab(Uuid),
    ResetPrefabOverrides(Uuid),
    EditScript(Uuid),
    EditScriptInFutty(Uuid),
    ApplyScriptDraft {
        clear: bool,
    },
    AttachAudio(Uuid),
    PreviewAudio(Uuid),
    StopAudioPreview,
    RemoveAsset(Uuid),
    ShowPanel(Panel),
    DockPanel(Panel),
    ResetLayout,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InspectedAsset {
    Material(Uuid),
    Prefab(Uuid),
}

impl Editor {
    fn new(
        cc: &eframe::CreationContext<'_>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let gpu = cc
            .wgpu_render_state
            .clone()
            .ok_or("A wgpu device is required")?;
        let adapter = gpu.adapter.get_info();
        let gpu_label = format!("{:?}  /  {}", adapter.backend, adapter.name);
        let gpu_error = Arc::new(Mutex::new(None));
        let errors = gpu_error.clone();
        let ctx = cc.egui_ctx.clone();
        gpu.device
            .on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
                *errors.lock().unwrap() = Some(error.to_string());
                ctx.request_repaint();
            }));
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "Inter".into(),
            egui::FontData::from_static(include_bytes!("../fonts/Inter.ttf")).into(),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "Inter".into());
        #[cfg(target_os = "windows")]
        if let Some(root) = std::env::var_os("WINDIR")
            && let Ok(font) = std::fs::read(std::path::Path::new(&root).join("Fonts/malgun.ttf"))
        {
            fonts.font_data.insert(
                "Malgun Gothic".into(),
                egui::FontData::from_owned(font).into(),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .push("Malgun Gothic".into());
        }
        cc.egui_ctx.set_fonts(fonts);
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(TEXT);
        visuals.panel_fill = PANEL;
        visuals.window_fill = PANEL;
        visuals.extreme_bg_color = BACKGROUND;
        visuals.faint_bg_color = Color32::from_gray(33);
        visuals.selection.bg_fill = Color32::from_rgb(49, 64, 80);
        visuals.selection.stroke = egui::Stroke::new(1.0, FOCUS);
        visuals.hyperlink_color = FOCUS;
        visuals.warn_fg_color = WARNING;
        visuals.error_fg_color = WARNING;
        for (widget, gray) in [
            (&mut visuals.widgets.inactive, 39),
            (&mut visuals.widgets.hovered, 53),
            (&mut visuals.widgets.active, 70),
            (&mut visuals.widgets.open, 49),
        ] {
            widget.bg_fill = Color32::from_gray(gray);
            widget.weak_bg_fill = widget.bg_fill;
        }
        cc.egui_ctx.set_visuals(visuals);
        cc.egui_ctx.global_style_mut(|style| {
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.spacing.interact_size = egui::vec2(44.0, 26.0);
            style.visuals.button_frame = true;
        });
        Ok(Self {
            doc: Document::new(Scene::default(), None),
            project: None,
            viewport: {
                let mut viewport = Viewport::new(gpu);
                viewport.transform_settings = cc
                    .storage
                    .and_then(|storage| {
                        eframe::get_value::<transform_tool::TransformSettings>(
                            storage,
                            "futty.transforms.v1",
                        )
                    })
                    .filter(|settings| settings.valid())
                    .unwrap_or_default();
                viewport
            },
            dock: workspace::load_layout(cc.storage),
            asset_bytes: HashMap::new(),
            script_editor: None,
            external_scripts: HashMap::new(),
            next_script_sync: std::time::Instant::now(),
            script_schemas: HashMap::new(),
            script_inspector: false,
            inspected_asset: None,
            hierarchy_search: String::new(),
            audio: futty_audio::Audio::default(),
            play: None,
            diagnostics: workspace::RunDiagnostics::default(),
            move_tool: true,
            transform_tool: None,
            transform_feedback: None,
            scene_rect: None,
            game_rect: None,
            game_aspect: 16.0 / 9.0,
            range_controls: range_view::ViewportControls::default(),
            input_settings: input::InputSettings::load(cc.storage),
            controls_open: false,
            hud_open: false,
            feel: feel::FeelPanel::default(),
            hud_selection: None,
            animation_preview: None,
            camera_preview: None,
            gpu_label,
            gpu_error,
            message: "Scene ready".into(),
            error: false,
            allow_close: false,
        })
    }

    fn report(&mut self, message: impl Into<String>, error: bool) {
        self.message = message.into();
        self.error = error;
    }

    fn transition(&mut self, ctx: &egui::Context, id: &str) {
        let result = (|| {
            let project = self
                .project
                .as_ref()
                .ok_or("Open a Futty project to switch scenes")?;
            let path = project.resolve(id)?;
            let state = self
                .play
                .as_ref()
                .map(|play| play.runtime().game_state().clone())
                .unwrap_or_default();
            player::GameScene::load(&path, ctx, self.viewport.gpu(), Camera::default(), state)
        })();
        match result {
            Ok(world) => {
                self.range_controls.release(ctx);
                self.audio.stop();
                self.report(format!("Playing {}", world.scene.name), false);
                self.play = Some(Play::Loaded(Box::new(world)));
            }
            Err(error) => {
                let message = format!("Scene {id:?}: {error}");
                if let Some(play) = &mut self.play {
                    match play {
                        Play::Edited { runtime, .. } => runtime.log_scene_error(message.clone()),
                        Play::Loaded(world) => world.runtime.log_scene_error(message.clone()),
                    }
                }
                self.report(message, true);
            }
        }
    }

    fn reset_scene_camera(&mut self) {
        self.feel.clear();
        self.viewport.reset_camera();
        if self.doc.scene.dimension == SceneDimension::TwoD {
            self.viewport.camera.eye = Vec3::Z * 10.0;
            self.viewport.camera.look_at(Vec3::ZERO);
            self.viewport.frame(Vec3::ZERO, 10.0);
        }
    }

    fn open_scene(&mut self, path: std::path::PathBuf) -> bool {
        if !self.confirm_close_document() {
            return false;
        }
        match Scene::load(&path) {
            Ok(scene) => {
                let errors = self.install_assets(&scene, &path);
                if let Some(error) = errors.first() {
                    self.report(
                        format!("{} asset(s) failed to load. {error}", errors.len()),
                        true,
                    );
                } else {
                    self.report(format!("Opened {}", file_name(&path)), false);
                }
                if self.project.as_ref().is_some_and(|project| {
                    path.canonicalize()
                        .ok()
                        .is_none_or(|path| !path.starts_with(&project.root))
                }) {
                    self.project = None;
                }
                self.doc = Document::new(scene, Some(path));
                self.reset_scene_camera();
                self.show_panel(Panel::Scene);
                true
            }
            Err(error) => {
                self.report(format!("Open failed: {error}"), true);
                false
            }
        }
    }

    fn save(&mut self, save_as: bool) -> bool {
        self.sync_external_scripts();
        if !self.apply_script_draft() {
            return false;
        }
        let path = if !save_as && self.doc.path.is_some() {
            self.doc.path.clone()
        } else {
            let mut dialog =
                scene_dialog().set_file_name(format!("{}.futty.scene.ron", self.doc.scene.name));
            if let Some(project) = &self.project {
                dialog = dialog.set_directory(&project.root);
            }
            dialog.save_file().map(|path| {
                if path.extension().is_none() {
                    path.with_extension("futty.scene.ron")
                } else {
                    path
                }
            })
        };
        let Some(path) = path else { return false };
        if let Some(project) = &self.project
            && path
                .parent()
                .and_then(|parent| parent.canonicalize().ok())
                .is_none_or(|parent| !parent.starts_with(&project.root))
        {
            self.report("Save the scene inside the project folder", true);
            return false;
        }
        if !self.doc.disk_matches_saved(&path) {
            self.report(
                "Scene changed on disk. Reopen it or use Save As with a new file name.",
                true,
            );
            return false;
        }
        self.refresh_scripts();
        match self
            .save_assets(&path)
            .and_then(|()| self.doc.scene.save(&path))
        {
            Ok(()) => {
                self.report(format!("Saved {}", file_name(&path)), false);
                self.doc.mark_saved(path);
                true
            }
            Err(error) => {
                self.report(format!("Save failed: {error}"), true);
                false
            }
        }
    }

    fn confirm_close_document(&mut self) -> bool {
        self.sync_external_scripts();
        if !self.apply_script_draft() {
            return false;
        }
        if !self.doc.dirty() {
            return true;
        }
        match MessageDialog::new()
            .set_title("Unsaved scene")
            .set_description("Save the current scene before closing it?")
            .set_buttons(MessageButtons::YesNoCancel)
            .show()
        {
            MessageDialogResult::Yes => self.save(false),
            MessageDialogResult::No => true,
            _ => false,
        }
    }

    fn export_windows(&mut self) {
        self.sync_external_scripts();
        if !self.apply_script_draft() {
            return;
        }
        if (self.doc.dirty() || self.doc.path.is_none()) && !self.save(false) {
            return;
        }
        let Some(source) = self
            .project
            .as_ref()
            .map(project::Project::path)
            .or(self.doc.path.as_deref())
        else {
            return;
        };
        let Some(parent) = FileDialog::new()
            .set_title("Choose Windows build location")
            .pick_folder()
        else {
            return;
        };
        let name = self
            .project
            .as_ref()
            .map_or(self.doc.scene.name.as_str(), project::Project::name);
        let stem: String = name
            .chars()
            .take(64)
            .map(|ch| {
                if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '-'
                }
            })
            .collect();
        let output = parent.join(format!("{stem}-Windows"));
        let result = std::env::current_exe()
            .map_err(Into::into)
            .and_then(|exe| player::export(source, &output, self.viewport.camera, &exe));
        match result {
            Ok(()) => self.report(
                format!(
                    "Windows build ready · {}",
                    output.join("Game.exe").display()
                ),
                false,
            ),
            Err(error) => self.report(format!("Export failed: {error}"), true),
        }
    }

    fn reload_play_tint(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let Some(Play::Edited { runtime, .. }) = &mut self.play else {
            return Err("Start Play from the saved edit scene first".into());
        };
        let path = self.doc.path.as_deref().ok_or("Save the scene first")?;
        let id = self
            .doc
            .selection
            .primary()
            .ok_or("Select an object before starting Play")?;
        let saved = Scene::load(path)?;
        if saved.scene_uuid != self.doc.scene.scene_uuid {
            return Err("The file now contains a different scene".into());
        }
        let object = saved
            .entities
            .iter()
            .find(|object| object.uuid == id && object.has_renderer())
            .ok_or("The selected object has no saved renderer")?;
        runtime.set_play_tint(id, object.color)?;
        Ok(format!("Reloaded {} tint in Play", object.name))
    }

    fn execute(&mut self, action: Action) {
        match action {
            Action::ReloadPlayTint => {
                match self.reload_play_tint() {
                    Ok(message) => self.report(message, false),
                    Err(error) => self.report(error.to_string(), true),
                }
                return;
            }
            #[cfg(windows)]
            Action::SetupAi(client) => {
                let mut dialog =
                    FileDialog::new().set_title("Choose a project folder for Futty MCP");
                if let Some(folder) = self
                    .project
                    .as_ref()
                    .map(project::Project::path)
                    .or(self.doc.path.as_deref())
                    .and_then(std::path::Path::parent)
                {
                    dialog = dialog.set_directory(folder);
                }
                if let Some(folder) = dialog.pick_folder() {
                    let (message, error) = match ai_setup::install(client, &folder) {
                        Ok(message) => (message.to_owned(), false),
                        Err(error) => (error.to_string(), true),
                    };
                    MessageDialog::new()
                        .set_title("Futty AI Setup")
                        .set_description(&message)
                        .set_level(if error {
                            MessageLevel::Error
                        } else {
                            MessageLevel::Info
                        })
                        .show();
                    self.report(message, error);
                }
                return;
            }
            Action::EditMaterial(id) => {
                self.inspected_asset = Some(InspectedAsset::Material(id));
                self.show_panel(Panel::Inspector);
                return;
            }
            Action::InspectPrefab(id) => {
                self.inspected_asset = Some(InspectedAsset::Prefab(id));
                self.show_panel(Panel::Inspector);
                return;
            }
            Action::ShowPanel(panel) => {
                self.show_panel(panel);
                return;
            }
            Action::DockPanel(panel) => {
                workspace::dock_panel(&mut self.dock, panel);
                return;
            }
            Action::ResetLayout => {
                self.dock = workspace::default_layout();
                if self.play.is_some() {
                    self.show_panel(Panel::Game);
                }
                self.report("Default layout restored", false);
                return;
            }
            _ => {}
        }
        if matches!(action, Action::PlayStop | Action::PreviewFeel) {
            self.viewport.reset_game_quality();
            self.sync_external_scripts();
            if let Some(play) = self.play.take() {
                self.diagnostics = workspace::RunDiagnostics::capture(play.runtime());
                self.audio.stop();
                self.show_panel(Panel::Scene);
                self.report("Stopped. Edit scene restored.", false);
            } else if self.apply_script_draft() {
                self.refresh_scripts();
                let preview = if matches!(action, Action::PreviewFeel) {
                    match self.feel.preview(&self.doc.scene) {
                        Ok(scene) => Some(scene),
                        Err(error) => {
                            self.report(error, true);
                            return;
                        }
                    }
                } else {
                    None
                };
                let scene = preview.as_ref().unwrap_or(&self.doc.scene);
                match Runtime::new(scene) {
                    Ok(mut runtime) => {
                        self.audio.stop();
                        runtime.start_scripts(scene, &self.script_sources());
                        self.script_editor = None;
                        self.play = Some(Play::Edited {
                            runtime: Box::new(runtime),
                            camera: self.viewport.camera,
                        });
                        self.show_panel(Panel::Game);
                        if preview.is_some() {
                            self.report(
                                "Previewing the Feel draft. Stop, then apply to keep it.",
                                false,
                            );
                        } else {
                            self.report("Playing a copy of the scene", false);
                        }
                    }
                    Err(error) => self.report(format!("Could not start Play: {error}"), true),
                }
            }
            return;
        }
        if self.play.is_some() {
            return;
        }
        if let Action::Project(choice) = action {
            self.project_action(choice);
            return;
        }
        match action {
            Action::CreatePrefab => {
                if self.doc.selection.ids().len() != 1 {
                    self.report(
                        "Select one root object. Its children will be included.",
                        true,
                    );
                    return;
                }
                match self.doc.create_prefab() {
                    Ok(id) => {
                        self.inspected_asset = Some(InspectedAsset::Prefab(id));
                        self.show_panel(Panel::Inspector);
                        self.report("Prefab saved in Project with its child hierarchy", false);
                    }
                    Err(error) => self.report(error, true),
                }
                return;
            }
            Action::PlacePrefab(id) => {
                self.place_prefab(id);
                return;
            }
            Action::RemovePrefab(id) => {
                let before = self.doc.scene.clone();
                self.doc.scene.remove_prefab(id);
                self.doc.record_frame(before, false);
                if self.inspected_asset == Some(InspectedAsset::Prefab(id)) {
                    self.inspected_asset = None;
                }
                return;
            }
            Action::EditPrefabSource(id) => {
                if let Some(source) = self.doc.scene.prefab_source(id) {
                    let before = self.doc.scene.clone();
                    if let Some(prefab) = self.doc.scene.prefabs.get_mut(&id) {
                        prefab.source = Some(source);
                    }
                    self.doc.record_frame(before, false);
                    self.doc.selection.replace(Some(source));
                    self.inspected_asset = None;
                    self.script_inspector = false;
                    self.hierarchy_search.clear();
                    self.show_panel(Panel::Inspector);
                    self.show_panel(Panel::Scene);
                    self.focus_selection();
                }
                return;
            }
            Action::UnpackPrefab(root) => {
                let before = self.doc.scene.clone();
                self.doc.scene.detach_prefab(root);
                self.doc.record_frame(before, false);
                self.report("Prefab instance unpacked", false);
                return;
            }
            Action::ResetPrefabOverrides(id) => {
                let before = self.doc.scene.clone();
                self.doc.scene.reset_prefab_overrides(id);
                self.doc.record_frame(before, false);
                self.report("Prefab overrides reset", false);
                return;
            }
            Action::ReparentSelection(parent) => {
                let ids = self.doc.selection.ids().to_vec();
                match self.doc.reparent(&ids, parent) {
                    Ok(()) => self.report("Parent updated", false),
                    Err(error) => self.report(error, true),
                }
                return;
            }
            Action::ReparentObject(id, parent) => {
                match self.doc.reparent(&[id], parent) {
                    Ok(()) => self.report("Parent updated", false),
                    Err(error) => self.report(error, true),
                }
                return;
            }
            Action::Delete => {
                self.doc.delete_selection();
                return;
            }
            Action::Duplicate => {
                self.doc.duplicate_selection();
                return;
            }
            _ => {}
        }
        let before = self.doc.scene.clone();
        match action {
            Action::New(dimension) if self.confirm_close_document() => {
                self.clear_assets();
                self.doc = Document::new(Scene::new(dimension), None);
                self.reset_scene_camera();
                self.report("New scene", false);
            }
            Action::PhysicsPlayground if self.confirm_close_document() => {
                self.clear_assets();
                self.doc = Document::new(physics::playground(), None);
                self.viewport.camera.eye = Vec3::new(8.3, 6.2, 10.7);
                self.viewport.camera.look_at(Vec3::Y * 1.9);
                self.report(
                    "Physics Playground ready. Press Play to drop the cubes.",
                    false,
                );
            }
            Action::ScriptPlayground if self.confirm_close_document() => {
                self.load_script_playground()
            }
            Action::AudioPlayground if self.confirm_close_document() => {
                self.load_audio_playground()
            }
            Action::SpritePlayground if self.confirm_close_document() => {
                self.load_sprite_playground()
            }
            Action::Open => {
                if let Some(path) = scene_dialog().pick_file() {
                    self.open_scene(path);
                }
            }
            Action::Save | Action::SaveAs => {
                self.save(matches!(action, Action::SaveAs));
            }
            Action::Add(primitive) => {
                self.inspected_asset = None;
                let x = self
                    .doc
                    .scene
                    .entities
                    .iter()
                    .filter(|object| object.parent.is_none())
                    .map(|object| object.transform.translation.x)
                    .reduce(f32::max)
                    .map_or(0.0, |x| x + 1.5);
                let y = -primitive.bounds()[0].y;
                let mut object = Object::cube(primitive.label().into(), Vec3::new(x, y, 0.0));
                object.primitive = primitive;
                object.spin = primitive.is_cube();
                self.hierarchy_search.clear();
                self.doc.selection.replace(Some(object.uuid));
                self.doc.scene.entities.push(object);
                self.script_inspector = false;
                self.focus_selection();
            }
            Action::AddCamera => {
                self.inspected_asset = None;
                self.add_camera();
            }
            Action::AddLight(kind) => {
                self.inspected_asset = None;
                self.add_light(kind);
            }
            Action::SetMainCamera(id) => {
                if self
                    .doc
                    .scene
                    .entities
                    .iter()
                    .any(|object| object.uuid == id && object.camera.is_some())
                {
                    self.doc.scene.main_camera = Some(id);
                    self.camera_preview = None;
                    self.report("Main Camera updated", false);
                }
            }
            Action::PreviewCamera(id) => {
                self.camera_preview = Some(id);
                self.show_panel(Panel::Game);
            }
            Action::AlignCamera(id) => self.align_camera(id),
            Action::SelectAll => {
                let Ok(hierarchy) = self.doc.scene.hierarchy() else {
                    return;
                };
                let rows = self
                    .hierarchy_rows(&hierarchy)
                    .into_iter()
                    .map(|(index, _)| self.doc.scene.entities[index].uuid)
                    .collect::<Vec<_>>();
                self.doc.selection.replace(rows);
            }
            Action::Undo => self.doc.undo(),
            Action::ExportWindows => self.export_windows(),
            Action::Redo => self.doc.redo(),
            Action::Focus => {
                self.show_panel(Panel::Scene);
                self.focus_selection();
            }
            Action::FrameAll => self.frame_objects(true),
            Action::Import => self.import_dialog(),
            Action::ReimportModel(id) => self.reimport_model(id),
            Action::Place(id) => {
                self.inspected_asset = None;
                self.place_model(id);
            }
            Action::PlaceSprite(id) => {
                self.inspected_asset = None;
                self.place_sprite(id);
            }
            Action::ApplyTexture(id) => {
                for object in self.doc.scene.entities.iter_mut().filter(|object| {
                    object.has_renderer() && self.doc.selection.ids().contains(&object.uuid)
                }) {
                    if let Some(sprite) = &mut object.sprite {
                        sprite.texture = id;
                    } else {
                        object.material = None;
                        object.texture = Some(id);
                    }
                }
            }
            Action::AttachScript(id) => {
                for object in self
                    .doc
                    .scene
                    .entities
                    .iter_mut()
                    .filter(|object| self.doc.selection.ids().contains(&object.uuid))
                {
                    script_editor::attach(object, Some(id));
                    self.script_inspector = true;
                }
            }
            Action::NewScript => self.create_script(),
            Action::NewMaterial => {
                let id = Uuid::new_v4();
                let material = futty_runtime::Material {
                    name: format!("Material {}", self.doc.scene.materials.len() + 1),
                    ..Default::default()
                };
                self.doc.scene.materials.insert(id, material);
                self.inspected_asset = Some(InspectedAsset::Material(id));
                self.show_panel(Panel::Inspector);
            }
            Action::ApplyMaterial(id) if self.doc.scene.materials.contains_key(&id) => {
                for object in self.doc.scene.entities.iter_mut().filter(|object| {
                    object.has_renderer()
                        && object.sprite.is_none()
                        && self.doc.selection.ids().contains(&object.uuid)
                }) {
                    material::assign(object, Some(id));
                }
            }
            Action::RemoveMaterial(id) => {
                if self
                    .doc
                    .scene
                    .referenced_objects()
                    .all(|object| object.material != Some(id))
                {
                    self.doc.scene.materials.remove(&id);
                    if self.inspected_asset == Some(InspectedAsset::Material(id)) {
                        self.inspected_asset = None;
                    }
                }
            }
            Action::EditScript(id) => self.edit_script_in_code(id),
            Action::EditScriptInFutty(id) => self.open_script(id),
            Action::ApplyScriptDraft { clear } => {
                if self.apply_script_draft() && clear {
                    self.script_editor = None;
                }
            }
            Action::AttachAudio(clip) => {
                for object in self
                    .doc
                    .scene
                    .entities
                    .iter_mut()
                    .filter(|object| self.doc.selection.ids().contains(&object.uuid))
                {
                    object.audio_source = Some(AudioSource {
                        clip,
                        ..object
                            .audio_source
                            .unwrap_or_else(|| AudioSource::new(clip))
                    });
                }
            }
            Action::PreviewAudio(clip) => match self.audio.preview(clip) {
                Ok(()) => self.report("Audio preview started", false),
                Err(error) => self.report(format!("Audio preview failed: {error}"), true),
            },
            Action::StopAudioPreview => {
                self.audio.stop_preview();
                self.report("Audio preview stopped", false);
            }
            Action::RemoveAsset(id) => {
                if self
                    .doc
                    .scene
                    .hud
                    .iter()
                    .any(|widget| widget.image == Some(id))
                {
                    self.report("The asset is used by a HUD image", true);
                } else {
                    self.doc.scene.assets.retain(|asset| asset.id != id);
                    self.doc.scene.script_sources.remove(&id);
                }
            }
            _ => {}
        }
        if matches!(
            action,
            Action::Add(_)
                | Action::AddCamera
                | Action::AddLight(_)
                | Action::SetMainCamera(_)
                | Action::AlignCamera(_)
                | Action::Import
                | Action::Place(_)
                | Action::PlaceSprite(_)
                | Action::ApplyTexture(_)
                | Action::AttachScript(_)
                | Action::AttachAudio(_)
                | Action::RemoveAsset(_)
                | Action::NewMaterial
                | Action::ApplyMaterial(_)
                | Action::RemoveMaterial(_)
        ) {
            self.doc.record_frame(before, false);
        }
    }

    fn selected_subtree(&self, selected: &[Uuid]) -> std::collections::HashSet<Uuid> {
        let scene = &self.doc.scene;
        let Ok(hierarchy) = scene.hierarchy() else {
            return Default::default();
        };
        selected_tree(&scene.entities, &hierarchy, selected)
            .map(|(index, _)| scene.entities[index].uuid)
            .collect()
    }

    fn focus_selection(&mut self) {
        self.frame_objects(false);
    }

    fn frame_objects(&mut self, all: bool) {
        let scene = &self.doc.scene;
        let selected = self.selected_subtree(self.doc.selection.ids());
        if !all && selected.is_empty() {
            return;
        };
        let Ok(hierarchy) = scene.hierarchy() else {
            return;
        };
        let worlds = hierarchy.world_matrices(|index| scene.entities[index].transform.matrix());
        let mut low = Vec3::splat(f32::INFINITY);
        let mut high = Vec3::splat(f32::NEG_INFINITY);
        for (object, world) in scene
            .entities
            .iter()
            .zip(worlds)
            .filter(|(object, _)| all || selected.contains(&object.uuid))
        {
            let world = object.sprite.map_or(world, |sprite| {
                sprite.matrix(world, self.viewport.camera.view())
            });
            let [min, max] = self.viewport.object_bounds(object);
            let center = world.transform_point3((min + max) * 0.5);
            let half = (max - min) * 0.5;
            let extent = world.x_axis.truncate().abs() * half.x
                + world.y_axis.truncate().abs() * half.y
                + world.z_axis.truncate().abs() * half.z;
            low = low.min(center - extent);
            high = high.max(center + extent);
        }
        let focus = (low + high) * 0.5;
        let radius = ((high - low) * 0.5).length();
        if focus.is_finite() && radius.is_finite() {
            let aspect = self
                .scene_rect
                .map_or(1.0, |rect| rect.aspect_ratio())
                .min(1.0);
            let distance = radius.max(0.35) * (2.7 / aspect.max(0.2));
            self.viewport.frame(focus, distance);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let playing = self.play.is_some();
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), 29.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(RichText::new("futty").size(23.0).strong());
                ui.label(RichText::new("Editor").size(15.0).strong());
                ui.add_space(8.0);
                ui.separator();
                ui.add_enabled_ui(!playing, |ui| {
                    ui.menu_button("File", |ui| {
                        menu_item(
                            ui,
                            "New 3D Scene",
                            Action::New(SceneDimension::ThreeD),
                            true,
                            action,
                        );
                        menu_item(
                            ui,
                            "New 2D Scene",
                            Action::New(SceneDimension::TwoD),
                            true,
                            action,
                        );
                        menu_item(
                            ui,
                            "Physics Playground",
                            Action::PhysicsPlayground,
                            true,
                            action,
                        );
                        menu_item(
                            ui,
                            "Script Playground",
                            Action::ScriptPlayground,
                            true,
                            action,
                        );
                        menu_item(
                            ui,
                            "Audio Playground",
                            Action::AudioPlayground,
                            true,
                            action,
                        );
                        menu_item(
                            ui,
                            "Sprite Playground",
                            Action::SpritePlayground,
                            true,
                            action,
                        );
                        menu_item(ui, "Open Scene…", Action::Open, true, action);
                        ui.separator();
                        menu_item(ui, "Save Scene     Ctrl+S", Action::Save, true, action);
                        menu_item(ui, "Save Scene As…", Action::SaveAs, true, action);
                        ui.separator();
                        self.project_menu(ui, action);
                        ui.separator();
                        menu_item(
                            ui,
                            if self.project.is_some() {
                                "Export Windows Project…"
                            } else {
                                "Export Windows Game…"
                            },
                            Action::ExportWindows,
                            cfg!(target_os = "windows"),
                            action,
                        );
                    });
                    ui.menu_button("Edit", |ui| {
                        menu_item(
                            ui,
                            "Undo     Ctrl+Z",
                            Action::Undo,
                            self.doc.can_undo(),
                            action,
                        );
                        menu_item(
                            ui,
                            "Redo     Ctrl+Y",
                            Action::Redo,
                            self.doc.can_redo(),
                            action,
                        );
                        ui.separator();
                        menu_item(
                            ui,
                            "Duplicate     Ctrl+D",
                            Action::Duplicate,
                            self.doc.selection.primary().is_some(),
                            action,
                        );
                        menu_item(
                            ui,
                            "Delete",
                            Action::Delete,
                            self.doc.selection.primary().is_some(),
                            action,
                        );
                    });
                });
                ui.menu_button("Window", |ui| {
                    workspace::panels_menu(ui, action);
                    ui.separator();
                    if ui.button("Game HUD").clicked() {
                        self.hud_open = true;
                        self.show_panel(Panel::Game);
                        ui.close();
                    }
                    if ui.button("Futty Feel").clicked() {
                        self.feel.open = true;
                        ui.close();
                    }
                    menu_item(
                        ui,
                        "Reload Selected Tint in Play",
                        Action::ReloadPlayTint,
                        matches!(self.play, Some(Play::Edited { .. }))
                            && self.doc.path.is_some()
                            && self.doc.selection.primary().is_some(),
                        action,
                    );
                    if ui.button("Game Controls     F10").clicked() {
                        self.controls_open = true;
                        ui.close();
                    }
                });
                #[cfg(windows)]
                ui.add_enabled_ui(!playing, |ui| {
                    ui.menu_button("AI", |ui| {
                        for client in ai_setup::Client::ALL {
                            menu_item(ui, client.name(), Action::SetupAi(client), true, action);
                        }
                    });
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.menu_button("Layout", |ui| {
                        menu_item(ui, "Reset to Default", Action::ResetLayout, true, action);
                    });
                    let filename = self
                        .doc
                        .path
                        .as_deref()
                        .map(file_name)
                        .unwrap_or("Untitled scene");
                    let dirty = if self.doc.dirty() { " • Unsaved" } else { "" };
                    ui.add(
                        egui::Label::new(RichText::new(format!("{filename}{dirty}")).color(MUTED))
                            .truncate(),
                    );
                });
            },
        );
        ui.separator();
        let label = if playing { "■  Stop" } else { "▶  Play" };
        let (fill, color) = if playing {
            (Color32::from_rgb(48, 59, 72), FOCUS)
        } else {
            (Color32::from_rgb(175, 196, 217), BACKGROUND)
        };
        let bar = ui.available_rect_before_wrap();
        let rect = egui::Rect::from_center_size(bar.center(), egui::vec2(90.0, 27.0));
        if ui
            .put(
                rect,
                egui::Button::new(RichText::new(label).strong().color(color)).fill(fill),
            )
            .clicked()
        {
            *action = Some(Action::PlayStop);
        }
    }

    fn hierarchy(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(self.play.is_none(), |ui| {
                    ui.menu_button("+ Add", |ui| {
                        context_menu::add_objects(ui, action);
                    });
                });
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            let width = (ui.available_width()
                - if self.hierarchy_search.is_empty() {
                    4.0
                } else {
                    55.0
                })
            .max(45.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.hierarchy_search)
                    .hint_text("Search objects")
                    .desired_width(width),
            );
            if !self.hierarchy_search.is_empty() && ui.small_button("Clear").clicked() {
                self.hierarchy_search.clear();
            }
        });
        let root_row = ui
            .add(
                egui::Label::new(RichText::new(&self.doc.scene.name).strong())
                    .sense(egui::Sense::hover()),
            )
            .on_hover_text("Drop objects here to move them to the scene root");
        hierarchy_drop(ui, &root_row, None, self.play.is_none(), action);
        let Ok(hierarchy) = self.doc.scene.hierarchy() else {
            return;
        };
        let rows = self.hierarchy_rows(&hierarchy);
        if rows.is_empty() {
            ui.label(
                RichText::new(if self.hierarchy_search.trim().is_empty() {
                    "Add an object to get started."
                } else {
                    "No matches. Try a different name."
                })
                .color(MUTED),
            );
        }
        let ids: Vec<_> = rows
            .iter()
            .map(|&(index, _)| self.doc.scene.entities[index].uuid)
            .collect();
        let count = self.doc.selection.ids().len();
        if count > 1 {
            ui.label(RichText::new(format!("{count} selected")).color(FOCUS));
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.indent("scene-objects", |ui| {
                for (index, depth) in rows {
                    let object = &self.doc.scene.entities[index];
                    let id = object.uuid;
                    let name = object.name.clone();
                    let selected = self.doc.selection.ids().contains(&id);
                    let label = RichText::new(format!(
                        "{}  {}",
                        if object.camera.is_some() {
                            "▣"
                        } else if object.light.is_some() {
                            "☼"
                        } else {
                            "◇"
                        },
                        name
                    ));
                    ui.push_id(id, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space((depth as f32 * 13.0).min(ui.available_width() * 0.35));
                            let response = ui
                                .add_sized(
                                    [ui.available_width(), 26.0],
                                    egui::Button::selectable(selected, label)
                                        .truncate()
                                        .sense(egui::Sense::click_and_drag()),
                                )
                                .on_hover_text(format!("{name} · Right-click for actions"));
                            let modifiers = ui.input(|input| input.modifiers);
                            if response.clicked() && ui.ctx().current_pass_index() == 0 {
                                self.inspected_asset = None;
                                self.doc.selection.click(Some(id), modifiers, &ids);
                            }
                            if response.double_clicked()
                                && !modifiers.command
                                && !modifiers.shift
                                && self.play.is_none()
                            {
                                *action = Some(Action::Focus);
                            }
                            if response.secondary_clicked() && ui.ctx().current_pass_index() == 0 {
                                self.inspected_asset = None;
                                self.doc.selection.context_target(id);
                            }
                            if self.play.is_none() {
                                if response.drag_started_by(egui::PointerButton::Primary) {
                                    self.doc.selection.context_target(id);
                                }
                                response.dnd_set_drag_payload(HierarchyDrag(id));
                                hierarchy_drop(ui, &response, Some(id), true, action);
                            }
                            response.context_menu(|ui| self.hierarchy_menu(ui, action));
                        })
                    });
                }
            });
        });
        let empty = ui.allocate_response(
            ui.available_size().max(egui::Vec2::ZERO),
            egui::Sense::click(),
        );
        hierarchy_drop(ui, &empty, None, self.play.is_none(), action);
        empty.context_menu(|ui| self.hierarchy_menu(ui, action));
    }

    fn hierarchy_rows(&self, hierarchy: &futty_runtime::Hierarchy) -> Vec<(usize, usize)> {
        let query = self.hierarchy_search.trim().to_lowercase();
        hierarchy
            .order
            .iter()
            .copied()
            .filter_map(|(index, depth)| {
                if query.is_empty() {
                    Some((index, depth))
                } else {
                    self.doc.scene.entities[index]
                        .name
                        .to_lowercase()
                        .contains(&query)
                        .then_some((index, 0))
                }
            })
            .collect()
    }

    fn inspector(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let dimension = self.doc.scene.dimension;
        if self.material_inspector(ui) || self.prefab_inspector(ui, action) {
            return;
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.script_inspector, false, "Object");
            ui.selectable_value(&mut self.script_inspector, true, "Script");
        });
        let primary = self.doc.selection.primary();
        let ids = self.doc.selection.ids();
        if ids.len() > 1
            && let Some(object) = self
                .doc
                .scene
                .entities
                .iter()
                .find(|object| Some(object.uuid) == primary)
        {
            ui.add(
                egui::Label::new(
                    RichText::new(format!("{} selected · Editing {}", ids.len(), object.name))
                        .color(FOCUS),
                )
                .truncate(),
            );
        }
        if self.script_inspector {
            self.show_script_inspector(ui, action);
            return;
        }
        let subtree = self.selected_subtree(primary.as_slice());
        let parent_choices: Vec<_> = self
            .doc
            .scene
            .entities
            .iter()
            .filter(|object| !subtree.contains(&object.uuid))
            .map(|object| (object.uuid, object.name.clone()))
            .collect();
        let assets = &self.doc.scene.assets;
        let materials = &self.doc.scene.materials;
        let light_beyond_limit = self
            .doc
            .scene
            .entities
            .iter()
            .filter(|object| object.light.is_some())
            .skip(futty_render::MAX_LIGHTS)
            .any(|object| Some(object.uuid) == primary);
        let main_camera = self
            .doc
            .scene
            .camera_index(None)
            .map(|index| self.doc.scene.entities[index].uuid);
        let listener_available = self
            .doc
            .scene
            .entities
            .iter()
            .all(|object| object.audio_listener.is_none() || Some(object.uuid) == primary);
        let Some(object) = self
            .doc
            .scene
            .entities
            .iter_mut()
            .find(|object| Some(object.uuid) == primary)
        else {
            ui.label(RichText::new("Select an object to edit it.").color(MUTED));
            return;
        };
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.push_id(object.uuid, |ui| {
                ui.add_enabled_ui(self.play.is_none(), |ui| {
                    if let Some(link) = object.prefab
                        && let Some(prefab) = self.doc.scene.prefabs.get(&link.asset)
                    {
                        ui.label(format!("Prefab · {}", prefab.name));
                        ui.horizontal(|ui| {
                            if ui.button("Inspect Prefab").clicked() {
                                *action = Some(Action::InspectPrefab(link.asset));
                            }
                            if ui.button("Unpack").clicked() {
                                *action = Some(Action::UnpackPrefab(link.instance));
                            }
                        });
                        if link.has_overrides() && ui.button("Reset Overrides").clicked() {
                            *action = Some(Action::ResetPrefabOverrides(object.uuid));
                        }
                        ui.separator();
                    } else if self
                        .doc
                        .scene
                        .prefabs
                        .values()
                        .any(|prefab| prefab.source == Some(object.uuid))
                    {
                        ui.label(RichText::new("Prefab source").color(MUTED));
                    }
                    ui.label("Name");
                    ui.add(
                        egui::TextEdit::singleline(&mut object.name).desired_width(f32::INFINITY),
                    );
                    ui.label("Tags");
                    let tag_id = ui.make_persistent_id(("object-tags", object.uuid));
                    let mut text = ui
                        .data(|data| data.get_temp::<String>(tag_id))
                        .unwrap_or_else(|| object.tags.join(", "));
                    let tag_field = ui.add(
                        egui::TextEdit::singleline(&mut text)
                            .id(tag_id)
                            .hint_text("Comma-separated tags")
                            .desired_width(f32::INFINITY),
                    );
                    if tag_field.changed() {
                        let mut seen = std::collections::HashSet::new();
                        object.tags = text
                            .split(',')
                            .map(str::trim)
                            .filter(|tag| {
                                !tag.is_empty()
                                    && tag.len() <= 64
                                    && !tag.chars().any(char::is_control)
                                    && seen.insert(*tag)
                            })
                            .take(32)
                            .map(str::to_owned)
                            .collect();
                    }
                    let editing_tags = tag_field.has_focus();
                    ui.data_mut(|data| {
                        if editing_tags {
                            data.insert_temp(tag_id, text);
                        } else {
                            data.remove_temp::<String>(tag_id);
                        }
                    });
                    let parent_name = parent_choices
                        .iter()
                        .find(|(id, _)| Some(*id) == object.parent)
                        .map_or("Scene root", |(_, name)| name.as_str());
                    ui.label("Parent");
                    ui.add_enabled_ui(object.body.is_none() && object.character.is_none(), |ui| {
                        let mut parent = object.parent;
                        egui::ComboBox::from_id_salt("object-parent")
                            .width(ui.available_width())
                            .selected_text(parent_name)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut parent, None, "Scene root");
                                for (id, name) in &parent_choices {
                                    ui.selectable_value(&mut parent, Some(*id), name);
                                }
                            });
                        if parent != object.parent {
                            *action = Some(Action::ReparentObject(object.uuid, parent));
                        }
                    });
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(if object.parent.is_some() {
                            "Local Transform"
                        } else {
                            "Transform"
                        })
                        .strong(),
                    );
                    ui.separator();
                    vector_fields(
                        ui,
                        "Position",
                        &mut object.transform.translation,
                        -10_000.0..=10_000.0,
                        0.02,
                    );
                    let (x, y, z) = object.transform.rotation.to_euler(EulerRot::XYZ);
                    let mut angles = Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees());
                    if vector_fields(ui, "Rotation (deg)", &mut angles, -360.0..=360.0, 0.5) {
                        object.transform.rotation = Quat::from_euler(
                            EulerRot::XYZ,
                            angles.x.to_radians(),
                            angles.y.to_radians(),
                            angles.z.to_radians(),
                        )
                        .normalize();
                    }
                    vector_fields(
                        ui,
                        "Scale",
                        &mut object.transform.scale,
                        0.01..=1000.0,
                        0.02,
                    );
                    ui.add_space(10.0);
                    if let Some(lens) = &mut object.camera {
                        camera::inspector(ui, lens, main_camera == Some(object.uuid));
                    } else if let Some(light) = &mut object.light {
                        light::inspector(ui, light);
                        if light_beyond_limit {
                            ui.label(
                                RichText::new("Only the first 8 lights affect the scene.")
                                    .color(WARNING),
                            );
                        }
                    } else if let Some(sprite) = &mut object.sprite {
                        sprite::inspector(ui, sprite, assets, &self.asset_bytes);
                    } else {
                        ui.label(RichText::new("Mesh Renderer").strong());
                        ui.separator();
                        let mesh = assets
                            .iter()
                            .find(|asset| Some(asset.id) == object.mesh)
                            .map_or(object.primitive.label(), |asset| asset.name.as_str());
                        ui.label(format!("Mesh  ·  {mesh}"));
                        if let Some(mesh) = object.mesh {
                            animation::inspector(
                                ui,
                                object,
                                self.viewport.clips(mesh),
                                &mut self.animation_preview,
                            );
                        }
                        material::selector(ui, object, materials, action);
                        ui.add_enabled_ui(object.material.is_none(), |ui| {
                            let texture = assets
                                .iter()
                                .find(|asset| Some(asset.id) == object.texture)
                                .map_or("Default material", |asset| asset.name.as_str());
                            egui::ComboBox::from_id_salt("base-texture")
                                .width(ui.available_width())
                                .selected_text(texture)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut object.texture,
                                        None,
                                        "Default material",
                                    );
                                    for asset in assets
                                        .iter()
                                        .filter(|asset| asset.kind == AssetKind::Texture)
                                    {
                                        ui.add_enabled_ui(
                                            self.asset_bytes.contains_key(&asset.id),
                                            |ui| {
                                                ui.selectable_value(
                                                    &mut object.texture,
                                                    Some(asset.id),
                                                    &asset.name,
                                                );
                                            },
                                        );
                                    }
                                });
                        });
                    }
                    if object.has_renderer() {
                        ui.horizontal(|ui| {
                            let mut color = object.color;
                            if ui.color_edit_button_rgb(&mut color).changed() {
                                object.color = color;
                            }
                            ui.label(if object.sprite.is_some() || object.material.is_some() {
                                "Tint"
                            } else {
                                "Base color"
                            });
                        });
                    }
                    audio::inspector(ui, object, assets, listener_available);
                    let bounds = object.sprite.map_or_else(
                        || self.viewport.object_bounds(object),
                        |sprite| {
                            futty_runtime::Sprite::BOUNDS
                                .map(|point| point * sprite.size.extend(1.0))
                        },
                    );
                    physics::inspector(ui, object, bounds, dimension);
                });
            })
        });
    }

    fn scene_view(
        &mut self,
        ui: &mut egui::Ui,
        modal: bool,
        game: bool,
        action: &mut Option<Action>,
    ) {
        let playing = self.play.is_some();
        if !game {
            self.scene_camera_controls(ui, action, playing, modal);
        }
        if game && let Some(Play::Loaded(world)) = &mut self.play {
            ui.label(format!("Playing {}", world.scene.name));
            let size = ui.available_size();
            if size.x >= 2.0 && size.y >= 2.0 {
                let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
                self.game_rect = Some(rect);
                self.game_aspect = rect.aspect_ratio();
                let settings = self.input_settings.with_profile(&world.scene.input);
                world.paint(
                    ui,
                    &response,
                    &mut self.range_controls,
                    &settings,
                    !modal && !self.controls_open && !self.hud_open,
                );
            }
            return;
        }
        let scene = &self.doc.scene;
        let camera_index = if game {
            scene.camera_index(if playing { None } else { self.camera_preview })
        } else {
            None
        };
        if game {
            ui.horizontal(|ui| {
                ui.label(if playing { "Playing" } else { "Preview" });
                let source = camera_index
                    .map_or("Scene camera", |index| scene.entities[index].name.as_str());
                ui.add(egui::Label::new(RichText::new(source).color(MUTED)).truncate());
                if !playing
                    && self.camera_preview.is_some()
                    && ui.button("Use Main Camera").clicked()
                {
                    self.camera_preview = None;
                }
            });
        }
        let size = ui.available_size();
        if size.x < 2.0 || size.y < 2.0 {
            return;
        }
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        if game {
            self.game_rect = Some(rect);
            self.game_aspect = rect.aspect_ratio();
        } else {
            self.scene_rect = Some(rect);
        }
        let Ok(hierarchy) = self.doc.scene.hierarchy() else {
            return;
        };
        if !game && !modal {
            if response.clicked() || response.secondary_clicked() {
                self.inspected_asset = None;
            }
            let orbiting = self.viewport.navigate_scene(&response);
            if !playing && !orbiting {
                self.viewport.select_and_move(
                    &response,
                    &mut self.doc.scene.entities,
                    &hierarchy,
                    &mut self.doc.selection,
                    self.move_tool,
                );
            }
            if !orbiting && !self.viewport.flew_with_right_button {
                if !playing
                    && response.secondary_clicked()
                    && ui.ctx().current_pass_index() == 0
                    && let Some(pos) = response.interact_pointer_pos()
                {
                    let objects = &self.doc.scene.entities;
                    let worlds =
                        hierarchy.world_matrices(|index| objects[index].transform.matrix());
                    if let Some(id) = self.viewport.pick_object(rect, pos, objects, worlds) {
                        self.doc.selection.context_target(id);
                    }
                }
                response.context_menu(|ui| self.scene_menu(ui, action));
            }
        }
        let mut camera = self.viewport.camera;
        if game
            && camera_index.is_none()
            && let Some(Play::Edited {
                camera: play_camera,
                ..
            }) = &mut self.play
        {
            *play_camera = self.viewport.navigate(&response, *play_camera);
            camera = *play_camera;
        }
        let scene = &self.doc.scene;
        let objects = &scene.entities;
        let preview = self
            .transform_tool
            .as_ref()
            .filter(|_| !playing)
            .map(|tool| &tool.preview);
        let selected = if game { &[] } else { self.doc.selection.ids() };
        let runtime = match &mut self.play {
            Some(Play::Edited { runtime, .. }) => Some(runtime.as_mut()),
            _ => None,
        };
        let mut frame = player::draw_objects(
            &self.doc.scene,
            &hierarchy,
            runtime,
            camera,
            camera_index,
            selected,
            preview,
        );
        if !playing
            && let Some(preview) = &self.animation_preview
            && self.doc.selection.primary() == Some(preview.object)
            && let Some((index, object)) = objects
                .iter()
                .filter(|object| object.has_renderer())
                .enumerate()
                .find(|(_, object)| object.uuid == preview.object)
            && let Some(sample) = preview.sample(object.animation)
        {
            frame.instances[index].animation = Some(sample);
        }
        let view = frame.render_scene(rect.aspect_ratio(), self.doc.scene.sky);
        if game && playing {
            self.viewport.render_game(
                ui,
                rect,
                view,
                !modal && !self.controls_open && !self.hud_open,
            );
        } else {
            self.viewport.render(ui, rect, view, game);
        }
        if game {
            let interactive = !modal
                && !self.controls_open
                && !self.hud_open
                && self.range_controls.cursor_free();
            let runtime = match &mut self.play {
                Some(Play::Edited { runtime, .. }) => Some(runtime.as_mut()),
                _ => None,
            };
            hud_view::paint(
                ui,
                rect,
                scene,
                &mut self.viewport,
                Some(&self.asset_bytes),
                runtime,
                interactive,
            );
            let capture = !modal
                && !self.controls_open
                && !self.hud_open
                && !hud_view::over_button(ui.ctx(), rect, &scene.hud);
            if let Some(status) = self
                .play
                .as_ref()
                .and_then(|play| play.runtime().range_status())
            {
                self.range_controls.paint(
                    ui,
                    &response,
                    status,
                    &self.input_settings.with_profile(&scene.input),
                    scene.hud.is_empty(),
                    capture,
                );
            } else if playing && scene.input.capture_mouse {
                self.range_controls
                    .paint_first_person(ui, &response, capture);
            }
        }
        if game && (!playing || camera_index.is_some()) {
            return;
        }
        if !game {
            let vp = camera.view_projection(rect.aspect_ratio());
            for (index, object) in objects
                .iter()
                .enumerate()
                .filter(|(_, object)| !object.has_renderer())
            {
                let selected = selected.contains(&object.uuid);
                if let Some(light) = object.light {
                    let position = frame.world[index].w_axis.truncate();
                    let rotation = hierarchy.world_rotation(index, |i| frame.local[i].rotation);
                    light::paint(ui, rect, vp, position, rotation, light, selected);
                }
                if let Some(view) =
                    self.doc
                        .scene
                        .camera_view(index, &hierarchy, &frame.world, &frame.local)
                {
                    camera::paint(ui, rect, vp, view, self.game_aspect, selected);
                }
            }
            if self.viewport.colliders {
                for (index, object) in objects
                    .iter()
                    .enumerate()
                    .filter(|(_, object)| selected.contains(&object.uuid))
                {
                    if let Some(body) = object.body {
                        viewport::paint_collider(
                            ui,
                            rect,
                            camera,
                            frame.world[index],
                            body,
                            scene.dimension,
                        );
                    }
                }
            }
            if !playing
                && !modal
                && self.move_tool
                && let Some(group) = TransformGroup::capture(
                    objects,
                    &hierarchy,
                    &frame.world,
                    selected,
                    self.viewport.transform_settings,
                )
            {
                self.viewport.paint_gizmo(ui, rect, &group);
            }
        }
        let controls = if !game && let Some(tool) = &self.transform_tool {
            tool.paint(ui, rect, camera);
            "X / Y / Z  Axis   ·   Type a value   ·   Enter / Click  Apply   ·   Esc  Cancel"
        } else if playing {
            "Right drag + WASD  Fly   ·   Scroll  Zoom"
        } else {
            if let Some((key, message, until)) = &self.transform_feedback
                && ui.input(|input| input.time) < *until
            {
                transform_tool::paint_banner(ui, rect, key, message, FOCUS);
            }
            "G  Move   ·   S  Scale   ·   R  Rotate   |   Right-click  Menu   ·   Right drag  Fly"
        };
        let font = egui::FontId::proportional(12.0);
        let painter = ui.painter().with_clip_rect(rect);
        let galley = painter.layout_no_wrap(controls.into(), font, MUTED);
        let origin = rect.left_bottom() + egui::vec2(14.0, -galley.size().y - 14.0);
        painter.rect_filled(
            egui::Rect::from_min_size(origin, galley.size()).expand2(egui::vec2(7.0, 4.0)),
            4.0,
            PANEL,
        );
        painter.galley(origin, galley, MUTED);
    }

    fn update_transform_tool(&mut self, ctx: &egui::Context) -> bool {
        let Some(rect) = self.scene_rect else {
            return false;
        };
        if self.transform_tool.is_none()
            && self.play.is_none()
            && !ctx.text_edit_focused()
            && !egui::Popup::is_any_open(ctx)
            && let Some((tool, anchor)) = ctx.input_mut(|input| {
                if input.pointer.secondary_down() || input.pointer.primary_down() {
                    return None;
                }
                Tool::KEYS
                    .into_iter()
                    .find(|(key, _)| input.consume_key(egui::Modifiers::NONE, *key))
                    .map(|(_, tool)| (tool, input.pointer.hover_pos().unwrap_or(rect.center())))
            })
        {
            self.transform_tool = TransformTool::new(
                &self.doc.scene,
                self.doc.selection.ids(),
                tool,
                anchor,
                self.viewport.transform_settings,
            );
            if self.transform_tool.is_none() {
                let message = if self.doc.selection.ids().is_empty() {
                    "Select an object to transform"
                } else {
                    "Transform is unavailable for this object"
                };
                self.show_transform_feedback(ctx, tool.key(), message.into());
            }
        }
        let Some(tool) = &mut self.transform_tool else {
            return false;
        };
        if let Some(apply) = tool.update(ctx, self.viewport.camera, rect) {
            if apply {
                for object in &mut self.doc.scene.entities {
                    if let Some(transform) = tool.preview.get(&object.uuid) {
                        object.transform = *transform;
                    }
                }
            }
            let (key, message) = tool.feedback(apply);
            self.transform_tool = None;
            self.show_transform_feedback(ctx, key, message);
        }
        true
    }

    fn show_transform_feedback(&mut self, ctx: &egui::Context, key: &'static str, message: String) {
        self.transform_feedback = Some((key, message, ctx.input(|input| input.time) + 1.25));
        ctx.request_repaint_after(std::time::Duration::from_millis(1250));
    }

    fn shortcut(&self, ctx: &egui::Context) -> Option<Action> {
        if self.play.is_some()
            || ctx.text_edit_focused()
            || egui::Popup::is_any_open(ctx)
            || ctx.input(|input| input.pointer.primary_down())
        {
            return None;
        }
        ctx.input_mut(|input| {
            [
                (Key::S, Action::Save),
                (Key::Z, Action::Undo),
                (Key::Y, Action::Redo),
                (Key::D, Action::Duplicate),
                (Key::A, Action::SelectAll),
            ]
            .into_iter()
            .find(|(key, _)| input.consume_key(egui::Modifiers::COMMAND, *key))
            .or_else(|| {
                if input.pointer.secondary_down() {
                    return None;
                }
                [(Key::Delete, Action::Delete), (Key::F, Action::Focus)]
                    .into_iter()
                    .find(|(key, _)| input.consume_key(egui::Modifiers::NONE, *key))
            })
            .map(|(_, action)| action)
        })
    }
}

impl eframe::App for Editor {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.input_settings.save(storage);
        eframe::set_value(storage, workspace::STORAGE_KEY, &self.dock);
        eframe::set_value(
            storage,
            "futty.transforms.v1",
            &self.viewport.transform_settings,
        );
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5)
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if !self.external_scripts.is_empty() {
            let now = std::time::Instant::now();
            if now >= self.next_script_sync || ctx.input(|input| input.viewport().close_requested())
            {
                self.sync_external_scripts();
                self.next_script_sync = now + std::time::Duration::from_millis(350);
            }
            ctx.request_repaint_after(self.next_script_sync.saturating_duration_since(now));
        }
        let gpu_error = self.gpu_error.lock().unwrap().take();
        if let Some(error) = gpu_error {
            self.report(format!("GPU error: {error}"), true);
        }
        if ctx.input(|input| input.viewport().close_requested())
            && (self.doc.dirty()
                || self
                    .script_editor
                    .as_ref()
                    .is_some_and(script_editor::ScriptEditor::dirty))
            && !self.allow_close
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.confirm_close_document() {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
        if ctx.current_pass_index() == 0 {
            input::InputSettings::toggle_shortcut(&ctx, &mut self.controls_open);
            if self.play.is_none()
                && let Some(preview) = &mut self.animation_preview
            {
                if self.doc.selection.primary() != Some(preview.object) {
                    self.animation_preview = None;
                } else if let Some(object) = self
                    .doc
                    .scene
                    .entities
                    .iter()
                    .find(|object| object.uuid == preview.object)
                    && let Some(mesh) = object.mesh
                    && preview.tick(
                        ctx.input(|input| input.stable_dt),
                        object,
                        self.viewport.clips(mesh),
                    )
                {
                    ctx.request_repaint();
                }
            }
            let mut audio_issue = self.audio.update();
            if self.audio.preview_active() {
                ctx.request_repaint_after(std::time::Duration::from_millis(150));
            }
            let is_range = self
                .play
                .as_ref()
                .is_some_and(|play| play.runtime().range_status().is_some());
            let game = self
                .game_rect
                .filter(|_| self.play.is_some() && !self.controls_open && !self.hud_open);
            let scene = match &self.play {
                Some(Play::Loaded(world)) => &world.scene,
                _ => &self.doc.scene,
            };
            let mode = input::MouseMode::for_scene(scene, is_range);
            let hud_blocks_mouse = self.range_controls.cursor_free()
                && game.is_some_and(|rect| hud_view::over_button(&ctx, rect, &scene.hud));
            let (dt, keys, second) = self.range_controls.input(
                &ctx,
                game,
                mode,
                &self.input_settings.with_profile(&scene.input),
                hud_blocks_mouse,
                scene.local_player_two().is_some(),
            );
            let player = scene
                .camera_index(None)
                .filter(|_| !is_range)
                .map(|index| scene.entities[index].uuid);
            if let Some(play) = &mut self.play {
                let (runtime, audio) = match play {
                    Play::Edited { runtime, .. } => (runtime.as_mut(), &mut self.audio),
                    Play::Loaded(world) => {
                        audio_issue = audio_issue.or(world.audio.update());
                        (&mut world.runtime, &mut world.audio)
                    }
                };
                if dt > 0.0 {
                    input::InputSettings::aim_character(&ctx, runtime, player, &keys, mode);
                    runtime.local_game_input(keys, second);
                    runtime.tick(dt);
                }
                audio_issue = audio_issue.or(audio::pump(audio, runtime));
                if let Some(id) = runtime.take_scene_request() {
                    self.transition(&ctx, &id);
                }
                ctx.request_repaint();
            }
            if let Some(error) = audio_issue {
                self.report(format!("Audio: {error}"), true);
            }
        }
        let before = self.doc.scene.clone();
        let modal = if ctx.current_pass_index() == 0 {
            self.update_transform_tool(&ctx)
        } else {
            self.transform_tool.is_some()
        };
        if !modal && self.play.is_none() && ctx.current_pass_index() == 0 {
            for path in ctx.input(|input| {
                input
                    .raw
                    .dropped_files
                    .iter()
                    .map(|file| file.path().to_path_buf())
                    .collect::<Vec<_>>()
            }) {
                self.import_asset(&path);
            }
        }
        let mut action = None;
        let frame = egui::Frame::new()
            .fill(PANEL)
            .inner_margin(egui::Margin::symmetric(8, 3));
        egui::Panel::top("toolbar")
            .exact_size(70.0)
            .resizable(false)
            .frame(frame)
            .show(ui, |ui| {
                ui.add_enabled_ui(!modal, |ui| self.toolbar(ui, &mut action));
            });
        egui::Panel::bottom("status")
            .exact_size(28.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(10, 2)),
            )
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(
                        RichText::new(if self.error {
                            "Error"
                        } else if self.play.is_some() {
                            "Play"
                        } else {
                            "Edit"
                        })
                        .strong()
                        .color(if self.error {
                            WARNING
                        } else if self.play.is_some() {
                            FOCUS
                        } else {
                            TEXT
                        }),
                    );
                    ui.separator();
                    ui.add(egui::Label::new(RichText::new(&self.message).color(MUTED)).truncate());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new(&self.gpu_label).color(MUTED));
                        ui.separator();
                        let dimension = match self.doc.scene.dimension {
                            SceneDimension::TwoD => "2D",
                            SceneDimension::ThreeD => "3D",
                        };
                        ui.label(
                            RichText::new(format!(
                                "{dimension} · {} objects",
                                self.doc.scene.entities.len()
                            ))
                            .color(MUTED),
                        );
                    });
                })
            });
        self.scene_rect = None;
        self.game_rect = None;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BACKGROUND))
            .show(ui, |ui| self.dock_workspace(ui, &mut action, modal));
        let controls_scene = match &self.play {
            Some(Play::Loaded(world)) => &world.scene,
            _ => &self.doc.scene,
        };
        let coop = self.input_settings.window(
            &ctx,
            &mut self.controls_open,
            controls_scene,
            self.play.is_none(),
        );
        if self.play.is_none() {
            self.doc.scene.local_coop = coop;
        }
        self.hud_window(&ctx);
        self.feel_window(&ctx, &mut action);
        self.doc.record_frame(
            before,
            ctx.egui_is_using_pointer() && ctx.input(|input| input.pointer.primary_down()),
        );
        if !modal && let Some(action) = action.or_else(|| self.shortcut(&ctx)) {
            self.execute(action);
            ctx.request_repaint();
        }
        if let Some(error) = self.doc.prefab_error.take() {
            self.report(format!("Prefab update failed: {error}"), true);
        }
    }
}

fn scene_dialog() -> FileDialog {
    FileDialog::new().add_filter("futty scene", &["ron"])
}

struct HierarchyDrag(Uuid);

fn hierarchy_drop(
    ui: &egui::Ui,
    response: &egui::Response,
    parent: Option<Uuid>,
    enabled: bool,
    action: &mut Option<Action>,
) {
    if !enabled {
        return;
    }
    if let Some(payload) = response.dnd_hover_payload::<HierarchyDrag>() {
        let color = if parent == Some(payload.0) {
            WARNING
        } else {
            FOCUS
        };
        ui.painter()
            .rect_stroke(response.rect, 2.0, (1.4, color), egui::StrokeKind::Inside);
    }
    if response.dnd_release_payload::<HierarchyDrag>().is_some() {
        *action = Some(Action::ReparentSelection(parent));
    }
}

fn script_console<'a>(ui: &mut egui::Ui, logs: impl DoubleEndedIterator<Item = &'a str>) {
    let mut logs = logs.rev().peekable();
    if logs.peek().is_none() {
        ui.label(RichText::new("No messages").color(MUTED));
    }
    egui::ScrollArea::vertical()
        .id_salt("script-log")
        .show(ui, |ui| {
            for line in logs {
                ui.label(line);
            }
        });
}

fn file_name(path: &std::path::Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Scene")
}

fn menu_item(
    ui: &mut egui::Ui,
    label: &str,
    command: Action,
    enabled: bool,
    action: &mut Option<Action>,
) {
    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
        *action = Some(command);
        ui.close();
    }
}

fn vector_fields(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Vec3,
    range: std::ops::RangeInclusive<f32>,
    speed: f64,
) -> bool {
    ui.label(RichText::new(label).color(MUTED));
    let mut changed = false;
    ui.horizontal(|ui| {
        let width = (ui.available_width() - 16.0) / 3.0;
        for (axis, (name, color)) in AXES.into_iter().enumerate() {
            let response = ui.add_sized(
                [width, 26.0],
                egui::DragValue::new(&mut value[axis])
                    .range(range.clone())
                    .speed(speed)
                    .fixed_decimals(2)
                    .prefix(format!("{name}  ")),
            );
            ui.painter().line_segment(
                [
                    response.rect.left_top() + egui::vec2(2.0, 5.0),
                    response.rect.left_bottom() + egui::vec2(2.0, -5.0),
                ],
                (1.5, color),
            );
            changed |= response.changed();
        }
    });
    changed
}

#[cfg(test)]
mod hierarchy_tests {
    use super::*;

    #[test]
    fn hierarchy_drop_emits_reparent_after_a_completed_pointer_drag() {
        let ctx = egui::Context::default();
        let (child, parent) = (Uuid::new_v4(), Uuid::new_v4());
        let mut centers = [egui::Pos2::ZERO; 2];
        let mut action = None;
        let mut frame = |events| {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 300.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let source = ui.add_sized(
                        [180.0, 26.0],
                        egui::Button::new("Child").sense(egui::Sense::click_and_drag()),
                    );
                    source.dnd_set_drag_payload(HierarchyDrag(child));
                    let destination = ui.add_sized(
                        [180.0, 26.0],
                        egui::Button::new("Parent").sense(egui::Sense::click_and_drag()),
                    );
                    hierarchy_drop(ui, &destination, Some(parent), true, &mut action);
                    centers = [source.rect.center(), destination.rect.center()];
                },
            );
            output.textures_delta.clear();
            centers
        };
        let [from, to] = frame(vec![]);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            pressed,
            button: egui::PointerButton::Primary,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![egui::Event::PointerMoved(from)]);
        frame(vec![button(from, true)]);
        frame(vec![egui::Event::PointerMoved(from.lerp(to, 0.5))]);
        frame(vec![egui::Event::PointerMoved(to)]);
        frame(vec![button(to, false)]);
        assert!(matches!(action, Some(Action::ReparentSelection(Some(id))) if id == parent));
    }
}
