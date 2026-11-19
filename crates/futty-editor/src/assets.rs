use std::path::Path;

use eframe::egui;
use egui::RichText;
use futty_core::{Asset, AssetKind};
use futty_runtime::{Object, Scene};
use glam::Vec3;
use rfd::FileDialog;
use uuid::Uuid;

use crate::{Action, Editor, MUTED, WARNING, menu_item, workspace};

impl Editor {
    pub(crate) fn import_dialog(&mut self) {
        if let Some(path) = FileDialog::new()
            .add_filter("Futty assets", &["glb", "png", "rhai", "wav", "ogg"])
            .pick_file()
        {
            self.import_asset(&path);
        }
    }

    pub(crate) fn import_asset(&mut self, path: &Path) {
        let result = (|| {
            let kind =
                futty_assets::kind(path).ok_or("Choose a GLB, PNG, Rhai, WAV or OGG file")?;
            let name = path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("Asset")
                .to_owned();
            let asset = Asset {
                id: Uuid::new_v4(),
                name,
                kind,
            };
            let bytes = futty_assets::read(path)?;
            self.prepare_asset(&asset, &bytes)?;
            Ok::<_, Box<dyn std::error::Error>>((asset, bytes))
        })();
        match result {
            Ok((asset, bytes)) => {
                let (id, kind) = (asset.id, asset.kind);
                self.report(format!("Imported {}", asset.name), false);
                self.asset_bytes.insert(id, bytes);
                self.doc.scene.assets.push(asset);
                if kind == AssetKind::Model {
                    self.place_model(id);
                    self.focus_selection();
                }
            }
            Err(error) => self.report(format!("Import failed: {error}"), true),
        }
    }

    pub(crate) fn install_assets(&mut self, scene: &Scene, path: &Path) -> Vec<String> {
        self.clear_assets();
        let mut errors = Vec::new();
        for asset in &scene.assets {
            if scene.script_sources.contains_key(&asset.id) {
                continue;
            }
            match futty_assets::read_saved(path, asset).and_then(|bytes| {
                self.prepare_asset(asset, &bytes)?;
                Ok(bytes)
            }) {
                Ok(bytes) => {
                    self.asset_bytes.insert(asset.id, bytes);
                }
                Err(error) => errors.push(format!("{}: {error}", asset.name)),
            }
        }
        errors
    }

    pub(crate) fn clear_assets(&mut self) {
        self.inspected_asset = None;
        self.hierarchy_search.clear();
        self.camera_preview = None;
        self.diagnostics = workspace::RunDiagnostics::default();
        self.script_editor = None;
        self.external_scripts.clear();
        self.script_schemas.clear();
        self.audio.clear();
        self.asset_bytes.clear();
        self.viewport.clear_assets();
    }

    fn prepare_asset(&mut self, asset: &Asset, bytes: &[u8]) -> futty_assets::AssetResult<()> {
        match asset.kind {
            AssetKind::Wav | AssetKind::Ogg => {
                self.audio.load(asset.id, asset.kind.extension(), bytes)
            }
            AssetKind::Script => futty_assets::script_source(bytes).map(|_| ()),
            AssetKind::Model | AssetKind::Texture => self.viewport.upload(asset, bytes),
        }
    }

    pub(crate) fn save_assets(&self, path: &Path) -> futty_assets::AssetResult<()> {
        for asset in &self.doc.scene.assets {
            if self.doc.scene.script_sources.contains_key(&asset.id) {
                continue;
            }
            let bytes = self
                .asset_bytes
                .get(&asset.id)
                .ok_or_else(|| format!("Missing asset {}", asset.name))?;
            futty_assets::save(path, asset, bytes)?;
        }
        Ok(())
    }

    pub(crate) fn refresh_scripts(&mut self) {
        let Some(path) = &self.doc.path else { return };
        for asset in self.doc.scene.assets.iter().filter(|asset| {
            asset.kind == AssetKind::Script
                && !self.doc.scene.script_sources.contains_key(&asset.id)
        }) {
            if let Ok(bytes) = futty_assets::read_saved(path, asset) {
                self.asset_bytes.insert(asset.id, bytes);
            }
        }
    }

