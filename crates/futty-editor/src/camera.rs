use eframe::egui::{self, Pos2, Rect, Vec2};
use futty_core::{Camera, CameraLens, CameraView, Projection};
use futty_runtime::{Object, Scene};
use glam::{Mat4, Quat};
use uuid::Uuid;

use crate::{
    Action, Editor, FOCUS, MUTED, menu_item,
    viewport::{marker, project},
};

impl Editor {
    pub(crate) fn scene_camera_controls(
        &mut self,
        ui: &mut egui::Ui,
        action: &mut Option<Action>,
        playing: bool,
        modal: bool,
    ) {
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!modal, |ui| {
                ui.menu_button("Camera", |ui| {
                    menu_item(
                        ui,
                        "Frame Selection   F",
                        Action::Focus,
                        self.doc.selection.primary().is_some(),
                        action,
                    );
                    menu_item(
                        ui,
                        "Frame All",
                        Action::FrameAll,
                        !self.doc.scene.entities.is_empty(),
                        action,
                    );
                    if ui.button("Reset View").clicked() {
                        self.viewport.reset_camera();
                        ui.close();
                    }
                    ui.separator();
                    ui.add(
                        egui::DragValue::new(&mut self.viewport.speed)
                            .prefix("Fly Speed ")
                            .range(0.5..=45.0)
                            .speed(0.1),
                    );
                    ui.label("Orbit with Alt + left drag");
                });
                ui.add_enabled_ui(!playing, |ui| {
                    let sky = &mut self.doc.scene.sky;
                    ui.menu_button(
                        if *sky {
                            "Background: Sky"
                        } else {
                            "Background: Solid"
                        },
                        |ui| {
                            if ui.selectable_value(sky, true, "Sky").clicked()
                                || ui.selectable_value(sky, false, "Solid").clicked()
                            {
                                ui.close();
                            }
                        },
                    );
                });
            });
        });
    }

    pub(crate) fn add_camera(&mut self) {
        let mut object = Object::cube("Camera".into(), self.viewport.camera.eye);
        object.spin = false;
        object.camera = Some(CameraLens::default());
        object.transform.rotation = self.viewport.camera.view().rotation;
        if self.doc.scene.camera_index(None).is_none() {
            object.name = "Main Camera".into();
            self.doc.scene.main_camera = Some(object.uuid);
        }
        self.doc.selection.replace(Some(object.uuid));
        self.doc.scene.entities.push(object);
        self.hierarchy_search.clear();
        self.script_inspector = false;
        self.report(
            "Camera placed at the Scene view. Open Game to check the framing.",
            false,
        );
    }

    pub(crate) fn align_camera(&mut self, id: Uuid) {
        if align_to_view(&mut self.doc.scene, id, self.viewport.camera) {
            self.report("Camera aligned to the Scene view", false);
        }
    }
}

fn align_to_view(scene: &mut Scene, id: Uuid, view: Camera) -> bool {
    let Some(index) = scene
        .entities
        .iter()
        .position(|object| object.uuid == id && object.camera.is_some())
    else {
        return false;
    };
    let Ok(hierarchy) = scene.hierarchy() else {
        return false;
    };
    let view = view.view();
    let (inverse, parent_rotation) =
        hierarchy.parents[index].map_or((Mat4::IDENTITY, Quat::IDENTITY), |parent| {
            let world = hierarchy.world_matrices(|i| scene.entities[i].transform.matrix());
            (
                world[parent].inverse(),
                hierarchy.world_rotation(parent, |i| scene.entities[i].transform.rotation),
            )
        });
    let translation = inverse.transform_point3(view.eye);
    if !translation.is_finite() {
        return false;
    }
    let transform = &mut scene.entities[index].transform;
    transform.translation = translation;
    transform.rotation = (parent_rotation.conjugate() * view.rotation).normalize();
    true
}

