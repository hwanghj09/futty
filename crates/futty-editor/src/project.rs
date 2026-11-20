use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use eframe::egui;
pub(crate) use futty_runtime::project::{Project, SceneList, is_project};
use futty_runtime::{
    Scene,
    project::{MAX_SCENES, SUFFIX},
};

use crate::{Action, Document, Editor, menu_item, scene_dialog, workspace::Panel};

#[derive(Clone, Copy)]
pub(crate) enum ProjectAction {
    New,
    FromCurrentScene,
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

fn create_project_folder(folder: &Path) -> Result<(Project, Scene, PathBuf), Box<dyn Error>> {
    let root = folder.canonicalize()?;
    if fs::read_dir(&root)?.next().transpose()?.is_some() {
        return Err("Choose an empty folder for the new project".into());
    }
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Game");
    let manifest = root.join(format!("{name}{SUFFIX}"));
    let path = root.join("Main.futty.scene.ron");
    let scene = Scene::default();
    scene.save_new(&path)?;
    match Project::create(&manifest, &path) {
        Ok(project) => Ok((project, scene, path)),
        Err(error) => {
            fs::remove_file(&path).map_err(|cleanup| {
                format!("{error}; could not remove the initial scene: {cleanup}")
            })?;
            Err(error)
        }
    }
}

impl Editor {
    pub(crate) fn project_menu(&self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.menu_button("Project", |ui| {
            let mut item = |ui: &mut egui::Ui, label, choice, enabled| {
                menu_item(ui, label, Action::Project(choice), enabled, action);
            };
            item(ui, "New Project…", ProjectAction::New, true);
            item(
                ui,
                "New Project from Current Scene…",
                ProjectAction::FromCurrentScene,
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
                let Some(folder) = rfd::FileDialog::new()
                    .set_title("Choose an empty folder for the new Futty project")
                    .pick_folder()
                else {
                    return Ok(());
                };
                if !self.confirm_close_document() {
                    return Ok(());
                }
                let (project, scene, path) = create_project_folder(&folder)?;
                self.clear_assets();
                self.doc = Document::new(scene, Some(path));
                self.viewport.reset_camera();
                self.show_panel(Panel::Scene);
                self.report(format!("Created project {}", project.name()), false);
                self.project = Some(project);
            }
            ProjectAction::FromCurrentScene => {
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
                let root = project.root.clone();
                let scenes = project.list_mut();
                let message = match change {
                    ProjectAction::Add => {
                        let Some(path) = scene_dialog().set_directory(&root).pick_file() else {
                            return Ok(());
                        };
                        let id = scenes.add(&root, &path)?;
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
                            project.set_start(&id)?;
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
    fn new_project_opens_its_own_saved_scene() -> Result<(), Box<dyn Error>> {
        let directory = tempfile::tempdir()?;
        let (project, mut scene, path) = create_project_folder(directory.path())?;
        let reopened = Project::load(project.path())?;
        assert_eq!(reopened.list().start, "Main");
        assert_eq!(reopened.resolve("Main")?, path);
        assert_eq!(Scene::load(&path)?.scene_uuid, scene.scene_uuid);
        assert!(create_project_folder(directory.path()).is_err());
        assert_eq!(Scene::load(&path)?.scene_uuid, scene.scene_uuid);
        let script = futty_core::Asset {
            id: uuid::Uuid::new_v4(),
            name: "Move".into(),
            kind: futty_core::AssetKind::Script,
        };
        let source = include_bytes!("../../../examples/script-playground/Move.rhai");
        futty_assets::save(&path, &script, source)?;
        scene.assets.push(script.clone());
        scene.save(&path)?;
        assert_eq!(
            futty_assets::read_saved(&reopened.resolve("Main")?, &script)?,
            source
        );
        assert!(project.root.join("Assets").join("Futty").is_dir());
        Ok(())
    }

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
        std::fs::create_dir(second.parent().unwrap())?;
        Scene::default().save(&second)?;
        let id = project.list_mut().add(&root, &second)?;
        project.list_mut().start = id;
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
                .list_mut()
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
