use std::collections::HashMap;

use eframe::egui;
use futty_core::{Asset, AssetKind};
use futty_runtime::{Billboard, Object, Scene, SceneDimension, Sprite, Sprite2d, SpriteSheet};
use glam::{Vec2, Vec3};
use uuid::Uuid;

use crate::{Editor, document::Document};

fn sprite_object(asset: &Asset, position: Vec3, size: Vec2, billboard: Billboard) -> Object {
    let mut object = Object::cube(asset.name.clone(), position);
    object.color = [1.0; 3];
    object.spin = false;
    object.sprite = Some(Sprite {
        texture: asset.id,
        size,
        billboard,
        sheet: None,
        planar: None,
    });
    object
}

impl Editor {
    pub(crate) fn place_sprite(&mut self, id: Uuid) {
        let Some(asset) = self.doc.scene.assets.iter().find(|asset| asset.id == id) else {
            return;
        };
        let Some([width, height]) = self.viewport.texture_size(id) else {
            return;
        };
        let size = (Vec2::new(width as f32, height as f32) * (1.8 / width.max(height) as f32))
            .max(Vec2::splat(0.01));
        let x = self
            .doc
            .scene
            .entities
            .iter()
            .filter(|object| object.parent.is_none())
            .map(|object| object.transform.translation.x)
            .reduce(f32::max)
            .map_or(0.0, |x| x + 2.0);
        let mut object = sprite_object(
            asset,
            Vec3::new(x, size.y * 0.5, 0.0),
            size,
            Billboard::FaceCamera,
        );
        if self.doc.scene.dimension == SceneDimension::TwoD {
            let sprite = object.sprite.as_mut().unwrap();
            sprite.planar = Some(Sprite2d::default());
            sprite.billboard = Billboard::Fixed;
        }
        self.hierarchy_search.clear();
        self.doc.selection.replace(Some(object.uuid));
        self.doc.scene.entities.push(object);
        self.script_inspector = false;
        self.focus_selection();
    }

    pub(crate) fn load_sprite_playground(&mut self) {
        let asset = Asset {
            id: Uuid::new_v4(),
            name: "Signal".into(),
            kind: AssetKind::Texture,
        };
        let bytes = include_bytes!("../../../examples/sprite-playground/Signal.png");
        if let Err(error) = futty_assets::png(bytes) {
            self.report(format!("Sprite Playground failed: {error}"), true);
            return;
        }
        self.clear_assets();
        if let Err(error) = self.viewport.upload(&asset, bytes) {
            self.report(format!("Sprite Playground failed: {error}"), true);
            return;
        }
        self.asset_bytes.insert(asset.id, bytes.to_vec());
        let mut scene = Scene::default();
        scene.name = "Sprite Playground".into();
        let mut floor = Object::cube("Platform".into(), Vec3::Y * -0.08);
        floor.transform.scale = Vec3::new(7.0, 0.16, 5.0);
        floor.color = [0.22; 3];
        floor.spin = false;
        scene.entities = vec![floor];
        for (x, mode) in [
            (-2.0, Billboard::FaceCamera),
            (0.0, Billboard::Upright),
            (2.0, Billboard::Fixed),
        ] {
            let mut object = sprite_object(&asset, Vec3::new(x, 1.1, 0.0), Vec2::splat(1.7), mode);
            object.name = mode.label().into();
            scene.entities.push(object);
        }
        scene.assets.push(asset);
        self.doc = Document::new(scene, None);
        self.doc
            .selection
            .replace(Some(self.doc.scene.entities[1].uuid));
        self.script_inspector = false;
        self.viewport.reset_camera();
        self.report(
            "Sprite Playground ready. Orbit the camera to compare billboard modes.",
            false,
        );
    }
}

