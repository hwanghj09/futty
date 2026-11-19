use eframe::egui::{self, RichText};
use futty_runtime::{Hierarchy, Object};
use glam::Vec3;
use uuid::Uuid;

use crate::{Action, Editor, InspectedAsset, MUTED, menu_item};

impl Editor {
    pub(crate) fn prefab_rows(&self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let edit = self.play.is_none();
        for (id, prefab) in &self.doc.scene.prefabs {
            ui.push_id(id, |ui| {
                let row = ui.add_sized(
                    [ui.available_width(), 26.0],
                    egui::Button::selectable(
                        self.inspected_asset == Some(InspectedAsset::Prefab(*id)),
                        format!(
                            "PREFAB   {}   ·   {} objects",
                            prefab.name,
                            prefab.entities.len()
                        ),
                    ),
                );
                if row.double_clicked() && edit {
                    *action = Some(Action::PlacePrefab(*id));
                } else if row.clicked() {
                    *action = Some(Action::InspectPrefab(*id));
                }
                row.context_menu(|ui| {
                    menu_item(ui, "Place in Scene", Action::PlacePrefab(*id), edit, action);
                    menu_item(
                        ui,
                        "Edit Source",
                        Action::EditPrefabSource(*id),
                        edit && self.doc.scene.prefab_source(*id).is_some(),
                        action,
                    );
                    menu_item(
                        ui,
                        "Inspect / Rename",
                        Action::InspectPrefab(*id),
                        true,
                        action,
                    );
                    ui.separator();
                    menu_item(ui, "Remove Prefab", Action::RemovePrefab(*id), edit, action);
                });
            });
        }
    }

    pub(crate) fn prefab_inspector(
        &mut self,
        ui: &mut egui::Ui,
        action: &mut Option<Action>,
    ) -> bool {
        let Some(InspectedAsset::Prefab(id)) = self.inspected_asset else {
            return false;
        };
        let source_exists = self.doc.scene.prefab_source(id).is_some();
        let instances = self
            .doc
            .scene
            .entities
            .iter()
            .filter(|object| {
                object
                    .prefab
                    .is_some_and(|link| link.asset == id && link.instance == object.uuid)
            })
            .count();
        let Some(prefab) = self.doc.scene.prefabs.get_mut(&id) else {
            self.inspected_asset = None;
            return false;
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new("Prefab").strong());
            if ui.button("Back to Object").clicked() {
                self.inspected_asset = None;
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_enabled_ui(self.play.is_none(), |ui| {
                ui.label("Name");
                ui.add(
                    egui::TextEdit::singleline(&mut prefab.name)
                        .char_limit(64)
                        .desired_width(f32::INFINITY),
                );
                let valid_name = !prefab.name.trim().is_empty();
                if !valid_name {
                    ui.colored_label(crate::WARNING, "Enter a name before saving or placing.");
                }
                ui.add_space(8.0);
                if ui
                    .add_enabled(valid_name, egui::Button::new("Place in Scene"))
                    .clicked()
                {
                    *action = Some(Action::PlacePrefab(id));
                }
                if ui
                    .add_enabled(source_exists, egui::Button::new("Edit Source"))
                    .clicked()
                {
                    *action = Some(Action::EditPrefabSource(id));
                }
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "{} objects · {instances} linked instances",
                    prefab.entities.len()
                ))
                .color(MUTED),
            );
            ui.separator();
            if let Ok(hierarchy) = prefab.hierarchy() {
                for (index, depth) in hierarchy.order {
                    ui.horizontal(|ui| {
                        ui.add_space((depth as f32 * 12.0).min(ui.available_width() * 0.4));
                        ui.add(egui::Label::new(&prefab.entities[index].name).truncate());
                    });
                }
            }
            ui.add_space(12.0);
            if !source_exists {
                ui.label(
                    RichText::new("Source is unavailable. Placements use the saved prefab.")
                        .color(MUTED),
                );
            }
            if prefab
                .entities
                .iter()
                .any(|object| object.audio_listener.is_some())
            {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("An existing scene listener is kept when placing this prefab.")
                        .color(MUTED),
                );
            }
        });
        true
    }

    pub(crate) fn place_prefab(&mut self, id: Uuid) {
        let result = (|| {
            let prefab = self
                .doc
                .scene
                .prefabs
                .get(&id)
                .ok_or("Prefab is no longer in the scene.")?;
            let bounds = |objects: &[Object], hierarchy: Hierarchy| {
                let worlds = hierarchy.world_matrices(|i| objects[i].transform.matrix());
                let mut low = Vec3::splat(f32::INFINITY);
                let mut high = Vec3::splat(f32::NEG_INFINITY);
                for (object, world) in objects
                    .iter()
                    .zip(worlds)
                    .filter(|(object, _)| object.has_renderer())
                {
                    let [min, max] = self.viewport.object_bounds(object);
                    let matrix = object.sprite.map_or(world, |sprite| {
                        sprite.matrix(world, self.viewport.camera.view())
                    });
                    let center = matrix.transform_point3((min + max) * 0.5);
                    let half = (max - min) * 0.5;
                    let extent = matrix.x_axis.truncate().abs() * half.x
                        + matrix.y_axis.truncate().abs() * half.y
                        + matrix.z_axis.truncate().abs() * half.z;
                    low = low.min(center - extent);
                    high = high.max(center + extent);
                }
                (low.is_finite() && high.is_finite()).then_some([low, high])
            };
            let [min, max] = bounds(
                &prefab.entities,
                prefab.hierarchy().map_err(|error| error.to_string())?,
            )
            .unwrap_or([Vec3::splat(-0.5), Vec3::splat(0.5)]);
            let existing = bounds(
                &self.doc.scene.entities,
                self.doc
                    .scene
                    .hierarchy()
                    .map_err(|error| error.to_string())?,
            );
            let x = existing.map_or(-(min.x + max.x) * 0.5, |[_, high]| high.x + 0.6 - min.x);
            let position = Vec3::new(x, -min.y, -(min.z + max.z) * 0.5);
            self.doc.instantiate_prefab(id, position)
        })();
        match result {
            Ok(_) => {
                self.inspected_asset = None;
                self.script_inspector = false;
                self.hierarchy_search.clear();
                self.show_panel(crate::workspace::Panel::Scene);
                self.focus_selection();
                self.report("Prefab placed · G to move, R to rotate, S to scale", false);
            }
            Err(error) => self.report(error, true),
        }
    }
}
