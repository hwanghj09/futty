use eframe::egui;
use futty_core::{LightKind, Primitive};

use crate::transform_tool::{AxisSpace, PivotMode};
use crate::{Action, Editor, menu_item, workspace};

pub(crate) fn add_objects(ui: &mut egui::Ui, action: &mut Option<Action>) {
    for shape in Primitive::ALL {
        menu_item(ui, shape.label(), Action::Add(shape), true, action);
    }
    ui.separator();
    menu_item(ui, "Camera", Action::AddCamera, true, action);
    ui.menu_button("Light", |ui| {
        for kind in [LightKind::Directional, LightKind::Point] {
            menu_item(ui, kind.label(), Action::AddLight(kind), true, action);
        }
    });
}

impl Editor {
    pub(crate) fn selection_menu(&self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let Some(id) = self.doc.selection.primary() else {
            return;
        };
        menu_item(ui, "Focus Selection   F", Action::Focus, true, action);
        menu_item(ui, "Duplicate   Ctrl+D", Action::Duplicate, true, action);
        menu_item(
            ui,
            "Create Prefab",
            Action::CreatePrefab,
            self.doc.selection.ids().len() == 1,
            action,
        );
        menu_item(ui, "Delete Selection", Action::Delete, true, action);
        if self.doc.scene.entities.iter().any(|object| {
            self.doc.selection.ids().contains(&object.uuid) && object.parent.is_some()
        }) {
            menu_item(
                ui,
                "Move to Scene Root",
                Action::ReparentSelection(None),
                true,
                action,
            );
        }
        let scene = &self.doc.scene;
        if scene
            .entities
            .iter()
            .any(|object| object.uuid == id && object.camera.is_some())
        {
            ui.separator();
            menu_item(
                ui,
                "Preview in Game",
                Action::PreviewCamera(id),
                true,
                action,
            );
            menu_item(
                ui,
                "Align to Scene View",
                Action::AlignCamera(id),
                true,
                action,
            );
            let main = scene
                .camera_index(None)
                .is_some_and(|index| scene.entities[index].uuid == id);
            if !main {
                menu_item(
                    ui,
                    "Make Main Camera",
                    Action::SetMainCamera(id),
                    true,
                    action,
                );
            }
        }
        ui.separator();
    }

    pub(crate) fn hierarchy_menu(&self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        if self.play.is_none() {
            self.selection_menu(ui, action);
            ui.menu_button("Add Object", |ui| add_objects(ui, action));
            menu_item(ui, "Select All   Ctrl+A", Action::SelectAll, true, action);
            ui.separator();
        }
        workspace::panels_submenu(ui, action);
    }

    pub(crate) fn scene_menu(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        if self.play.is_none() {
            self.selection_menu(ui, action);
            ui.menu_button("Add Object", |ui| add_objects(ui, action));
            ui.separator();
            ui.menu_button("Tool", |ui| {
                ui.selectable_value(&mut self.move_tool, false, "Select");
                ui.selectable_value(&mut self.move_tool, true, "Move");
            });
            ui.menu_button("Transform Settings", |ui| {
                let settings = &mut self.viewport.transform_settings;
                ui.label("Move / Rotate Axes");
                ui.selectable_value(&mut settings.space, AxisSpace::World, "World");
                ui.selectable_value(
                    &mut settings.space,
                    AxisSpace::Local,
                    "Local (Active Object)",
                );
                ui.separator();
                ui.label("Pivot");
                ui.selectable_value(&mut settings.pivot, PivotMode::Center, "Selection Center");
                ui.selectable_value(&mut settings.pivot, PivotMode::Active, "Active Object");
                ui.selectable_value(
                    &mut settings.pivot,
                    PivotMode::Individual,
                    "Individual Origins",
                );
                ui.separator();
                ui.checkbox(&mut settings.snap, "Snap");
                for (label, value, range) in [
                    ("Move ", &mut settings.move_step, 0.001..=100.0),
                    ("Rotate ", &mut settings.rotate_step, 0.1..=180.0),
                    ("Scale ", &mut settings.scale_step, 0.001..=10.0),
                ] {
                    ui.add(
                        egui::DragValue::new(value)
                            .prefix(label)
                            .range(range)
                            .speed(0.05),
                    );
                }
            });
        }
        ui.checkbox(&mut self.viewport.grid, "Show Grid");
        ui.checkbox(&mut self.viewport.colliders, "Show Selected Colliders");
        ui.separator();
        workspace::panels_submenu(ui, action);
    }
}