pub(crate) fn inspector(
    ui: &mut egui::Ui,
    sprite: &mut Sprite,
    assets: &[Asset],
    loaded: &HashMap<Uuid, Vec<u8>>,
) {
    ui.strong(if sprite.planar.is_some() {
        "Sprite2D"
    } else {
        "Sprite3D"
    });
    ui.separator();
    let texture = assets
        .iter()
        .find(|asset| asset.id == sprite.texture)
        .map_or("Missing texture", |asset| asset.name.as_str());
    ui.label("Texture");
    egui::ComboBox::from_id_salt("sprite-texture")
        .width(ui.available_width())
        .selected_text(texture)
        .show_ui(ui, |ui| {
            for asset in assets
                .iter()
                .filter(|asset| asset.kind == AssetKind::Texture)
            {
                ui.add_enabled_ui(loaded.contains_key(&asset.id), |ui| {
                    ui.selectable_value(&mut sprite.texture, asset.id, &asset.name);
                });
            }
        });
    let mut planar = sprite.planar.is_some();
    if ui.checkbox(&mut planar, "XY sprite").changed() {
        sprite.planar = planar.then(Sprite2d::default);
    }
    if let Some(planar) = &mut sprite.planar {
        ui.add(egui::DragValue::new(&mut planar.layer).prefix("Layer "));
        ui.horizontal(|ui| {
            for (axis, value) in ["Pivot X ", "Y "].into_iter().zip(&mut planar.pivot) {
                ui.add(
                    egui::DragValue::new(value)
                        .range(0.0..=1.0)
                        .speed(0.01)
                        .prefix(axis),
                );
            }
        });
        ui.horizontal(|ui| {
            ui.checkbox(&mut planar.flip[0], "Flip X");
            ui.checkbox(&mut planar.flip[1], "Flip Y");
        });
    } else {
        ui.label("Billboard");
        egui::ComboBox::from_id_salt("billboard")
            .width(ui.available_width())
            .selected_text(sprite.billboard.label())
            .show_ui(ui, |ui| {
                for mode in [Billboard::FaceCamera, Billboard::Upright, Billboard::Fixed] {
                    ui.selectable_value(&mut sprite.billboard, mode, mode.label());
                }
            });
    }
    ui.horizontal(|ui| {
        for (label, value) in [("W ", &mut sprite.size.x), ("H ", &mut sprite.size.y)] {
            ui.add(
                egui::DragValue::new(value)
                    .range(0.01..=1000.0)
                    .speed(0.02)
                    .prefix(label),
            );
        }
    });
    let mut use_sheet = sprite.sheet.is_some();
    if ui.checkbox(&mut use_sheet, "Sprite sheet").changed() {
        sprite.sheet = use_sheet.then_some(SpriteSheet {
            columns: 1,
            rows: 1,
            first_frame: 0,
            frames: 1,
            fps: 8.0,
            looping: true,
        });
    }
    if let Some(sheet) = &mut sprite.sheet {
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut sheet.columns)
                    .range(1..=256)
                    .prefix("Columns "),
            );
            ui.add(
                egui::DragValue::new(&mut sheet.rows)
                    .range(1..=256)
                    .prefix("Rows "),
            );
        });
        let cells = u32::from(sheet.columns) * u32::from(sheet.rows);
        let last = (cells - 1) as u16;
        sheet.first_frame = sheet.first_frame.min(last);
        ui.add(
            egui::DragValue::new(&mut sheet.first_frame)
                .range(0..=last)
                .prefix("First cell "),
        );
        let available = (cells - u32::from(sheet.first_frame)).min(u32::from(u16::MAX)) as u16;
        sheet.frames = sheet.frames.min(available);
        ui.add(
            egui::DragValue::new(&mut sheet.frames)
                .range(1..=available)
                .prefix("Frames "),
        );
        ui.add(
            egui::DragValue::new(&mut sheet.fps)
                .range(1.0..=60.0)
                .speed(0.1)
                .suffix(" fps"),
        );
        ui.checkbox(&mut sheet.looping, "Loop");
    }
}
