use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Primitive {
    #[default]
    Cube,
    Sphere,
    Cylinder,
    Cone,
    Plane,
}

impl Primitive {
    pub const ALL: [Self; 5] = [
        Self::Cube,
        Self::Sphere,
        Self::Cylinder,
        Self::Cone,
        Self::Plane,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Cube => "Cube",
            Self::Sphere => "Sphere",
            Self::Cylinder => "Cylinder",
            Self::Cone => "Cone",
            Self::Plane => "Plane",
        }
    }

    pub fn bounds(self) -> [Vec3; 2] {
        let extent = if self == Self::Plane {
            Vec3::new(0.5, 0.0, 0.5)
        } else {
            Vec3::splat(0.5)
        };
        [-extent, extent]
    }

    pub fn is_cube(&self) -> bool {
        *self == Self::Cube
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetKind {
    Model,
    Texture,
    Script,
    Wav,
    Ogg,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LightKind {
    Directional,
    Point,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Surface {
    pub roughness: f32,
    pub emission: [f32; 3],
    pub emission_strength: f32,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            roughness: 0.7,
            emission: [0.0; 3],
            emission_strength: 0.0,
        }
    }
}

impl Surface {
    pub fn valid(self) -> bool {
        (0.0..=1.0).contains(&self.roughness)
            && self
                .emission
                .iter()
                .all(|value| (0.0..=1.0).contains(value))
            && (0.0..=10.0).contains(&self.emission_strength)
    }
}

impl LightKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Directional => "Directional Light",
            Self::Point => "Point Light",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Light {
    pub kind: LightKind,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    #[serde(default)]
    pub cast_shadows: bool,
    #[serde(default = "default_shadow_distance")]
    pub shadow_distance: f32,
}

fn default_shadow_distance() -> f32 {
    24.0
}

impl Light {
    pub fn new(kind: LightKind) -> Self {
        Self {
            kind,
            color: [1.0; 3],
            intensity: if kind == LightKind::Point { 3.0 } else { 0.75 },
            range: 8.0,
            cast_shadows: kind == LightKind::Directional,
            shadow_distance: default_shadow_distance(),
        }
    }

    pub fn valid(self) -> bool {
        self.color.iter().all(|value| (0.0..=1.0).contains(value))
            && (0.0..=10.0).contains(&self.intensity)
            && (0.1..=250.0).contains(&self.range)
            && (2.0..=100.0).contains(&self.shadow_distance)
    }
}

impl AssetKind {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Model => "glb",
            Self::Texture => "png",
            Self::Script => "rhai",
            Self::Wav => "wav",
            Self::Ogg => "ogg",
        }
    }

    pub fn is_audio(self) -> bool {
        matches!(self, Self::Wav | Self::Ogg)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub id: Uuid,
    pub name: String,
    pub kind: AssetKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn matrix(self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }

    pub fn from_matrix(matrix: Mat4) -> Option<Self> {
        if !matrix.is_finite() {
            return None;
        }
        let (scale, rotation, translation) = matrix.to_scale_rotation_translation();
        let transform = Self {
            scale,
            rotation,
            translation,
        };
        let tolerance = matrix
            .to_cols_array()
            .into_iter()
            .take(12)
            .map(f32::abs)
            .fold(1.0, f32::max)
            * 0.0001;
        (rotation.is_normalized()
            && scale.is_finite()
            && scale.min_element() >= 0.01
            && transform.matrix().abs_diff_eq(matrix, tolerance))
        .then_some(transform)
    }
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Camera {
    pub eye: Vec3,
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for Camera {
    fn default() -> Self {
        let eye = Vec3::new(4.5, 3.0, 6.5);
        let mut camera = Self {
            eye,
            yaw: 0.0,
            pitch: 0.0,
        };
        camera.look_at(Vec3::new(0.0, 0.7, 0.0));
        camera
    }
}

impl Camera {
    pub fn look_at(&mut self, target: Vec3) {
        let direction = (target - self.eye).normalize();
        self.yaw = direction.x.atan2(-direction.z);
        self.pitch = direction.y.asin();
    }

    pub fn forward(self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        Vec3::new(sin_yaw * cos_pitch, sin_pitch, -cos_yaw * cos_pitch)
    }

    pub fn right(self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    pub fn view_projection(self, aspect: f32) -> Mat4 {
        self.view().view_projection(aspect)
    }

    pub fn view(self) -> CameraView {
        CameraView {
            eye: self.eye,
            rotation: Quat::from_rotation_y(-self.yaw) * Quat::from_rotation_x(self.pitch),
            lens: CameraLens::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Projection {
    #[default]
    Perspective,
    Orthographic,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraLens {
    pub projection: Projection,
    pub fov: f32,
    pub size: f32,
    pub near: f32,
    pub far: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pixels_per_unit: Option<u16>,
}

impl Default for CameraLens {
    fn default() -> Self {
        Self {
            projection: Projection::Perspective,
            fov: 50.0,
            size: 5.0,
            near: 0.05,
            far: 250.0,
            pixels_per_unit: None,
        }
    }
}

impl CameraLens {
    pub fn valid(self) -> bool {
        (5.0..=160.0).contains(&self.fov)
            && (0.05..=500.0).contains(&self.size)
            && (0.01..=9_999.0).contains(&self.near)
            && self.far > self.near
            && self.far <= 10_000.0
            && self
                .pixels_per_unit
                .is_none_or(|pixels| (1..=1024).contains(&pixels))
    }

    pub fn half_height(self, distance: f32) -> f32 {
        match self.projection {
            Projection::Perspective => distance * (self.fov.to_radians() * 0.5).tan(),
            Projection::Orthographic => self.size,
        }
    }
}

#[derive(Clone, Copy)]
pub struct CameraView {
    pub eye: Vec3,
    pub rotation: Quat,
    pub lens: CameraLens,
}

impl CameraView {
    pub fn pixel_aligned(mut self, size: [u32; 2]) -> Self {
        let Some(pixels) = self
            .lens
            .pixels_per_unit
            .filter(|_| self.lens.projection == Projection::Orthographic)
        else {
            return self;
        };
        let pixels = f32::from(pixels);
        let height = size[1].max(1) as f32;
        let scale = (height / (2.0 * self.lens.size * pixels)).round().max(1.0);
        self.lens.size = height / (2.0 * pixels * scale);
        self.eye = self.snap_to_pixel_grid(self.eye);
        // At odd framebuffer dimensions its center is half a display pixel from an edge.
        self.eye += (self.right() * (size[0] % 2) as f32 - self.up() * (size[1] % 2) as f32)
            / (2.0 * pixels * scale);
        self
    }

    pub fn snap_to_pixel_grid(self, position: Vec3) -> Vec3 {
        let Some(pixels) = self.lens.pixels_per_unit else {
            return position;
        };
        let pixels = f32::from(pixels);
        let [right, up] = [self.right(), self.up()];
        let correction = |axis: Vec3| {
            let coordinate = position.dot(axis);
            axis * ((coordinate * pixels).round() / pixels - coordinate)
        };
        position + correction(right) + correction(up)
    }

    pub fn forward(self) -> Vec3 {
        self.rotation * Vec3::NEG_Z
    }
    pub fn right(self) -> Vec3 {
        self.rotation * Vec3::X
    }
    pub fn up(self) -> Vec3 {
        self.rotation * Vec3::Y
    }

    pub fn view_projection(self, aspect: f32) -> Mat4 {
        use glam::camera::rh::proj::directx;
        let lens = self.lens;
        let projection = match lens.projection {
            Projection::Perspective => {
                directx::perspective(lens.fov.to_radians(), aspect, lens.near, lens.far)
            }
            Projection::Orthographic => {
                let half_width = lens.size * aspect;
                directx::orthographic(
                    -half_width,
                    half_width,
                    -lens.size,
                    lens.size,
                    lens.near,
                    lens.far,
                )
            }
        };
        projection * Mat4::from_quat(self.rotation.conjugate()) * Mat4::from_translation(-self.eye)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_centers_its_target() {
        let camera = Camera::default();
        let point = camera.view_projection(1.5).project_point3(Vec3::Y * 0.7);
        assert!(point.x.abs() < 1e-5 && point.y.abs() < 1e-5);
        assert!((0.0..1.0).contains(&point.z));
    }

    #[test]
    fn pixel_camera_uses_integer_zoom_and_aligns_the_odd_framebuffer() {
        let view = CameraView {
            eye: Vec3::new(0.14, 0.28, 10.0),
            rotation: Quat::from_rotation_z(0.3),
            lens: CameraLens {
                projection: Projection::Orthographic,
                size: 2.2,
                pixels_per_unit: Some(16),
                ..CameraLens::default()
            },
        };
        let size = [321, 183];
        let aligned = view.pixel_aligned(size);
        let matrix = aligned.view_projection(size[0] as f32 / size[1] as f32);
        let screen = |point| {
            let projected = matrix.project_point3(point);
            glam::Vec2::new((projected.x + 1.0) * 160.5, (1.0 - projected.y) * 91.5)
        };
        let origin = screen(Vec3::ZERO);
        assert!(origin.abs_diff_eq(origin.round(), 1e-4));
        assert!((screen(view.right() / 16.0).x - origin.x - 3.0).abs() < 1e-4);
        assert!((origin.y - screen(view.up() / 16.0).y - 3.0).abs() < 1e-4);
        assert_eq!(aligned.eye.z, 10.0);
        assert_eq!(
            aligned.snap_to_pixel_grid(Vec3::new(0.001, 0.002, 3.0)).z,
            3.0
        );
    }

    #[test]
    fn orthographic_camera_preserves_size_with_depth_and_honors_roll() {
        let view = CameraView {
            eye: Vec3::ZERO,
            rotation: Quat::from_rotation_z(0.5),
            lens: CameraLens {
                projection: Projection::Orthographic,
                near: 1.0,
                far: 11.0,
                ..Default::default()
            },
        };
        let vp = view.view_projection(2.0);
        let offset = view.right() * 5.0;
        let near = vp.project_point3(offset + view.forward());
        let far = vp.project_point3(offset + view.forward() * 11.0);
        assert!(near.abs_diff_eq(Vec3::new(0.5, 0.0, 0.0), 1e-5));
        assert!(far.abs_diff_eq(Vec3::new(0.5, 0.0, 1.0), 1e-5));
    }
}
