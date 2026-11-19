use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::{GameInput, InputAction, physics::Physics};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterController {
    pub radius: f32,
    pub height: f32,
    pub origin_height: f32,
    pub speed: f32,
    pub jump_speed: f32,
    pub collision_mask: u32,
}

impl Default for CharacterController {
    fn default() -> Self {
        Self {
            radius: 0.30,
            height: 1.75,
            origin_height: 1.65,
            speed: 4.4,
            jump_speed: 4.7,
            collision_mask: u32::MAX,
        }
    }
}

impl CharacterController {
    pub(crate) fn valid(self) -> bool {
        (0.05..=2.0).contains(&self.radius)
            && (self.radius * 2.0..=8.0).contains(&self.height)
            && (0.0..=8.0).contains(&self.origin_height)
            && (0.1..=50.0).contains(&self.speed)
            && (0.0..=30.0).contains(&self.jump_speed)
    }
}

pub(crate) struct CharacterMotion {
    config: CharacterController,
    grounded: bool,
    vertical: f32,
}

impl CharacterMotion {
    pub(crate) fn new(config: CharacterController) -> Self {
        Self {
            config,
            grounded: false,
            vertical: 0.0,
        }
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::new(self.config);
    }

    pub(crate) fn step(
        &mut self,
        physics: &Physics,
        dt: f32,
        position: Vec3,
        movement: Vec3,
        jump: bool,
    ) -> Vec3 {
        if dt <= 0.0 {
            return position;
        }
        let config = self.config;
        self.vertical = if self.grounded && jump && config.jump_speed > 0.0 {
            config.jump_speed
        } else {
            (self.vertical - 9.81 * dt).max(-36.0)
        };
        let desired = (movement * config.speed + Vec3::Y * self.vertical) * dt;
        let center = position + Vec3::Y * (config.height * 0.5 - config.origin_height);
        let (moved, grounded) = physics.move_capsule(
            dt,
            center,
            [config.radius, config.height],
            desired,
            config.collision_mask,
        );
        self.grounded = grounded && self.vertical <= 0.0;
        if self.grounded || (self.vertical > 0.0 && moved.y + 0.001 < desired.y) {
            self.vertical = 0.0;
        }
        position + moved
    }
}

pub(crate) fn move_direction(input: &GameInput, forward: Vec3) -> Vec3 {
    let planar = Vec3::new(forward.x, 0.0, forward.z)
        .try_normalize()
        .unwrap_or(Vec3::NEG_Z);
    let axis = |positive, negative| {
        i8::from(input.action(positive).held) as f32 - i8::from(input.action(negative).held) as f32
    };
    (planar * axis(InputAction::MoveForward, InputAction::MoveBackward)
        + planar.cross(Vec3::Y) * axis(InputAction::MoveRight, InputAction::MoveLeft))
    .normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Body, CollisionLayers, Motion, physics::STEP};
    use futty_core::Transform;
    use uuid::Uuid;

    #[test]
    fn capsule_lands_jumps_and_stops_at_the_wall_queried_by_layer() {
        let mut physics = Physics::new();
        let floor = Body {
            motion: Motion::Fixed,
            size: Vec3::new(18.0, 0.5, 18.0),
            ..Default::default()
        };
        physics.insert(
            Uuid::new_v4(),
            Transform {
                translation: Vec3::Y * -0.25,
                ..Default::default()
            },
            floor,
        );
        let wall = Uuid::new_v4();
        physics.insert(
            wall,
            Transform {
                translation: Vec3::new(0.0, 1.5, -2.0),
                ..Default::default()
            },
            Body {
                motion: Motion::Fixed,
                size: Vec3::new(4.0, 3.0, 0.2),
                layers: CollisionLayers {
                    layer: 2,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        physics.insert(
            Uuid::new_v4(),
            Transform {
                translation: Vec3::new(0.0, 1.5, -0.75),
                ..Default::default()
            },
            Body {
                motion: Motion::Fixed,
                sensor: true,
                size: Vec3::new(4.0, 3.0, 0.1),
                ..Default::default()
            },
        );
        physics.step();

        let origin = Vec3::Y * 1.65;
        assert!(physics.raycast(origin, Vec3::NEG_Z, 5.0, 1).is_none());
        let hit = physics.raycast(origin, Vec3::NEG_Z * 2.0, 5.0, 2).unwrap();
        assert_eq!(hit.entity, wall);
        assert!((hit.distance - 1.9).abs() < 0.001);

        let mut character = CharacterMotion::new(CharacterController::default());
        let mut position = origin;
        for _ in 0..90 {
            position = character.step(&physics, STEP, position, Vec3::NEG_Z, false);
        }
        assert!(character.grounded);
        assert!((-1.62..-1.55).contains(&position.z));
        let ground = position.y;
        position = character.step(&physics, STEP, position, Vec3::ZERO, true);
        assert!(position.y > ground && !character.grounded);
        for _ in 0..100 {
            position = character.step(&physics, STEP, position, Vec3::ZERO, false);
        }
        assert!(character.grounded && (position.y - ground).abs() < 0.02);
    }
}
