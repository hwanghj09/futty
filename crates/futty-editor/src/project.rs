use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use eframe::egui;
use futty_runtime::{Scene, valid_scene_id};
use serde::{Deserialize, Serialize};

use crate::{Action, Editor, file_name, menu_item, scene_dialog};

const SUFFIX: &str = ".futty.project.ron";
const MAX_SCENES: usize = 64;

pub(crate) fn is_project(path: &Path) -> bool {
    file_name(path).ends_with(SUFFIX)
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SceneList {
    pub start: String,
    pub scenes: BTreeMap<String, String>,
}

impl SceneList {
    pub fn validate(&self) -> Result<(), Box<dyn Error>> {
        let mut paths = HashSet::new();
        if self.scenes.len() > MAX_SCENES
            || !self.scenes.contains_key(&self.start)
            || self.scenes.iter().any(|(id, path)| {
                !valid_scene_id(id)
                    || !path.ends_with(".ron")
                    || path.contains(['\\', ':'])
                    || path
                        .split('/')
                        .any(|part| part.is_empty() || part == "." || part == "..")
                    || !paths.insert(path)
            })
        {
            return Err("Invalid scene list. Use unique IDs and relative .ron paths, and choose a start scene.".into());
        }
        Ok(())
    }

    pub fn resolve(&self, root: &Path, id: &str) -> Result<PathBuf, Box<dyn Error>> {
        let relative = self
            .scenes
            .get(id)
            .ok_or_else(|| format!("Unknown scene ID {id:?}"))?;
        let path = root
            .join(relative)
            .canonicalize()
            .map_err(|error| format!("{id}: {error}"))?;
        if !path.starts_with(root) || !path.is_file() {
            return Err(format!("Scene {id:?} must be a file inside the project folder").into());
        }
        Ok(path)
    }

    fn add(&mut self, root: &Path, path: &Path) -> Result<String, Box<dyn Error>> {
        let path = path.canonicalize()?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "Choose a scene inside the project folder")?;
        let relative = relative
            .to_str()
            .ok_or("Scene paths must be UTF-8")?
            .replace('\\', "/");
        let filename = file_name(&path);
        let id = filename
            .strip_suffix(".futty.scene.ron")
            .or_else(|| filename.strip_suffix(".ron"))
            .ok_or("Choose a scene .ron file")?
            .to_owned();
        if self.scenes.contains_key(&id) {
            return Err(format!("Scene ID {id:?} is already registered").into());
        }
        Scene::load(&path)?;
        self.scenes.insert(id.clone(), relative);
        if self.start.is_empty() {
            self.start.clone_from(&id);
        }
        self.validate()?;
        Ok(id)
    }
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: u32,
    scenes: SceneList,
}

#[derive(Clone)]
pub(crate) struct Project {
    path: PathBuf,
    pub root: PathBuf,
    manifest: Manifest,
}

impl Project {
    pub fn load(path: &Path) -> Result<Self, Box<dyn Error>> {
        let path = path.canonicalize()?;
        let root = path.parent().ok_or("Missing project folder")?.to_path_buf();
        let manifest: Manifest = ron::from_str(&fs::read_to_string(&path)?)?;
        if manifest.format != 1 {
            return Err("Unsupported Futty project format".into());
        }
        manifest.scenes.validate()?;
        Ok(Self {
            path,
            root,
            manifest,
        })
    }

    fn create(path: &Path, first: &Path) -> Result<Self, Box<dyn Error>> {
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let root = parent.canonicalize()?;
        let path = root.join(path.file_name().ok_or("Missing project filename")?);
        let mut scenes = SceneList {
            start: String::new(),
            scenes: BTreeMap::new(),
        };
        scenes.add(&root, first)?;
        let project = Self {
            path,
            root,
            manifest: Manifest { format: 1, scenes },
        };
        project.save(false)?;
        Ok(project)
    }

    fn save(&self, replace: bool) -> Result<(), Box<dyn Error>> {
        self.list().validate()?;
        let mut file = tempfile::NamedTempFile::new_in(&self.root)?;
        file.write_all(ron::ser::to_string_pretty(&self.manifest, Default::default())?.as_bytes())?;
        file.as_file().sync_all()?;
        if replace {
            file.persist(&self.path)?;
        } else {
            file.persist_noclobber(&self.path)?;
        }
        Ok(())
    }

    pub fn name(&self) -> &str {
        file_name(&self.path)
            .strip_suffix(SUFFIX)
            .unwrap_or("Project")
    }

