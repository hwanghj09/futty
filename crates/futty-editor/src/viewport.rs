use crate::{
    selection::Selection,
    transform_tool::{TransformGroup, TransformSettings},
};
use eframe::{egui, egui_wgpu::RenderState, wgpu};
use egui::{Color32, Pos2, Rect, Vec2};
use futty_core::{Asset, AssetKind, Camera, Primitive};
use futty_render::{RenderScene, Renderer, Targets};
use futty_runtime::{Body, Hierarchy, Object, Sprite};
use glam::{Mat4, Vec3};
use uuid::Uuid;

pub const AXES: [(&str, Color32); 3] = [
    ("X", Color32::from_rgb(212, 138, 139)),
    ("Y", Color32::from_rgb(152, 190, 151)),
    ("Z", Color32::from_rgb(137, 167, 213)),
];

const DEFAULT_ORBIT_DISTANCE: f32 = 8.0;

pub struct Viewport {
    pub camera: Camera,
    pub speed: f32,
    pub grid: bool,
    pub colliders: bool,
    pub flew_with_right_button: bool,
    pub transform_settings: TransformSettings,
    orbit_distance: f32,
    orbiting: bool,
    gpu: RenderState,
    renderer: Renderer,
    views: [(Targets, egui::TextureId); 2],
    drag: Option<AxisDrag>,
}

struct AxisDrag {
    group: TransformGroup,
    start: Pos2,
    direction: Vec3,
    pixels_per_unit: Vec2,
}

impl Viewport {
    pub fn new(gpu: RenderState) -> Self {
        let renderer = Renderer::new(&gpu.device, &gpu.queue);
        let views = std::array::from_fn(|_| {
            let targets = Targets::new(&gpu.device, [1, 1]);
            let image = gpu.renderer.write().register_native_texture(
                &gpu.device,
                &targets.color,
                wgpu::FilterMode::Linear,
            );
            (targets, image)
        });
        Self {
            camera: Camera::default(),
            speed: 4.0,
            grid: true,
            colliders: true,
            flew_with_right_button: false,
            transform_settings: TransformSettings::default(),
            orbit_distance: DEFAULT_ORBIT_DISTANCE,
            orbiting: false,
            gpu,
            renderer,
            views,
            drag: None,
        }
    }

    pub fn clear_assets(&mut self) {
        self.renderer.clear_assets();
    }

    pub fn upload(&mut self, asset: &Asset, bytes: &[u8]) -> futty_assets::AssetResult<()> {
        match asset.kind {
            AssetKind::Model => self.renderer.upload_model(
                &self.gpu.device,
                &self.gpu.queue,
                asset.id,
                futty_assets::model(bytes)?,
            ),
            AssetKind::Texture => {
                let image = futty_assets::png(bytes)?;
                self.renderer.upload_image(
                    &self.gpu.device,
                    &self.gpu.queue,
                    asset.id,
                    [image.width(), image.height()],
                    image.as_raw(),
                );
            }
            _ => return Err("Expected a mesh or texture".into()),
        }
        Ok(())
    }

    pub fn bounds(&self, mesh: Option<Uuid>) -> [Vec3; 2] {
        self.renderer.bounds(mesh, Primitive::Cube)
    }

    pub fn object_bounds(&self, object: &Object) -> [Vec3; 2] {
        if !object.has_renderer() {
            [Vec3::splat(-0.2), Vec3::splat(0.2)]
        } else if object.sprite.is_some() {
            Sprite::BOUNDS
        } else {
            self.renderer.bounds(object.mesh, object.primitive)
        }
    }

    pub fn texture_size(&self, id: Uuid) -> Option<[u32; 2]> {
        self.renderer.texture_size(id)
    }

