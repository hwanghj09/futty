use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::{GameInput, InputAction, physics::Physics};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterMode {
    #[default]
    Spatial,
    SideScroller,
    #[serde(rename = "top_down_2d")]
    TopDown2d,
}

impl CharacterMode {
    pub fn is_spatial(&self) -> bool {
        *self == Self::Spatial
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterController {
    pub radius: f32,
    pub height: f32,
    pub origin_height: f32,
    pub speed: f32,
    pub jump_speed: f32,
    #[serde(default, skip_serializing_if = "zero_seconds")]
    pub coyote_time: f32,
    #[serde(default, skip_serializing_if = "zero_seconds")]
    pub jump_buffer: f32,
    pub collision_mask: u32,
    #[serde(default, skip_serializing_if = "CharacterMode::is_spatial")]
    pub mode: CharacterMode,
}

fn zero_seconds(seconds: &f32) -> bool {
    *seconds == 0.0
}

impl Default for CharacterController {
    fn default() -> Self {
        Self {
            radius: 0.30,
            height: 1.75,
            origin_height: 1.65,
            speed: 4.4,
            jump_speed: 4.7,
            coyote_time: 0.0,
            jump_buffer: 0.0,
            collision_mask: u32::MAX,
            mode: CharacterMode::Spatial,
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
            && (0.0..=0.25).contains(&self.coyote_time)
            && (0.0..=0.25).contains(&self.jump_buffer)
    }
}

pub(crate) struct CharacterMotion {
    config: CharacterController,
    grounded: bool,
    vertical: f32,
    coyote_remaining: f32,
    jump_remaining: f32,
}

impl CharacterMotion {
    pub(crate) fn new(config: CharacterController) -> Self {
        Self {
            config,
            grounded: false,
            vertical: 0.0,
            coyote_remaining: 0.0,
            jump_remaining: 0.0,
        }
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::new(self.config);
    }

    pub(crate) fn direction(&self, input: &GameInput, forward: Vec3) -> Vec3 {
        let x = input_axis(input, InputAction::MoveRight, InputAction::MoveLeft);
        match self.config.mode {
            CharacterMode::Spatial => move_direction(input, forward),
            CharacterMode::SideScroller => Vec3::X * x,
            CharacterMode::TopDown2d => Vec3::new(
                x,
                input_axis(input, InputAction::MoveForward, InputAction::MoveBackward),
                0.0,
            )
            .normalize_or_zero(),
        }
    }

    pub(crate) fn is_spatial(&self) -> bool {
        self.config.mode.is_spatial()
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
        if config.mode == CharacterMode::SideScroller {
            self.coyote_remaining = if self.grounded {
                config.coyote_time
            } else {
                (self.coyote_remaining - dt).max(0.0)
            };
            self.jump_remaining = if jump {
                config.jump_buffer
            } else {
                (self.jump_remaining - dt).max(0.0)
            };
        }
        self.vertical = if config.mode == CharacterMode::TopDown2d {
            0.0
        } else if (self.grounded || self.coyote_remaining > 0.0)
            && (jump || self.jump_remaining > 0.0)
            && config.jump_speed > 0.0
        {
            self.coyote_remaining = 0.0;
            self.jump_remaining = 0.0;
            config.jump_speed
        } else {
            (self.vertical - 9.81 * dt).max(-36.0)
        };
        let desired = (movement * config.speed + Vec3::Y * self.vertical) * dt;
        let center = if config.mode == CharacterMode::TopDown2d {
            position
        } else {
            position + Vec3::Y * (config.height * 0.5 - config.origin_height)
        };
        let (mut moved, grounded) = physics.move_capsule(
            dt,
            center,
            [config.radius, config.height],
            desired,
            config.collision_mask,
        );
        if !config.mode.is_spatial() {
            moved.z = 0.0;
        }
        self.grounded = grounded && self.vertical <= 0.0;
        if self.grounded || (self.vertical > 0.0 && moved.y + 0.001 < desired.y) {
            self.vertical = 0.0;
        }
        position + moved
    }
}

fn input_axis(input: &GameInput, positive: InputAction, negative: InputAction) -> f32 {
    f32::from(u8::from(input.action(positive).held))
        - f32::from(u8::from(input.action(negative).held))
}

pub(crate) fn move_direction(input: &GameInput, forward: Vec3) -> Vec3 {
    let planar = Vec3::new(forward.x, 0.0, forward.z)
        .try_normalize()
        .unwrap_or(Vec3::NEG_Z);
    (planar * input_axis(input, InputAction::MoveForward, InputAction::MoveBackward)
        + planar.cross(Vec3::Y) * input_axis(input, InputAction::MoveRight, InputAction::MoveLeft))
    .normalize_or_zero()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Body, CollisionLayers, Motion, physics::STEP};
    use futty_core::Transform;
    use uuid::Uuid;

    #[test]
    fn side_scroller_accepts_early_and_late_presses_once() {
        let mut physics = Physics::new();
        physics.insert(
            Uuid::new_v4(),
            Transform {
                translation: Vec3::Y * -0.25,
                ..Default::default()
            },
            Body {
                motion: Motion::Fixed,
                size: Vec3::new(2.0, 0.5, 2.0),
                ..Default::default()
            },
        );
        physics.step();
        let mut character = CharacterMotion::new(CharacterController {
            mode: CharacterMode::SideScroller,
            coyote_time: 0.12,
            jump_buffer: 0.12,
            ..CharacterController::default()
        });
        let mut position = Vec3::Y * 1.69;
        position = character.step(&physics, STEP, position, Vec3::ZERO, true);
        assert!(character.vertical <= 0.0 && character.jump_remaining > 0.0);
        for _ in 0..10 {
            position = character.step(&physics, STEP, position, Vec3::ZERO, false);
            if character.vertical > 0.0 {
                break;
            }
        }
        assert!(
            character.vertical > 0.0,
            "early jump should start on landing"
        );
        assert_eq!(character.jump_remaining, 0.0);
        for _ in 0..100 {
            position = character.step(&physics, STEP, position, Vec3::ZERO, false);
        }
        assert!(character.grounded);
        for _ in 0..30 {
            position = character.step(&physics, STEP, position, Vec3::X, false);
            if !character.grounded {
                break;
            }
        }
        assert!(!character.grounded, "must step off the ledge first");
        let jumped = character.step(&physics, STEP, position, Vec3::X, true);
        assert!(
            jumped.y > position.y,
            "late jump should start after leaving the ledge"
        );
        assert_eq!(character.coyote_remaining, 0.0);
    }

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
