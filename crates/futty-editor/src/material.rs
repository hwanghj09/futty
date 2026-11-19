use std::collections::BTreeMap;

use eframe::egui::{self, RichText};
use futty_core::AssetKind;
use futty_runtime::{Material, Object};
use uuid::Uuid;

use crate::{Action, Editor, InspectedAsset, MUTED, menu_item};

pub(crate) fn assign(object: &mut Object, material: Option<Uuid>) {
    if material != object.material {
        object.material = material;
        if material.is_some() {
            object.color = [1.0; 3];
        }
    }
}

pub(crate) fn selector(
    ui: &mut egui::Ui,
    object: &mut Object,
    materials: &BTreeMap<Uuid, Material>,
    action: &mut Option<Action>,
) {
    ui.label("Material");
    let mut selected = object.material;
    let name = selected
        .and_then(|id| materials.get(&id))
        .map_or("Per-object material", |material| material.name.as_str());
    egui::ComboBox::from_id_salt("shared-material")
        .width(ui.available_width())
        .selected_text(name)
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut selected, None, "Per-object material");
            for (id, material) in materials {
                ui.selectable_value(&mut selected, Some(*id), &material.name);
            }
        });
    assign(object, selected);
    if let Some(id) = selected
        && ui.button("Edit Shared Material").clicked()
    {
        *action = Some(Action::EditMaterial(id));
    }
}

impl Editor {
    pub(crate) fn material_rows(&self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let edit = self.play.is_none();
        let has_mesh = self.doc.scene.entities.iter().any(|object| {
            object.has_renderer()
                && object.sprite.is_none()
                && self.doc.selection.ids().contains(&object.uuid)
        });
        for (id, material) in &self.doc.scene.materials {
            ui.push_id(id, |ui| {
                let row = ui.add_sized(
                    [ui.available_width(), 26.0],
                    egui::Button::selectable(
                        self.inspected_asset == Some(InspectedAsset::Material(*id)),
                        format!("MAT   {}", material.name),
                    ),
                );
                if row.clicked() {
                    *action = Some(Action::EditMaterial(*id));
                }
                row.context_menu(|ui| {
                    menu_item(ui, "Edit Material", Action::EditMaterial(*id), true, action);
                    menu_item(
                        ui,
                        "Apply to Selection",
                        Action::ApplyMaterial(*id),
                        edit && has_mesh,
                        action,
                    );
                    ui.separator();
                    let unused = self
                        .doc
                        .scene
                        .referenced_objects()
                        .all(|object| object.material != Some(*id));
                    menu_item(
                        ui,
                        "Remove Material",
                        Action::RemoveMaterial(*id),
                        edit && unused,
                        action,
                    );
                });
            });
        }
    }

    pub(crate) fn material_inspector(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(InspectedAsset::Material(id)) = self.inspected_asset else {
            return false;
        };
        let users = self
            .doc
            .scene
            .entities
            .iter()
            .filter(|object| object.material == Some(id))
            .count();
        let prefabs = self
            .doc
            .scene
            .prefabs
            .values()
            .filter(|prefab| {
                prefab
                    .entities
                    .iter()
                    .any(|object| object.material == Some(id))
            })
            .count();
        let Some(material) = self.doc.scene.materials.get_mut(&id) else {
            self.inspected_asset = None;
            return false;
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new("Material").strong());
            if ui.button("Back to Object").clicked() {
                self.inspected_asset = None;
            }
        });
        ui.label(
            RichText::new(format!("Shared by {users} object(s) · {prefabs} prefab(s)"))
                .color(MUTED),
        );
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_enabled_ui(self.play.is_none(), |ui| {
                ui.label("Name");
                ui.add(egui::TextEdit::singleline(&mut material.name).char_limit(64).desired_width(f32::INFINITY));
                if material.name.trim().is_empty() {
                    ui.colored_label(crate::WARNING, "Enter a name before saving.");
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.color_edit_button_rgb(&mut material.color);
                    ui.label("Base color");
                });
                ui.label("Base texture");
                let texture_name = material.texture.and_then(|id| self.doc.scene.assets.iter().find(|asset| asset.id == id))
                    .map_or("None", |asset| asset.name.as_str());
                egui::ComboBox::from_id_salt("material-texture")
                    .width(ui.available_width()).selected_text(texture_name)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut material.texture, None, "None");
                        for asset in self.doc.scene.assets.iter().filter(|asset| asset.kind == AssetKind::Texture) {
                            ui.add_enabled_ui(self.asset_bytes.contains_key(&asset.id), |ui| {
                                ui.selectable_value(&mut material.texture, Some(asset.id), &asset.name);
                            });
                        }
                    });
                ui.add_space(10.0);
                ui.add(egui::Slider::new(&mut material.surface.roughness, 0.0..=1.0).text("Roughness"));
                ui.horizontal(|ui| {
                    ui.color_edit_button_rgb(&mut material.surface.emission);
                    ui.label("Emission color");
                });
                ui.add(egui::Slider::new(&mut material.surface.emission_strength, 0.0..=10.0).text("Emission"));
                ui.add_space(10.0);
                ui.label(RichText::new("Changes update every object using this material. Apply from the Project context menu or an object's Material field.").color(MUTED));
            });
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assigning_a_new_material_resets_tint_only_once() {
        let mut object = Object::cube("Cube".into(), glam::Vec3::ZERO);
        let id = Uuid::new_v4();
        assign(&mut object, Some(id));
        assert_eq!(object.color, [1.0; 3]);
        object.color = [0.5; 3];
        assign(&mut object, Some(id));
        assert_eq!(object.color, [0.5; 3]);
        assign(&mut object, None);
        assert_eq!(object.color, [0.5; 3]);
    }
}
