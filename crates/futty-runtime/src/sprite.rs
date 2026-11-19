use futty_core::CameraView;
use glam::{Mat4, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Billboard {
    #[default]
    FaceCamera,
    Upright,
    Fixed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use futty_core::Camera;

    #[test]
    fn billboard_modes_keep_the_center_size_and_expected_orientation() {
        let mut camera = Camera::default();
        let center = Vec3::new(1.0, 1.5, -0.5);
        let world = Mat4::from_translation(center)
            * Mat4::from_rotation_y(0.4)
            * Mat4::from_scale(Vec3::new(2.0, 3.0, 1.0));
        let mut sprite = Sprite {
            texture: Uuid::new_v4(),
            size: Vec2::new(2.0, 1.0),
            billboard: Billboard::FaceCamera,
        };
        let screen_facing = sprite.matrix(world, camera.view());
        assert_eq!(screen_facing.w_axis.truncate(), center);
        assert!((screen_facing.x_axis.length() - 4.0).abs() < 1e-5);
        assert!((screen_facing.y_axis.length() - 3.0).abs() < 1e-5);
        assert!(
            screen_facing
                .z_axis
                .truncate()
                .abs_diff_eq(-camera.forward(), 1e-5)
        );
        sprite.billboard = Billboard::Upright;
        let upright = sprite.matrix(world, camera.view());
        assert!(upright.y_axis.truncate().abs_diff_eq(Vec3::Y * 3.0, 1e-5));
        assert_eq!(upright.z_axis.y, 0.0);
        camera.eye = center + Vec3::Y;
        assert!(sprite.matrix(world, camera.view()).is_finite());
        sprite.billboard = Billboard::Fixed;
        assert_eq!(
            sprite.matrix(world, camera.view()),
            world * Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0))
        );
    }
}

impl Billboard {
    pub fn label(self) -> &'static str {
        match self {
            Self::FaceCamera => "Face camera",
            Self::Upright => "Upright",
            Self::Fixed => "Fixed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sprite {
    pub texture: Uuid,
    pub size: Vec2,
    pub billboard: Billboard,
}

impl Sprite {
    pub const BOUNDS: [Vec3; 2] = [Vec3::new(-0.5, -0.5, 0.0), Vec3::new(0.5, 0.5, 0.0)];

    pub fn matrix(self, world: Mat4, camera: CameraView) -> Mat4 {
        if self.billboard == Billboard::Fixed {
            return world * Mat4::from_scale(self.size.extend(1.0));
        }
        let toward_camera = -camera.forward();
        let (right, up, normal) = if self.billboard == Billboard::Upright {
            let offset = camera.eye - world.w_axis.truncate();
            let normal = Vec3::new(offset.x, 0.0, offset.z)
                .try_normalize()
                .or_else(|| Vec3::new(toward_camera.x, 0.0, toward_camera.z).try_normalize())
                .unwrap_or(Vec3::Z);
            (Vec3::Y.cross(normal), Vec3::Y, normal)
        } else {
            (camera.right(), camera.up(), toward_camera)
        };
        let width = self.size.x * world.x_axis.truncate().length();
        let height = self.size.y * world.y_axis.truncate().length();
        Mat4::from_cols(
            (right * width).extend(0.0),
            (up * height).extend(0.0),
            normal.extend(0.0),
            world.w_axis,
        )
    }

    pub(crate) fn valid(self) -> bool {
        self.size.is_finite()
            && self.size.min_element() >= 0.01
            && self.size.max_element() <= 1000.0
    }
}
