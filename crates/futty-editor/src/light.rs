use eframe::egui::{self, Rect, Vec2};
use futty_core::{Light, LightKind};
use futty_runtime::Object;
use glam::{Mat4, Quat, Vec3};

use crate::{
    Editor, FOCUS, WARNING,
    viewport::{marker, project},
};

impl Editor {
    pub(crate) fn add_light(&mut self, kind: LightKind) {
        let scene = &self.doc.scene;
        let center = self
            .doc
            .selection
            .primary()
            .and_then(|id| {
                scene
                    .entities
                    .iter()
                    .position(|object| object.uuid == id && object.has_renderer())
            })
            .and_then(|index| {
                scene.hierarchy().ok().map(|tree| {
                    tree.world_matrices(|index| scene.entities[index].transform.matrix())[index]
                        .w_axis
                        .truncate()
                })
            })
            .filter(|center| center.is_finite())
            .unwrap_or(Vec3::ZERO);
        let mut object = Object::cube(kind.label().into(), center + Vec3::new(0.7, 2.0, 0.8));
        object.spin = false;
        object.light = Some(Light::new(kind));
        if kind == LightKind::Directional {
            object.transform.rotation =
                Quat::from_rotation_arc(Vec3::Z, Vec3::new(0.35, 1.0, 0.65).normalize());
        }
        self.doc.selection.replace(Some(object.uuid));
        self.doc.scene.entities.push(object);
        self.hierarchy_search.clear();
        self.script_inspector = false;
        self.report(format!("Added {}", kind.label()), false);
    }
}

pub(crate) fn inspector(ui: &mut egui::Ui, light: &mut Light) {
    ui.strong("Light");
    ui.separator();
    egui::ComboBox::from_id_salt("light-type")
        .selected_text(light.kind.label())
        .show_ui(ui, |ui| {
            for kind in [LightKind::Directional, LightKind::Point] {
                ui.selectable_value(&mut light.kind, kind, kind.label());
            }
        });
    ui.horizontal(|ui| {
        ui.label("Color");
        ui.color_edit_button_rgb(&mut light.color);
    });
    ui.horizontal(|ui| {
        ui.label("Intensity");
        ui.add(
            egui::DragValue::new(&mut light.intensity)
                .range(0.0..=10.0)
                .speed(0.025),
        );
    });
    if light.kind == LightKind::Point {
        ui.horizontal(|ui| {
            ui.label("Range");
            ui.add(
                egui::DragValue::new(&mut light.range)
                    .range(0.1..=250.0)
                    .speed(0.05),
            );
        });
    } else {
        ui.checkbox(&mut light.cast_shadows, "Cast Shadows");
        ui.add_enabled_ui(light.cast_shadows, |ui| {
            ui.horizontal(|ui| {
                ui.label("Shadow Distance");
                ui.add(
                    egui::DragValue::new(&mut light.shadow_distance)
                        .range(2.0..=100.0)
                        .speed(0.15),
                )
                .on_hover_text("Maximum distance from the viewing camera");
            });
        });
    }
}

pub(crate) fn paint(
    ui: &egui::Ui,
    rect: Rect,
    vp: Mat4,
    position: Vec3,
    rotation: Quat,
    light: Light,
    selected: bool,
) {
    let painter = ui.painter().with_clip_rect(rect);
    let color = if selected { FOCUS } else { WARNING };
    if let Some(icon) = marker(vp, rect, position) {
        let center = icon.center();
        painter.circle(center, 4.0, crate::BACKGROUND, (1.3, color));
        for index in 0..8 {
            let (sin, cos) = (index as f32 * std::f32::consts::FRAC_PI_4).sin_cos();
            let direction = Vec2::new(cos, sin);
            painter.line_segment(
                [center + direction * 6.0, center + direction * 10.5],
                (1.2, color),
            );
        }
    }
    if !selected {
        return;
    }
    let project = |point| project(vp, rect, point);
    match light.kind {
        LightKind::Directional => {
            let direction = rotation * Vec3::NEG_Z;
            for offset in [
                Vec3::ZERO,
                rotation * Vec3::X * 0.3,
                rotation * Vec3::NEG_X * 0.3,
            ] {
                if let (Some(start), Some(end)) = (
                    project(position + offset),
                    project(position + offset + direction * 2.3),
                ) {
                    painter.arrow(start, end - start, (1.3, color));
                }
            }
        }
        LightKind::Point => {
            const STEPS: u32 = 32;
            for (a, b) in [(Vec3::X, Vec3::Y), (Vec3::X, Vec3::Z), (Vec3::Y, Vec3::Z)] {
                let mut last = None;
                for i in 0..=STEPS {
                    let (sin, cos) = (i as f32 * std::f32::consts::TAU / STEPS as f32).sin_cos();
                    let point = project(position + (a * cos + b * sin) * light.range);
                    if let (Some(start), Some(end)) = (last, point) {
                        painter.line_segment([start, end], (1.0, color.gamma_multiply(0.55)));
                    }
                    last = point;
                }
            }
        }
    }
}