    pub fn render(&mut self, ui: &egui::Ui, rect: Rect, mut scene: RenderScene<'_>, game: bool) {
        let max = self.gpu.device.limits().max_texture_dimension_2d;
        let scale = ui.pixels_per_point();
        let size =
            [rect.width(), rect.height()].map(|side| ((side * scale).round() as u32).clamp(1, max));
        let (targets, image) = &mut self.views[usize::from(game)];
        if targets.size != size {
            *targets = Targets::new(&self.gpu.device, size);
            self.gpu
                .renderer
                .write()
                .update_egui_texture_from_wgpu_texture(
                    &self.gpu.device,
                    &targets.color,
                    wgpu::FilterMode::Linear,
                    *image,
                );
        }
        scene.grid = self.grid && !game;
        self.renderer
            .draw(&self.gpu.device, &self.gpu.queue, targets, scene);
        ui.painter().image(
            *image,
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    pub fn reset_camera(&mut self) {
        self.camera = Camera::default();
        self.orbit_distance = DEFAULT_ORBIT_DISTANCE;
    }

    pub fn frame(&mut self, target: Vec3, distance: f32) {
        self.camera.eye = target - self.camera.forward() * distance;
        self.orbit_distance = distance;
    }

    pub fn navigate_scene(&mut self, response: &egui::Response) -> bool {
        let (pressed, down, alt) = response.ctx.input(|input| {
            (
                input.pointer.primary_pressed(),
                input.pointer.primary_down(),
                input.modifiers.alt,
            )
        });
        if pressed {
            self.orbiting = response.is_pointer_button_down_on() && alt;
        }
        let orbiting = self.orbiting;
        if orbiting {
            if response.dragged_by(egui::PointerButton::Primary) {
                orbit(&mut self.camera, self.orbit_distance, response.drag_delta());
            }
            self.orbiting = down;
        } else {
            let previous = self.camera;
            self.camera = self.navigate(response, previous);
            if response.hovered()
                && response.ctx.input(|input| {
                    !input.pointer.secondary_down()
                        && !input.pointer.middle_down()
                        && input.smooth_scroll_delta.y != 0.0
                })
            {
                let dolly = (self.camera.eye - previous.eye).dot(previous.forward());
                let distance = (self.orbit_distance - dolly).max(0.1);
                self.camera.eye =
                    previous.eye + previous.forward() * (self.orbit_distance - distance);
                self.orbit_distance = distance;
            }
        }
        orbiting
    }

    pub fn navigate(&mut self, response: &egui::Response, mut camera: Camera) -> Camera {
        if response.is_pointer_button_down_on()
            && response
                .ctx
                .input(|input| input.pointer.button_pressed(egui::PointerButton::Secondary))
        {
            self.flew_with_right_button = false;
        }
        let looking = response.is_pointer_button_down_on()
            && response.ctx.input(|input| input.pointer.secondary_down());
        if response.dragged_by(egui::PointerButton::Middle) {
            let delta = response.drag_delta();
            camera.eye += (camera.right() * -delta.x + Vec3::Y * delta.y) * 0.012;
        }
        if response.hovered() {
            let scroll = response.ctx.input(|input| input.smooth_scroll_delta.y);
            if looking {
                self.speed = (self.speed * (scroll * 0.005).exp()).clamp(0.5, 45.0);
            } else {
                camera.eye += camera.forward() * (scroll * 0.014);
            }
        }
        if looking {
            let delta = response.drag_delta();
            turn(&mut camera, delta);
            let (motion, dt) = response.ctx.input(|input| {
                let axis = |positive, negative| {
                    input.key_down(positive) as i32 as f32 - input.key_down(negative) as i32 as f32
                };
                (
                    Vec3::new(
                        axis(egui::Key::D, egui::Key::A),
                        axis(egui::Key::E, egui::Key::Q),
                        axis(egui::Key::W, egui::Key::S),
                    ),
                    input.stable_dt.min(0.05),
                )
            });
            if motion != Vec3::ZERO {
                self.flew_with_right_button = true;
                let direction =
                    camera.right() * motion.x + Vec3::Y * motion.y + camera.forward() * motion.z;
                camera.eye += direction.normalize_or_zero() * self.speed * dt;
                response.ctx.request_repaint();
            }
        }
        camera
    }

    pub fn select_and_move(
        &mut self,
        response: &egui::Response,
        objects: &mut [Object],
        hierarchy: &Hierarchy,
        selection: &mut Selection,
        move_tool: bool,
    ) {
        let rect = response.rect;
        let vp = self.camera.view_projection(rect.aspect_ratio());
        let world = hierarchy.world_matrices(|index| objects[index].transform.matrix());
        let dragging = response.dragged_by(egui::PointerButton::Primary);
        let released = response.drag_stopped_by(egui::PointerButton::Primary);
        if !dragging && !released {
            self.drag = None;
        }
        let pointer = response.interact_pointer_pos();
        let modifiers = response.ctx.input(|input| input.modifiers);
        if move_tool
            && !modifiers.command
            && !modifiers.shift
            && response.drag_started_by(egui::PointerButton::Primary)
            && let Some(pos) = response.ctx.input(|input| input.pointer.press_origin())
            && let Some(group) = TransformGroup::capture(
                objects,
                hierarchy,
                &world,
                selection.ids(),
                self.transform_settings,
            )
            && let Some((origin, ends, length)) = gizmo(vp, rect, self.camera, &group)
        {
            let closest = ends
                .iter()
                .enumerate()
                .filter(|(_, end)| end.distance(origin) >= 8.0)
                .map(|(axis, &end)| (axis, segment_distance(pos, origin.lerp(end, 0.25), end)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((axis, distance)) = closest
                && distance < 9.0
            {
                self.drag = Some(AxisDrag {
                    start: pos,
                    direction: group.axis_direction(axis, false),
                    pixels_per_unit: (ends[axis] - origin) / length,
                    group,
                });
            }
        }
        if let Some(drag) = &self.drag
            && let Some(pointer) = pointer
        {
            let delta =
                (pointer - drag.start).dot(drag.pixels_per_unit) / drag.pixels_per_unit.length_sq();
            let settings = drag.group.settings;
            let delta = settings.snapped(delta, settings.move_step, modifiers.ctrl);
            drag.group.move_objects(objects, drag.direction * delta);
            response.ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        if released {
            self.drag = None;
        }
        if response.clicked_by(egui::PointerButton::Primary)
            && response.ctx.current_pass_index() == 0
            && let Some(pos) = pointer
        {
            selection.click(self.pick_object(rect, pos, objects, world), modifiers, &[]);
        }
    }

    pub fn pick_object(
        &self,
        rect: Rect,
        pos: Pos2,
        objects: &[Object],
        world: Vec<Mat4>,
    ) -> Option<Uuid> {
        let vp = self.camera.view_projection(rect.aspect_ratio());
        let ndc = Vec3::new(
            2.0 * (pos.x - rect.left()) / rect.width() - 1.0,
            1.0 - 2.0 * (pos.y - rect.top()) / rect.height(),
            0.0,
        );
        let point = vp.inverse().project_point3(ndc);
        let direction = (point - self.camera.eye).normalize();
        objects
            .iter()
            .zip(world)
            .filter_map(|(object, matrix)| {
                if !object.has_renderer() {
                    let eye = matrix.w_axis.truncate();
                    return marker(vp, rect, eye)
                        .filter(|marker| marker.contains(pos))
                        .map(|_| (object.uuid, (eye - self.camera.eye).dot(direction)));
                }
                let matrix = object
                    .sprite
                    .map_or(matrix, |sprite| sprite.matrix(matrix, self.camera.view()));
                ray_hit(
                    self.camera.eye,
                    direction,
                    matrix,
                    self.object_bounds(object),
                )
                .map(|t| (object.uuid, t))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    pub fn paint_gizmo(&self, ui: &egui::Ui, rect: Rect, group: &TransformGroup) {
        let vp = self.camera.view_projection(rect.aspect_ratio());
        let Some((origin, ends, _)) = gizmo(vp, rect, self.camera, group) else {
            return;
        };
        let painter = ui.painter().with_clip_rect(rect);
        for ((label, color), end) in AXES.into_iter().zip(ends) {
            if end.distance(origin) < 8.0 {
                continue;
            }
            let axis = (end - origin).normalized();
            let base = end - axis * 9.0;
            let edge = axis.rot90() * 3.7;
            painter.line_segment([origin, base], (3.5, Color32::from_gray(24)));
            painter.line_segment([origin, base], (1.4, color));
            painter.add(egui::Shape::convex_polygon(
                vec![end, base + edge, base - edge],
                color,
                (1.0, Color32::from_gray(24)),
            ));
            painter.text(
                end + axis * 10.0,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::monospace(12.0),
                color,
            );
        }
    }
}

fn turn(camera: &mut Camera, delta: Vec2) {
    camera.yaw += delta.x * 0.005;
    camera.pitch = (camera.pitch - delta.y * 0.005).clamp(-1.52, 1.52);
}

fn orbit(camera: &mut Camera, distance: f32, delta: Vec2) {
    let pivot = camera.eye + camera.forward() * distance;
    turn(camera, delta);
    camera.eye = pivot - camera.forward() * distance;
}

pub fn paint_collider(ui: &egui::Ui, rect: Rect, camera: Camera, world: Mat4, body: Body) {
    let vp = camera.view_projection(rect.aspect_ratio());
    let corners: [_; 8] = std::array::from_fn(|index| {
        let side = |bit| if index & bit == 0 { -0.5 } else { 0.5 };
        let point = body.center + body.size * Vec3::new(side(1), side(2), side(4));
        project(vp, rect, world.transform_point3(point))
    });
    let color = if body.sensor {
        crate::WARNING
    } else {
        crate::FOCUS
    };
    let painter = ui.painter().with_clip_rect(rect);
    for (start, point) in corners.iter().enumerate() {
        for bit in [1, 2, 4] {
            if start & bit == 0
                && let (Some(a), Some(b)) = (*point, corners[start | bit])
            {
                painter.line_segment([a, b], (1.1, color));
            }
        }
    }
}

pub(crate) fn project(vp: Mat4, rect: Rect, point: Vec3) -> Option<Pos2> {
    let clip = vp * point.extend(1.0);
    if !clip.is_finite() || clip.w <= 0.05 {
        return None;
    }
    let ndc = clip.truncate() / clip.w;
    Some(rect.center() + Vec2::new(ndc.x, -ndc.y) * rect.size() * 0.5)
}

pub(crate) fn marker(vp: Mat4, rect: Rect, world: Vec3) -> Option<Rect> {
    project(vp, rect, world)
        .filter(|&center| rect.contains(center))
        .map(|center| Rect::from_center_size(center, Vec2::splat(26.0)))
}

fn gizmo(
    vp: Mat4,
    rect: Rect,
    camera: Camera,
    group: &TransformGroup,
) -> Option<(Pos2, [Pos2; 3], f32)> {
    let position = group.pivot;
    let project = |point| project(vp, rect, point);
    let length = (position - camera.eye).dot(camera.forward()) * 0.15;
    let origin = project(position)?;
    Some((
        origin,
        [
            project(position + group.axis_direction(0, false) * length)?,
            project(position + group.axis_direction(1, false) * length)?,
            project(position + group.axis_direction(2, false) * length)?,
        ],
        length,
    ))
}

fn segment_distance(point: Pos2, start: Pos2, end: Pos2) -> f32 {
    let vector = end - start;
    let t = ((point - start).dot(vector) / vector.length_sq()).clamp(0.0, 1.0);
    point.distance(start + vector * t)
}

fn ray_hit(origin: Vec3, direction: Vec3, matrix: Mat4, [min, max]: [Vec3; 2]) -> Option<f32> {
    let inverse = matrix.inverse();
    if !inverse.is_finite() {
        return None;
    }
    let origin = inverse.transform_point3(origin);
    let direction = inverse.transform_vector3(direction);
    let (mut start, mut end) = (0.0_f32, f32::INFINITY);
    for axis in 0..3 {
        if direction[axis].abs() < 1.0e-8 {
            if origin[axis] < min[axis] || origin[axis] > max[axis] {
                return None;
            }
            continue;
        }
        let a = (min[axis] - origin[axis]) / direction[axis];
        let b = (max[axis] - origin[axis]) / direction[axis];
        start = start.max(a.min(b));
        end = end.min(a.max(b));
        if end < start {
            return None;
        }
    }
    Some(start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futty_core::Transform;
    use glam::Quat;

    #[test]
    fn orbit_keeps_the_same_pivot_and_distance() {
        let mut camera = Camera::default();
        let distance = 3.5;
        let start = camera.eye;
        let pivot = start + camera.forward() * distance;
        orbit(&mut camera, distance, Vec2::new(110.0, -700.0));
        assert_ne!(camera.eye, start);
        assert!((camera.eye + camera.forward() * distance).abs_diff_eq(pivot, 1e-5));
        assert!((camera.eye.distance(pivot) - distance).abs() < 1e-5);
        assert_eq!(camera.pitch, 1.52);
    }

    #[test]
    fn picking_respects_object_transform() {
        let transform = Transform {
            translation: Vec3::new(1.0, 0.0, 2.0),
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
            scale: Vec3::new(2.0, 1.0, 1.0),
        };
        let cube = [Vec3::splat(-0.5), Vec3::splat(0.5)];
        let distance = ray_hit(
            Vec3::new(1.0, 0.0, 6.0),
            Vec3::NEG_Z,
            transform.matrix(),
            cube,
        )
        .unwrap();
        assert!((distance - 3.0).abs() < 1e-4);
        assert!(
            ray_hit(
                Vec3::new(3.0, 0.0, 6.0),
                Vec3::NEG_Z,
                transform.matrix(),
                cube
            )
            .is_none()
        );
    }
}
