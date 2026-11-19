use std::collections::HashMap;

use crate::selection::selected_tree;
use eframe::egui::{self, Key, Pos2, Rect, Vec2};
use futty_core::{Camera, Transform};
use futty_runtime::{Hierarchy, Object, Scene};
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum AxisSpace {
    #[default]
    World,
    Local,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum PivotMode {
    #[default]
    Center,
    Active,
    Individual,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct TransformSettings {
    pub space: AxisSpace,
    pub pivot: PivotMode,
    pub snap: bool,
    pub move_step: f32,
    pub rotate_step: f32,
    pub scale_step: f32,
}

impl Default for TransformSettings {
    fn default() -> Self {
        Self {
            space: AxisSpace::World,
            pivot: PivotMode::Center,
            snap: false,
            move_step: 0.5,
            rotate_step: 15.0,
            scale_step: 0.1,
        }
    }
}

impl TransformSettings {
    pub fn valid(self) -> bool {
        (0.001..=100.0).contains(&self.move_step)
            && (0.1..=180.0).contains(&self.rotate_step)
            && (0.001..=10.0).contains(&self.scale_step)
    }

    pub fn snapped(self, value: f32, step: f32, temporary: bool) -> f32 {
        if self.snap || temporary {
            (value / step).round() * step
        } else {
            value
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tool {
    Move,
    Scale,
    Rotate,
}

impl Tool {
    pub const KEYS: [(Key, Self); 3] = [
        (Key::G, Self::Move),
        (Key::S, Self::Scale),
        (Key::R, Self::Rotate),
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Move => "Move",
            Self::Scale => "Scale",
            Self::Rotate => "Rotate",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Move => "G",
            Self::Scale => "S",
            Self::Rotate => "R",
        }
    }
}

pub struct TransformTool {
    pub preview: HashMap<Uuid, Transform>,
    group: TransformGroup,
    tool: Tool,
    axis: Option<usize>,
    anchor: Pos2,
    input: String,
    value: f32,
}

pub(crate) struct TransformGroup {
    targets: Vec<Target>,
    pub pivot: Vec3,
    basis: Quat,
    pub settings: TransformSettings,
}

struct Target {
    index: usize,
    id: Uuid,
    initial: Transform,
    parent_inverse: Mat4,
    parent_rotation: Quat,
    position: Vec3,
}

#[derive(Clone, Copy)]
enum Change {
    Move(Vec3),
    Scale(Vec3),
    Rotate { axis: Vec3, angle: f32 },
}

impl TransformGroup {
    pub fn capture(
        objects: &[Object],
        hierarchy: &Hierarchy,
        worlds: &[Mat4],
        selected: &[Uuid],
        settings: TransformSettings,
    ) -> Option<Self> {
        let mut targets = Vec::new();
        let mut pivot = Vec3::ZERO;
        for (index, _) in selected_tree(objects, hierarchy, selected).filter(|&(_, root)| root) {
            let world = worlds[index];
            let position = world.w_axis.truncate();
            let parent_inverse =
                hierarchy.parents[index].map_or(Mat4::IDENTITY, |parent| worlds[parent].inverse());
            if !parent_inverse.is_finite() || !position.is_finite() {
                return None;
            }
            pivot += position;
            targets.push(Target {
                index,
                id: objects[index].uuid,
                initial: objects[index].transform,
                parent_inverse,
                parent_rotation: hierarchy.parents[index].map_or(Quat::IDENTITY, |parent| {
                    hierarchy.world_rotation(parent, |i| objects[i].transform.rotation)
                }),
                position,
            });
        }
        if targets.is_empty() {
            return None;
        }
        let active = selected
            .last()
            .and_then(|id| objects.iter().position(|object| object.uuid == *id))?;
        let basis = hierarchy.world_rotation(active, |i| objects[i].transform.rotation);
        let pivot = if settings.pivot == PivotMode::Active {
            worlds[active].w_axis.truncate()
        } else {
            pivot / targets.len() as f32
        };
        pivot.is_finite().then_some(Self {
            targets,
            pivot,
            basis,
            settings,
        })
    }

    pub fn axis_direction(&self, axis: usize, scale: bool) -> Vec3 {
        if scale || self.settings.space == AxisSpace::Local {
            self.basis * Vec3::AXES[axis]
        } else {
            Vec3::AXES[axis]
        }
    }

    fn transformed(&self, target: &Target, change: Change) -> Transform {
        let pivot = if self.settings.pivot == PivotMode::Individual {
            target.position
        } else {
            self.pivot
        };
        target.transformed(change, pivot, self.basis)
    }

    pub fn move_objects(&self, objects: &mut [Object], delta: Vec3) {
        for target in &self.targets {
            if let Some(object) = objects
                .get_mut(target.index)
                .filter(|object| object.uuid == target.id)
            {
                object.transform = self.transformed(target, Change::Move(delta));
            }
        }
    }
}

impl Target {
    fn transformed(&self, change: Change, pivot: Vec3, basis: Quat) -> Transform {
        let mut next = self.initial;
        let offset = self.position - pivot;
        let delta = match change {
            Change::Move(delta) => delta,
            Change::Scale(factor) => {
                next.scale = (next.scale * factor).clamp(Vec3::splat(0.01), Vec3::splat(1000.0));
                basis * ((basis.conjugate() * offset) * factor) - offset
            }
            Change::Rotate { axis, angle } => {
                let local = self.parent_rotation.conjugate() * axis;
                if local != Vec3::ZERO && angle != 0.0 {
                    next.rotation =
                        (Quat::from_axis_angle(local, angle) * next.rotation).normalize();
                }
                Quat::from_axis_angle(axis, angle) * offset - offset
            }
        };
        let translation = next.translation + self.parent_inverse.transform_vector3(delta);
        if delta != Vec3::ZERO && translation.is_finite() {
            next.translation = translation.clamp(Vec3::splat(-10_000.0), Vec3::splat(10_000.0));
        }
        next
    }
}

impl TransformTool {
    pub fn new(
        scene: &Scene,
        selected: &[Uuid],
        tool: Tool,
        anchor: Pos2,
        settings: TransformSettings,
    ) -> Option<Self> {
        let hierarchy = scene.hierarchy().ok()?;
        let worlds = hierarchy.world_matrices(|index| scene.entities[index].transform.matrix());
        let group =
            TransformGroup::capture(&scene.entities, &hierarchy, &worlds, selected, settings)?;
        Some(Self {
            preview: HashMap::new(),
            group,
            tool,
            axis: None,
            anchor,
            input: String::new(),
            value: if tool == Tool::Scale { 1.0 } else { 0.0 },
        })
    }

    pub fn update(&mut self, context: &egui::Context, camera: Camera, rect: Rect) -> Option<bool> {
        let (pointer, done) = context.input_mut(|input| {
            for (axis, key) in [Key::X, Key::Y, Key::Z].into_iter().enumerate() {
                if input.events.iter().any(|event| {
                    matches!(event, egui::Event::Key {
                    key: event_key, pressed: true, repeat: false, ..
                } if *event_key == key)
                }) && input.consume_key(egui::Modifiers::NONE, key)
                {
                    self.axis = (self.axis != Some(axis)).then_some(axis);
                }
            }
            if input.consume_key(egui::Modifiers::NONE, Key::Backspace) {
                self.input.pop();
            }
            for text in input.events.iter().filter_map(|event| {
                if let egui::Event::Text(text) = event {
                    Some(text)
                } else {
                    None
                }
            }) {
                for digit in text
                    .chars()
                    .filter(|char| char.is_ascii_digit() || matches!(char, '.' | '-'))
                {
                    if self.input.len() < 12 {
                        self.input.push(digit);
                    }
                }
            }
            let cancelled = input.consume_key(egui::Modifiers::NONE, Key::Escape)
                || input
                    .pointer
                    .button_released(egui::PointerButton::Secondary)
                || !input.focused;
            let confirmed = input.consume_key(egui::Modifiers::NONE, Key::Enter)
                || input.pointer.button_released(egui::PointerButton::Primary);
            (
                input.pointer.hover_pos().unwrap_or(self.anchor),
                if cancelled {
                    Some(false)
                } else if confirmed && (self.input.is_empty() || self.number().is_some()) {
                    Some(true)
                } else {
                    None
                },
            )
        });
        let temporary = context.input(|input| input.modifiers.ctrl);
        let (change, value) = self.calculate(camera, rect, pointer - self.anchor, temporary);
        self.preview.clear();
        self.preview.extend(
            self.group
                .targets
                .iter()
                .map(|target| (target.id, self.group.transformed(target, change))),
        );
        self.value = value;
        done
    }

    fn number(&self) -> Option<f32> {
        self.input
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
    }

    fn calculate(&self, camera: Camera, rect: Rect, delta: Vec2, temporary: bool) -> (Change, f32) {
        let number = self.number();
        let axis = self
            .axis
            .map(|index| self.group.axis_direction(index, self.tool == Tool::Scale));
        let settings = self.group.settings;
        let view_plane =
            camera.right() * delta.x - camera.right().cross(camera.forward()) * delta.y;
        let pivot = self.group.pivot;
        match self.tool {
            Tool::Move => {
                let depth = (pivot - camera.eye).dot(camera.forward()).max(0.5);
                let units = 2.0 * depth * 25.0_f32.to_radians().tan() / rect.height().max(1.0);
                let motion = if let Some(axis) = axis {
                    let vp = camera.view_projection(rect.aspect_ratio());
                    let projected = crate::viewport::project(vp, rect, pivot + axis)
                        .zip(crate::viewport::project(vp, rect, pivot))
                        .map(|(end, start)| end - start)
                        .filter(|direction| direction.length_sq() > 4.0);
                    let distance = number.unwrap_or_else(|| {
                        settings.snapped(
                            projected.map_or(-delta.y * units, |direction| {
                                delta.dot(direction) / direction.length_sq()
                            }),
                            settings.move_step,
                            temporary,
                        )
                    });
                    axis * distance
                } else if let Some(distance) = number {
                    view_plane.try_normalize().unwrap_or_else(|| camera.right()) * distance
                } else {
                    let basis = if settings.space == AxisSpace::Local {
                        self.group.basis
                    } else {
                        Quat::IDENTITY
                    };
                    let local = basis.conjugate() * (view_plane * units);
                    basis
                        * Vec3::new(
                            settings.snapped(local.x, settings.move_step, temporary),
                            settings.snapped(local.y, settings.move_step, temporary),
                            settings.snapped(local.z, settings.move_step, temporary),
                        )
                };
                (
                    Change::Move(motion),
                    axis.map_or_else(
                        || number.unwrap_or(motion.length()),
                        |axis| motion.dot(axis),
                    ),
                )
            }
            Tool::Scale => {
                let factor = number
                    .unwrap_or_else(|| {
                        1.0 + settings.snapped(
                            ((delta.x - delta.y) * 0.008).exp() - 1.0,
                            settings.scale_step,
                            temporary,
                        )
                    })
                    .clamp(0.001, 1000.0);
                let scale = self.axis.map_or(Vec3::splat(factor), |axis| {
                    Vec3::ONE + Vec3::AXES[axis] * (factor - 1.0)
                });
                (Change::Scale(scale), factor)
            }
            Tool::Rotate => {
                let axis = axis.unwrap_or(-camera.forward()).normalize();
                let degrees = number.unwrap_or_else(|| {
                    settings.snapped(delta.x - delta.y, settings.rotate_step, temporary)
                });
                (
                    Change::Rotate {
                        axis,
                        angle: degrees.rem_euclid(360.0).to_radians(),
                    },
                    degrees,
                )
            }
        }
    }

    pub fn paint(&self, ui: &egui::Ui, rect: Rect, camera: Camera) {
        let axis = match self.axis {
            Some(axis) => {
                let (name, color) = crate::viewport::AXES[axis];
                let local_scale =
                    self.tool == Tool::Scale || self.group.settings.space == AxisSpace::Local;
                let direction = self.group.axis_direction(axis, self.tool == Tool::Scale);
                let vp = camera.view_projection(rect.aspect_ratio());
                let pivot = self.group.pivot;
                if let Some(center) = crate::viewport::project(vp, rect, pivot)
                    && let Some(end) = crate::viewport::project(vp, rect, pivot + direction)
                    && (end - center).length_sq() > 1.0
                {
                    let span = (end - center).normalized() * rect.size().length();
                    ui.painter().with_clip_rect(rect).line_segment(
                        [center - span, center + span],
                        (1.4, color.gamma_multiply(0.8)),
                    );
                }
                (
                    format!("{} {name}", if local_scale { "Local" } else { "World" }),
                    color,
                )
            }
            None if self.tool == Tool::Scale => ("Uniform".into(), crate::FOCUS),
            None => ("View".into(), crate::FOCUS),
        };
        let value = if self.input.is_empty() || self.number().is_some() {
            self.formatted_value()
        } else {
            self.input.clone()
        };
        paint_banner(
            ui,
            rect,
            self.tool.key(),
            &format!(
                "{}   ·   {}   ·   {value}   ·   {} object(s)",
                self.tool.name(),
                axis.0,
                self.group.targets.len()
            ),
            axis.1,
        );
    }

    fn formatted_value(&self) -> String {
        match self.tool {
            Tool::Move => format!("{:+.3} m", self.value),
            Tool::Scale => format!("×{:.3}", self.value),
            Tool::Rotate => format!("{:+.1}°", self.value),
        }
    }

    pub fn feedback(&self, applied: bool) -> (&'static str, String) {
        let message = if applied {
            format!(
                "{} applied   ·   {}",
                self.tool.name(),
                self.formatted_value()
            )
        } else {
            format!("{} canceled", self.tool.name())
        };
        (self.tool.key(), message)
    }
}

pub fn paint_banner(ui: &egui::Ui, rect: Rect, key: &str, message: &str, color: egui::Color32) {
    let painter = ui.painter().with_clip_rect(rect);
    let accessible_label = format!("{key}: {message}");
    let key = painter.layout_no_wrap(key.into(), egui::FontId::monospace(16.0), crate::BACKGROUND);
    let message = painter.layout_no_wrap(
        message.into(),
        egui::FontId::proportional(16.0),
        crate::TEXT,
    );
    let outer = Rect::from_min_size(
        rect.min + Vec2::splat(13.0),
        Vec2::new(message.size().x + 60.0, 44.0),
    );
    ui.interact(
        outer.intersect(rect),
        ui.scope_id().with("transform-feedback"),
        egui::Sense::hover(),
    )
    .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &accessible_label));
    painter.rect_filled(outer, 7.0, crate::PANEL);
    painter.rect_stroke(outer, 7.0, (1.0, color), egui::StrokeKind::Inside);
    let badge = Rect::from_min_size(outer.min + Vec2::splat(8.0), Vec2::splat(28.0));
    painter.rect_filled(badge, 4.0, color);
    painter.galley(badge.center() - key.size() * 0.5, key, crate::BACKGROUND);
    let position = Pos2::new(
        badge.right() + 10.0,
        outer.center().y - message.size().y * 0.5,
    );
    painter.galley(position, message, crate::TEXT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_move_scale_and_rotation_are_based_on_original_transform() {
        let scene = Scene::default();
        let object = &scene.entities[0];
        let camera = Camera::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0));
        let mut tool = TransformTool::new(
            &scene,
            &[object.uuid],
            Tool::Move,
            Pos2::ZERO,
            TransformSettings::default(),
        )
        .unwrap();
        let result = |tool: &TransformTool| {
            let (change, _) = tool.calculate(camera, rect, Vec2::ZERO, false);
            tool.group.transformed(&tool.group.targets[0], change)
        };
        tool.axis = Some(0);
        tool.input = "2".into();
        assert_eq!(
            result(&tool).translation,
            object.transform.translation + Vec3::X * 2.0
        );
        tool.tool = Tool::Scale;
        assert_eq!(result(&tool).scale, Vec3::new(2.0, 1.0, 1.0));
        tool.tool = Tool::Rotate;
        tool.axis = Some(1);
        tool.input = "90".into();
        assert!(
            result(&tool)
                .rotation
                .abs_diff_eq(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), 1e-5)
        );
        assert_eq!(scene.entities[0].transform, tool.group.targets[0].initial);
    }

    #[test]
    fn group_uses_common_world_delta_and_preserves_selected_child_local_transform() {
        let mut scene = Scene::default();
        let root = scene.entities[0].uuid;
        scene.entities[0].transform.scale = Vec3::splat(2.0);
        let mut child = Object::cube("Child".into(), Vec3::X);
        child.parent = Some(root);
        let child_id = child.uuid;
        let child_transform = child.transform;
        let outside = Object::cube("Outside".into(), Vec3::X * 4.0);
        let outside_id = outside.uuid;
        scene.entities.extend([child, outside]);
        let hierarchy = scene.hierarchy().unwrap();
        let worlds = hierarchy.world_matrices(|i| scene.entities[i].transform.matrix());
        let group = TransformGroup::capture(
            &scene.entities,
            &hierarchy,
            &worlds,
            &[root, child_id, outside_id],
            TransformSettings::default(),
        )
        .unwrap();
        assert_eq!(group.targets.len(), 2);
        let delta = Vec3::new(2.5, 0.0, 1.0);
        group.move_objects(&mut scene.entities, delta);
        group.move_objects(&mut scene.entities, delta);
        let moved = hierarchy.world_matrices(|i| scene.entities[i].transform.matrix());
        for (before, after) in worlds.iter().zip(moved) {
            assert!((before.w_axis.truncate() + delta).abs_diff_eq(after.w_axis.truncate(), 1e-5));
        }
        assert_eq!(scene.entities[1].transform, child_transform);
        let scale = group.transformed(&group.targets[1], Change::Scale(Vec3::splat(2.0)));
        assert_eq!(scale.translation.x, 6.0);
        let rotation = group.transformed(
            &group.targets[1],
            Change::Rotate {
                axis: Vec3::Y,
                angle: std::f32::consts::PI,
            },
        );
        assert!(rotation.translation.x.abs() < 1e-5);
    }

    #[test]
    fn local_axes_snapping_and_pivot_modes_produce_distinct_transforms() {
        let mut scene = Scene::default();
        scene.entities[0].transform.translation = Vec3::ZERO;
        scene.entities[0].transform.rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let a = scene.entities[0].uuid;
        let b = Object::cube("Active".into(), Vec3::X * 4.0);
        let b_id = b.uuid;
        scene.entities.push(b);
        let settings = TransformSettings {
            space: AxisSpace::Local,
            snap: true,
            ..Default::default()
        };
        let camera = Camera::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0));
        let mut tool = TransformTool::new(&scene, &[a], Tool::Move, Pos2::ZERO, settings).unwrap();
        tool.axis = Some(0);
        tool.input = "2".into();
        let (change, _) = tool.calculate(camera, rect, Vec2::ZERO, false);
        let transformed = tool.group.transformed(&tool.group.targets[0], change);
        assert!(transformed.translation.abs_diff_eq(Vec3::NEG_Z * 2.0, 1e-5));
        assert_eq!(settings.snapped(1.26, 0.5, false), 1.5);
        assert_eq!(settings.snapped(-1.26, 0.5, false), -1.5);
        tool.input.clear();
        tool.tool = Tool::Rotate;
        tool.axis = Some(1);
        let (_, snapped_angle) = tool.calculate(camera, rect, Vec2::new(22.0, 0.0), false);
        assert_eq!(snapped_angle, 15.0);

        let hierarchy = scene.hierarchy().unwrap();
        let worlds = hierarchy.world_matrices(|i| scene.entities[i].transform.matrix());
        for (mode, expected) in [
            (PivotMode::Center, -2.0),
            (PivotMode::Active, -4.0),
            (PivotMode::Individual, 0.0),
        ] {
            let group = TransformGroup::capture(
                &scene.entities,
                &hierarchy,
                &worlds,
                &[a, b_id],
                TransformSettings {
                    pivot: mode,
                    ..Default::default()
                },
            )
            .unwrap();
            let scaled = group.transformed(&group.targets[0], Change::Scale(Vec3::splat(2.0)));
            assert!((scaled.translation.x - expected).abs() < 1e-5);
        }
    }
}