    pub fn list(&self) -> &SceneList {
        &self.manifest.scenes
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn resolve(&self, id: &str) -> Result<PathBuf, Box<dyn Error>> {
        self.list().resolve(&self.root, id)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ProjectAction {
    New,
    Open,
    Add,
    Edit(usize),
    Start(usize),
    Remove(usize),
    Close,
}

fn project_dialog() -> rfd::FileDialog {
    rfd::FileDialog::new().add_filter("Futty project", &["futty.project.ron", "ron"])
}

impl Editor {
    pub(crate) fn project_menu(&self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.menu_button("Project", |ui| {
            let mut item = |ui: &mut egui::Ui, label, choice, enabled| {
                menu_item(ui, label, Action::Project(choice), enabled, action);
            };
            item(
                ui,
                "New Project from Current Scene…",
                ProjectAction::New,
                true,
            );
            item(ui, "Open Project…", ProjectAction::Open, true);
            if let Some(project) = &self.project {
                ui.separator();
                ui.label(format!(
                    "{} · Starts in {}",
                    project.name(),
                    project.list().start
                ));
                item(
                    ui,
                    "Add Scene…",
                    ProjectAction::Add,
                    project.list().scenes.len() < MAX_SCENES,
                );
                ui.menu_button("Edit Scene", |ui| {
                    for (index, id) in project.list().scenes.keys().enumerate() {
                        item(ui, id, ProjectAction::Edit(index), true);
                    }
                });
                ui.menu_button("Start Scene", |ui| {
                    for (index, id) in project.list().scenes.keys().enumerate() {
                        item(
                            ui,
                            id,
                            ProjectAction::Start(index),
                            id != &project.list().start,
                        );
                    }
                });
                ui.menu_button("Remove Scene", |ui| {
                    for (index, id) in project.list().scenes.keys().enumerate() {
                        item(
                            ui,
                            id,
                            ProjectAction::Remove(index),
                            id != &project.list().start,
                        );
                    }
                });
                item(ui, "Close Project", ProjectAction::Close, true);
            }
        });
    }

    pub(crate) fn open_project(&mut self, path: &Path) {
        let result = Project::load(path).and_then(|project| {
            let start = project.resolve(&project.list().start)?;
            Ok((project, start))
        });
        match result {
            Ok((project, start)) => {
                if self.open_scene(start) {
                    self.project = Some(project);
                }
            }
            Err(error) => self.report(format!("Open project failed: {error}"), true),
        }
    }

    pub(crate) fn project_action(&mut self, action: ProjectAction) {
        let result = self.apply_project_action(action);
        if let Err(error) = result {
            self.report(format!("Project: {error}"), true);
        }
    }

    fn apply_project_action(&mut self, action: ProjectAction) -> Result<(), Box<dyn Error>> {
        match action {
            ProjectAction::New => {
                self.sync_external_scripts();
                if !self.apply_script_draft() {
                    return Ok(());
                }
                if (self.doc.dirty() || self.doc.path.is_none()) && !self.save(false) {
                    return Ok(());
                }
                let Some(scene) = &self.doc.path else {
                    return Ok(());
                };
                let Some(path) = project_dialog()
                    .set_directory(scene.parent().unwrap_or(Path::new(".")))
                    .set_file_name(format!("Game{SUFFIX}"))
                    .save_file()
                else {
                    return Ok(());
                };
                let path = if is_project(&path) {
                    path
                } else {
                    path.with_extension(&SUFFIX[1..])
                };
                let project = Project::create(&path, scene)?;
                self.report(format!("Created project {}", project.name()), false);
                self.project = Some(project);
            }
            ProjectAction::Open => {
                if let Some(path) = project_dialog().pick_file() {
                    self.open_project(&path);
                }
            }
            ProjectAction::Close => {
                self.project = None;
                self.report("Project closed", false);
            }
            change => {
                let Some(mut project) = self.project.clone() else {
                    return Ok(());
                };
                let scenes = &mut project.manifest.scenes;
                let message = match change {
                    ProjectAction::Add => {
                        let Some(path) = scene_dialog().set_directory(&project.root).pick_file()
                        else {
                            return Ok(());
                        };
                        let id = scenes.add(&project.root, &path)?;
                        format!("Added scene {id}")
                    }
                    ProjectAction::Edit(index)
                    | ProjectAction::Start(index)
                    | ProjectAction::Remove(index) => {
                        let Some(id) = scenes.scenes.keys().nth(index).cloned() else {
                            return Ok(());
                        };
                        if matches!(change, ProjectAction::Edit(_)) {
                            self.open_scene(project.resolve(&id)?);
                            return Ok(());
                        }
                        if matches!(change, ProjectAction::Start(_)) {
                            Scene::load(&scenes.resolve(&project.root, &id)?)?;
                            scenes.start.clone_from(&id);
                            format!("Start scene set to {id}")
                        } else {
                            if id == scenes.start {
                                return Ok(());
                            }
                            scenes.scenes.remove(&id);
                            format!("Removed {id} from the project")
                        }
                    }
                    _ => unreachable!(),
                };
                project.save(true)?;
                self.project = Some(project);
                self.report(message, false);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_persists_start_and_rejects_unlisted_or_escaping_scenes() -> Result<(), Box<dyn Error>>
    {
        let directory = tempfile::tempdir()?;
        let root = directory.path().canonicalize()?;
        let path = root.join("Game.futty.project.ron");
        let first = root.join("Main.futty.scene.ron");
        Scene::default().save(&first)?;
        let mut project = Project::create(&path, &first)?;
        assert!(Project::create(&path, &first).is_err());
        let second = root.join("Sub").join("Level.futty.scene.ron");
        fs::create_dir(second.parent().unwrap())?;
        Scene::default().save(&second)?;
        let id = project.manifest.scenes.add(&project.root, &second)?;
        project.manifest.scenes.start = id;
        project.save(true)?;
        let mut project = Project::load(&path)?;
        assert_eq!(project.list().start, "Level");
        assert_eq!(project.resolve("Level")?, second.canonicalize()?);
        assert!(project.resolve("Unknown").is_err());

        for invalid in [
            "../outside.ron",
            "/main.ron",
            "C:/main.ron",
            "Sub\\Level.ron",
        ] {
            project
                .manifest
                .scenes
                .scenes
                .insert("Level".into(), invalid.into());
            assert!(project.save(true).is_err());
        }
        assert_eq!(
            Project::load(&path)?.resolve("Level")?,
            second.canonicalize()?
        );
        Ok(())
    }
}