    pub(crate) fn script_sources(&self) -> std::collections::HashMap<Uuid, String> {
        self.doc
            .scene
            .assets
            .iter()
            .filter(|asset| asset.kind == AssetKind::Script)
            .filter_map(|asset| {
                let source = futty_assets::script_source(self.asset_bytes.get(&asset.id)?).ok()?;
                Some((asset.id, source.to_owned()))
            })
            .collect()
    }

    pub(crate) fn load_script_playground(&mut self) {
        let mut scene = Scene::default();
        scene.name = "Script Playground".into();
        let id = Uuid::new_v4();
        scene.assets.push(Asset {
            id,
            name: "Move".into(),
            kind: AssetKind::Script,
        });
        scene.entities[0].name = "Movement cube".into();
        scene.entities[0].transform.translation.y = 0.5;
        scene.entities[0].color = [0.20, 0.36, 0.61];
        scene.entities[0].script = Some(id);
        scene.entities[0].spin = false;
        let mut floor = Object::cube("Platform".into(), Vec3::Y * -0.14);
        floor.transform.scale = Vec3::new(8.0, 0.28, 8.0);
        floor.color = [0.25; 3];
        floor.spin = false;
        scene.entities.push(floor);
        self.clear_assets();
        scene.script_sources.insert(
            id,
            include_str!("../../../examples/script-playground/Move.rhai").into(),
        );
        self.doc = crate::document::Document::new(scene, None);
        self.script_inspector = true;
        self.viewport.reset_camera();
        self.report(
            "Script Playground ready. Press Play, then use W A S D.",
            false,
        );
    }

    pub(crate) fn place_model(&mut self, id: Uuid) {
        let Some(asset) = self
            .doc
            .scene
            .assets
            .iter()
            .find(|asset| asset.id == id && asset.kind == AssetKind::Model)
        else {
            return;
        };
        if !self.asset_bytes.contains_key(&id) {
            return;
        }
        let [min, max] = self.viewport.bounds(Some(id));
        let right = self
            .doc
            .scene
            .entities
            .iter()
            .filter(|object| object.parent.is_none())
            .map(|object| {
                let [lo, hi] = self.viewport.object_bounds(object);
                let matrix = object.transform.matrix();
                let center = matrix.transform_point3((lo + hi) * 0.5).x;
                let half = (hi - lo) * 0.5;
                center
                    + half.dot(Vec3::new(
                        matrix.x_axis.x.abs(),
                        matrix.y_axis.x.abs(),
                        matrix.z_axis.x.abs(),
                    ))
            })
            .reduce(f32::max);
        let x = right.map_or(-(min.x + max.x) * 0.5, |right| right + 0.6 - min.x);
        let position = Vec3::new(x, -min.y, -(min.z + max.z) * 0.5);
        let mut object = Object::cube(asset.name.clone(), position);
        object.mesh = Some(id);
        object.color = [1.0; 3];
        object.spin = false;
        self.hierarchy_search.clear();
        self.doc.selection.replace(Some(object.uuid));
        self.doc.scene.entities.push(object);
    }