pub(crate) fn inspector(ui: &mut egui::Ui, lens: &mut CameraLens, main: bool) {
    ui.strong("Camera");
    ui.separator();
    if main {
        ui.label(egui::RichText::new("Main Camera").color(FOCUS));
    }
    let is_perspective = lens.projection == Projection::Perspective;
    ui.label("Projection");
    egui::ComboBox::from_id_salt("camera-projection")
        .selected_text(if is_perspective {
            "Perspective"
        } else {
            "Orthographic"
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut lens.projection, Projection::Perspective, "Perspective");
            ui.selectable_value(
                &mut lens.projection,
                Projection::Orthographic,
                "Orthographic",
            );
        });
    ui.horizontal(|ui| {
        if lens.projection == Projection::Perspective {
            ui.label("Field of view");
            ui.add(
                egui::DragValue::new(&mut lens.fov)
                    .range(5.0..=160.0)
                    .speed(0.2)
                    .suffix("°"),
            );
        } else {
            ui.label("Orthographic size");
            ui.add(
                egui::DragValue::new(&mut lens.size)
                    .range(0.05..=500.0)
                    .speed(0.05),
            )
            .on_hover_text("Half the vertical size in world units");
        }
    });
    if lens.projection == Projection::Orthographic {
        let mut enabled = lens.pixels_per_unit.is_some();
        if ui.checkbox(&mut enabled, "Pixel perfect").changed() {
            lens.pixels_per_unit = enabled.then_some(32);
        }
        if let Some(pixels) = &mut lens.pixels_per_unit {
            ui.add(
                egui::DragValue::new(pixels)
                    .range(1..=1024)
                    .prefix("Pixels per unit "),
            );
        }
    }
    ui.label("Clipping planes");
    ui.horizontal_wrapped(|ui| {
        let near_max = (lens.far - 0.01).clamp(0.01, 9999.0);
        ui.add(
            egui::DragValue::new(&mut lens.near)
                .range(0.01..=near_max)
                .speed(0.01)
                .prefix("Near "),
        );
        ui.add(
            egui::DragValue::new(&mut lens.far)
                .range(lens.near + 0.01..=10_000.0)
                .speed(0.2)
                .prefix("Far "),
        );
    });
}

pub(crate) fn paint(
    ui: &egui::Ui,
    rect: Rect,
    editor_view: Mat4,
    view: CameraView,
    aspect: f32,
    selected: bool,
) {
    let painter = ui.painter().with_clip_rect(rect);
    if let Some(icon) = marker(editor_view, rect, view.eye) {
        let center = icon.center();
        let body = Rect::from_center_size(center - Vec2::new(3.0, 0.0), Vec2::new(17.0, 13.0));
        let color = if selected { FOCUS } else { MUTED };
        painter.rect(
            body,
            2.0,
            crate::BACKGROUND,
            (1.3, color),
            egui::StrokeKind::Inside,
        );
        painter.line(
            vec![
                Pos2::new(body.right(), center.y - 3.0),
                center + Vec2::new(12.0, -7.0),
                center + Vec2::new(12.0, 7.0),
                Pos2::new(body.right(), center.y + 3.0),
            ],
            (1.3, color),
        );
    }
    if selected {
        let corners: [_; 8] = std::array::from_fn(|i| {
            let d = if i & 4 == 0 {
                view.lens.near
            } else {
                view.lens.far
            };
            let extent = view.lens.half_height(d);
            let side = |bit| if i & bit == 0 { -1.0 } else { 1.0 };
            let point = view.eye
                + view.forward() * d
                + view.right() * (extent * aspect * side(1))
                + view.up() * (extent * side(2));
            project(editor_view, rect, point)
        });
        for (i, point) in corners.iter().enumerate() {
            for bit in [1, 2, 4] {
                if i & bit == 0
                    && let (Some(a), Some(b)) = (*point, corners[i | bit])
                {
                    painter.line_segment([a, b], (1.1, FOCUS.gamma_multiply(0.8)));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn align_to_view_resolves_parent_scale_and_rotation() {
        let mut scene = Scene::default();
        let parent = &mut scene.entities[0];
        parent.transform.scale = Vec3::new(2.0, 1.0, 3.0);
        parent.transform.rotation = Quat::from_rotation_y(0.9);
        let mut object = Object::cube("Camera".into(), Vec3::ZERO);
        object.parent = Some(parent.uuid);
        object.camera = Some(CameraLens::default());
        let id = object.uuid;
        scene.entities.push(object);
        let camera = Camera::default();
        assert!(align_to_view(&mut scene, id, camera));
        let hierarchy = scene.hierarchy().unwrap();
        let local: Vec<_> = scene
            .entities
            .iter()
            .map(|object| object.transform)
            .collect();
        let worlds = hierarchy.world_matrices(|i| local[i].matrix());
        let camera_view = scene.camera_view(1, &hierarchy, &worlds, &local).unwrap();
        assert!(camera_view.eye.abs_diff_eq(camera.eye, 1e-5));
        assert!(camera_view.forward().abs_diff_eq(camera.forward(), 1e-5));
    }
}
