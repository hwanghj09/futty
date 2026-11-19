use std::{collections::VecDeque, sync::mpsc};

use futty_core::Transform;
use glam::{Quat, Vec3};
use rapier3d::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) const STEP: f32 = 1.0 / 60.0;
pub(crate) const MAX_STEPS: usize = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Motion {
    #[default]
    Dynamic,
    Fixed,
    Kinematic,
}

impl Motion {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dynamic => "Dynamic",
            Self::Fixed => "Fixed",
            Self::Kinematic => "Kinematic",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Body {
    pub motion: Motion,
    pub size: Vec3,
    pub center: Vec3,
    pub mass: f32,
    pub gravity_scale: f32,
    pub friction: f32,
    pub restitution: f32,
    pub sensor: bool,
}

impl Default for Body {
    fn default() -> Self {
        Self {
            motion: Motion::Dynamic,
            size: Vec3::ONE,
            center: Vec3::ZERO,
            mass: 1.0,
            gravity_scale: 1.0,
            friction: 0.6,
            restitution: 0.1,
            sensor: false,
        }
    }
}

impl Body {
    pub(crate) fn valid(self, transform: Transform) -> bool {
        let dimensions = self.size * transform.scale;
        let center = self.center * transform.scale;
        self.size.is_finite()
            && self.size.min_element() >= 0.01
            && dimensions.is_finite()
            && dimensions.max_element() <= 20_000.0
            && center.is_finite()
            && center.abs().max_element() <= 20_000.0
            && transform.translation.abs().max_element() <= 100_000.0
            && (0.01..=1000.0).contains(&self.mass)
            && (0.0..=10.0).contains(&self.gravity_scale)
            && (0.0..=5.0).contains(&self.friction)
            && (0.0..=1.0).contains(&self.restitution)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PhysicsEvent {
    pub first: Uuid,
    pub second: Uuid,
    pub started: bool,
    pub sensor: bool,
}

impl PhysicsEvent {
    pub fn label(self) -> &'static str {
        match (self.sensor, self.started) {
            (true, true) => "Trigger entered",
            (true, false) => "Trigger exited",
            (false, true) => "Collision started",
            (false, false) => "Collision ended",
        }
    }
}

pub(crate) struct Physics {
    world: PhysicsWorld,
    collector: ChannelEventCollector,
    incoming: mpsc::Receiver<CollisionEvent>,
    events: VecDeque<PhysicsEvent>,
    recent: Vec<PhysicsEvent>,
}

impl Physics {
    pub(crate) fn new() -> Self {
        let (sender, incoming) = mpsc::channel();
        let world = PhysicsWorld {
            gravity: Vector::new(0.0, -9.81, 0.0),
            integration_parameters: IntegrationParameters {
                dt: STEP,
                ..Default::default()
            },
            ..Default::default()
        };
        Self {
            world,
            collector: ChannelEventCollector::new(sender, mpsc::channel().0, mpsc::channel().0),
            incoming,
            events: VecDeque::new(),
            recent: Vec::new(),
        }
    }

    pub(crate) fn insert(
        &mut self,
        id: Uuid,
        transform: Transform,
        config: Body,
    ) -> RigidBodyHandle {
        let body = match config.motion {
            Motion::Dynamic => RigidBodyBuilder::dynamic().ccd_enabled(true),
            Motion::Fixed => RigidBodyBuilder::fixed(),
            Motion::Kinematic => RigidBodyBuilder::kinematic_position_based(),
        }
        .gravity_scale(config.gravity_scale)
        .pose(Pose::from_parts(
            Vector::from_array(transform.translation.to_array()),
            Rotation::from_array(transform.rotation.to_array()),
        ));
        let half = config.size * transform.scale * 0.5;
        let collider = ColliderBuilder::cuboid(half.x, half.y, half.z)
            .translation(Vector::from_array(
                (config.center * transform.scale).to_array(),
            ))
            .mass(config.mass)
            .friction(config.friction)
            .restitution(config.restitution)
            .sensor(config.sensor)
            .user_data(id.as_u128())
            .active_events(ActiveEvents::COLLISION_EVENTS)
            .active_collision_types(if config.sensor {
                ActiveCollisionTypes::all()
            } else {
                ActiveCollisionTypes::default()
            });
        self.world.insert(body, collider).0
    }

    pub(crate) fn move_kinematic(&mut self, handle: RigidBodyHandle, target: Transform) {
        self.world.bodies[handle].set_next_kinematic_position(Pose::from_parts(
            Vector::from_array(target.translation.to_array()),
            Rotation::from_array(target.rotation.to_array()),
        ));
    }

    pub(crate) fn apply_commands(
        &mut self,
        handle: RigidBodyHandle,
        commands: &crate::script::Commands,
    ) {
        let body = &mut self.world.bodies[handle];
        if let Some(velocity) = commands.velocity {
            body.set_linvel(Vector::from_array(velocity.to_array()), true);
        }
        if commands.impulse != Vec3::ZERO {
            body.apply_impulse(Vector::from_array(commands.impulse.to_array()), true);
        }
    }

    pub(crate) fn step(&mut self) {
        self.recent.clear();
        self.world.step_with_events(&(), &self.collector);
        for event in self.incoming.try_iter() {
            let first = &self.world.colliders[event.collider1()];
            let second = &self.world.colliders[event.collider2()];
            let (first, second) = if second.is_sensor() && !first.is_sensor() {
                (second, first)
            } else {
                (first, second)
            };
            let event = PhysicsEvent {
                first: Uuid::from_u128(first.user_data),
                second: Uuid::from_u128(second.user_data),
                started: event.started(),
                sensor: event.sensor(),
            };
            self.events.push_back(event);
            self.recent.push(event);
            if self.events.len() > 24 {
                self.events.pop_front();
            }
        }
    }

    pub(crate) fn recent(&self) -> &[PhysicsEvent] {
        &self.recent
    }

    pub(crate) fn pose(&self, handle: RigidBodyHandle, transform: &mut Transform) {
        let body = &self.world.bodies[handle];
        transform.translation = Vec3::from_array(body.translation().to_array());
        transform.rotation = Quat::from_array(body.rotation().to_array());
    }

    pub(crate) fn count(&self) -> usize {
        self.world.bodies.len()
    }

    pub(crate) fn events(&self) -> &VecDeque<PhysicsEvent> {
        &self.events
    }
}