    pub(crate) fn assets_panel(&self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        let edit = self.play.is_none();
        ui.horizontal(|ui| {
            if ui
                .add_enabled(edit, egui::Button::new("Import…"))
                .on_hover_text("Import GLB, PNG, Rhai, WAV or OGG")
                .clicked()
            {
                *action = Some(Action::Import);
            }
            if self.audio.preview_active() && ui.button("Stop preview").clicked() {
                *action = Some(Action::StopAudioPreview);
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.material_rows(ui, action);
            self.prefab_rows(ui, action);
            for asset in &self.doc.scene.assets {
                ui.push_id(asset.id, |ui| {
                    let ready = self.asset_bytes.contains_key(&asset.id)
                        || self.doc.scene.script_sources.contains_key(&asset.id);
                    let mut label =
                        format!("{}   {}", asset.kind.extension().to_uppercase(), asset.name);
                    if let Some(duration) = self.audio.duration(asset.id) {
                        label.push_str(&format!("   {:.2}s", duration.as_secs_f32()));
                    }
                    let color = if ready { crate::TEXT } else { WARNING };
                    if !ready {
                        label.push_str("  ·  Missing or invalid");
                    }
                    let row = ui
                        .add_sized(
                            [ui.available_width(), 26.0],
                            egui::Label::new(RichText::new(label).color(color))
                                .halign(egui::Align::Min)
                                .truncate()
                                .sense(egui::Sense::click()),
                        )
                        .on_hover_text(format!("{} · Right-click for actions", asset.name));
                    if row.double_clicked() && edit && ready {
                        *action = Some(asset_primary_action(asset));
                    }
                    row.context_menu(|ui| self.asset_actions(ui, asset, edit, ready, action));
                });
            }
        });
        let empty = ui.available_size().max(egui::Vec2::ZERO);
        let response = ui.allocate_response(empty, egui::Sense::click());
        if self.doc.scene.assets.is_empty()
            && self.doc.scene.materials.is_empty()
            && self.doc.scene.prefabs.is_empty()
        {
            ui.painter().text(
                response.rect.min + egui::vec2(0.0, 6.0),
                egui::Align2::LEFT_TOP,
                "Drop assets here or use Import…",
                egui::FontId::proportional(13.0),
                MUTED,
            );
        }
        response.context_menu(|ui| {
            menu_item(ui, "Import…", Action::Import, edit, action);
            menu_item(ui, "New Script", Action::NewScript, edit, action);
            menu_item(ui, "New Material", Action::NewMaterial, edit, action);
            menu_item(
                ui,
                "Create Prefab from Selection",
                Action::CreatePrefab,
                edit && self.doc.selection.ids().len() == 1,
                action,
            );
            ui.separator();
            workspace::panels_submenu(ui, action);
        });
    }

    fn asset_actions(
        &self,
        ui: &mut egui::Ui,
        asset: &Asset,
        edit: bool,
        ready: bool,
        action: &mut Option<Action>,
    ) {
        let id = asset.id;
        let enabled = edit && ready;
        let has_selection = self.doc.selection.primary().is_some();
        let (label, attach) = match asset.kind {
            AssetKind::Model => ("Place", None),
            AssetKind::Texture => (
                "Place Sprite",
                Some(("Apply to Selection", Action::ApplyTexture(id))),
            ),
            AssetKind::Script => (
                "Edit Code in VS Code",
                Some(("Attach to Selection", Action::AttachScript(id))),
            ),
            AssetKind::Wav | AssetKind::Ogg => (
                "Preview Audio",
                Some(("Attach to Selection", Action::AttachAudio(id))),
            ),
        };
        menu_item(ui, label, asset_primary_action(asset), enabled, action);
        if asset.kind == AssetKind::Script {
            menu_item(
                ui,
                "Edit in futty",
                Action::EditScriptInFutty(id),
                enabled,
                action,
            );
        }
        if let Some((label, attach)) = attach {
            let target = has_selection
                && (asset.kind != AssetKind::Texture
                    || self.doc.scene.entities.iter().any(|object| {
                        object.has_renderer() && self.doc.selection.ids().contains(&object.uuid)
                    }));
            menu_item(ui, label, attach, enabled && target, action);
        }
        let unused = self
            .doc
            .scene
            .materials
            .values()
            .all(|material| material.texture != Some(id))
            && self
                .script_editor
                .as_ref()
                .is_none_or(|editor| editor.asset != id)
            && self.doc.scene.referenced_objects().all(|object| {
                object.mesh != Some(id)
                    && object.texture != Some(id)
                    && object.sprite.is_none_or(|sprite| sprite.texture != id)
                    && object.script != Some(id)
                    && object.audio_source.is_none_or(|source| source.clip != id)
            });
        ui.separator();
        menu_item(
            ui,
            "Remove from Scene",
            Action::RemoveAsset(id),
            edit && unused,
            action,
        );
    }
}

fn asset_primary_action(asset: &Asset) -> Action {
    match asset.kind {
        AssetKind::Model => Action::Place(asset.id),
        AssetKind::Texture => Action::PlaceSprite(asset.id),
        AssetKind::Script => Action::EditScript(asset.id),
        AssetKind::Wav | AssetKind::Ogg => Action::PreviewAudio(asset.id),
    }
}
