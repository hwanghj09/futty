use eframe::egui;
use futty_runtime::{CharacterFeel, Scene};
use uuid::Uuid;

use crate::{Action, Editor};

#[derive(Default)]
pub(crate) struct FeelPanel {
    pub open: bool,
    draft: Option<Draft>,
}

#[derive(Clone, Copy)]
struct Draft {
    object_id: Uuid,
    original: CharacterFeel,
    values: CharacterFeel,
}

impl Draft {
    fn apply(self, scene: &mut Scene) -> Result<(), &'static str> {
        let controller = scene
            .entities
            .iter_mut()
            .find(|object| object.uuid == self.object_id)
            .and_then(|object| object.character.as_mut())
            .ok_or("The character controller was removed")?;
        if CharacterFeel::read(*controller) != self.original {
            return Err("Character values changed. Reopen the draft with the latest values.");
        }
        self.values.apply(controller)
    }
}

impl FeelPanel {
    pub fn clear(&mut self) {
        self.draft = None;
    }

    pub fn preview(&self, scene: &Scene) -> Result<Scene, String> {
        let draft = self.draft.ok_or("Choose a character to preview")?;
        let mut preview = scene.clone();
        draft.apply(&mut preview)?;
        preview
            .sync_prefabs(scene)
            .map_err(|error| error.to_string())?;
        Ok(preview)
    }
}

impl Editor {
    pub(crate) fn feel_window(&mut self, ctx: &egui::Context, action: &mut Option<Action>) {
        if !self.feel.open {
            return;
        }
        let scene = &self.doc.scene;
        let target = self
            .doc
            .selection
            .primary()
            .and_then(|id| scene.entities.iter().find(|object| object.uuid == id))
            .filter(|object| object.character.is_some())
            .or_else(|| scene.player_index().map(|index| &scene.entities[index]));
        let mut apply = None;
        let mut open = self.feel.open;
        egui::Window::new("Futty Feel")
            .open(&mut open)
            .default_width(345.0)
            .show(ctx, |ui| {
                let Some((object, controller)) = target
                    .and_then(|object| object.character.map(|controller| (object, controller)))
                else {
                    ui.label("Select an object with a character controller.");
                    return;
                };
                if self.play.is_some() {
                    ui.label("Stop Play to adjust or apply this draft.");
                    if ui.button("Stop Play").clicked() {
                        *action = Some(Action::PlayStop);
                    }
                    return;
                }
                let original = CharacterFeel::read(controller);
                if self.feel.draft.is_none_or(|draft| {
                    draft.object_id != object.uuid || draft.original != original
                }) {
                    self.feel.draft = Some(Draft {
                        object_id: object.uuid,
                        original,
                        values: original,
                    });
                }
                let draft = self.feel.draft.as_mut().unwrap();
                let can_jump = CharacterFeel::supports_jump(controller.mode, scene.dimension);
                ui.heading(&object.name);
                ui.horizontal(|ui| {
                    for (name, preset) in CharacterFeel::PRESETS {
                        if ui.button(name).clicked() {
                            draft.values.speed = preset.speed;
                            if can_jump {
                                draft.values.jump_speed = preset.jump_speed;
                            }
                        }
                    }
                });
                ui.separator();
                ui.add(
                    egui::Slider::new(&mut draft.values.speed, CharacterFeel::SPEED_RANGE)
                        .fixed_decimals(2)
                        .suffix(" u/s")
                        .text("Move speed"),
                );
                if can_jump {
                    ui.add(
                        egui::Slider::new(&mut draft.values.jump_speed, CharacterFeel::JUMP_RANGE)
                            .fixed_decimals(2)
                            .suffix(" u/s")
                            .text("Jump speed"),
                    );
                }
                let changed = draft.values != draft.original;
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(changed, egui::Button::new("Preview in Play"))
                        .clicked()
                    {
                        *action = Some(Action::PreviewFeel);
                    }
                    if ui
                        .add_enabled(changed, egui::Button::new("Apply"))
                        .clicked()
                    {
                        apply = Some(*draft);
                    }
                    if ui
                        .add_enabled(changed, egui::Button::new("Reset draft"))
                        .clicked()
                    {
                        draft.values = draft.original;
                    }
                });
            });
        self.feel.open = open;
        if !open {
            self.feel.clear();
        }
        if let Some(draft) = apply {
            match draft.apply(&mut self.doc.scene) {
                Ok(()) => self.report("Feel applied. Save Scene to write the changes.", false),
                Err(error) => self.report(error, true),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    #[test]
    fn feel_preview_is_isolated_and_applied_values_can_be_undone() {
        let mut scene = Scene::default();
        let object_id = scene.entities[0].uuid;
        let controller = futty_runtime::CharacterController::default();
        scene.entities[0].character = Some(controller);
        let original = CharacterFeel::read(controller);
        let values = CharacterFeel::PRESETS[2].1;
        let draft = Draft {
            object_id,
            original,
            values,
        };
        let mut feel = FeelPanel {
            open: true,
            draft: Some(draft),
        };
        let preview = feel.preview(&scene).unwrap();
        assert_eq!(
            CharacterFeel::read(preview.entities[0].character.unwrap()),
            values
        );
        assert_eq!(
            CharacterFeel::read(scene.entities[0].character.unwrap()),
            original
        );
        feel.clear();
        assert!(feel.preview(&scene).is_err());

        let mut doc = Document::new(scene.clone(), None);
        draft.apply(&mut doc.scene).unwrap();
        assert!(draft.apply(&mut doc.scene).is_err());
        doc.record_frame(scene.clone(), false);
        assert!(doc.scene == preview);
        doc.undo();
        assert!(doc.scene == scene);
        doc.redo();
        assert!(doc.scene == preview);
    }
}
