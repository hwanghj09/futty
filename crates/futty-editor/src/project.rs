use std::{error::Error, path::Path};

use eframe::egui;
use futty_runtime::project::{MAX_SCENES, SUFFIX};
pub(crate) use futty_runtime::project::{Project, SceneList, is_project};

use crate::{Action, Editor, menu_item, scene_dialog};

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
    use futty_runtime::Scene;

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
