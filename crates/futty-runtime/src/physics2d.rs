use std::{collections::VecDeque, sync::mpsc};

use futty_core::Transform;
use glam::{EulerRot, Quat, Vec3};
use rapier2d::{control::KinematicCharacterController, prelude::*};
use uuid::Uuid;

use super::{Body, Motion, PhysicsEvent, RayHit, STEP};

fn planar_pose(transform: Transform) -> Pose {
    let (angle, _, _) = transform.rotation.to_euler(EulerRot::ZYX);
    Pose::from_parts(
        transform.translation.truncate(),
        Rotation::from_angle(angle),
    )
}

pub(crate) struct PlanarPhysics {
    world: PhysicsWorld,
    collector: ChannelEventCollector,
    incoming: mpsc::Receiver<CollisionEvent>,
    events: VecDeque<PhysicsEvent>,
    recent: Vec<PhysicsEvent>,
    prepared: bool,
}

impl PlanarPhysics {
    pub(crate) fn new() -> Self {
        let (sender, incoming) = mpsc::channel();
        let world = PhysicsWorld {
            gravity: Vector::new(0.0, -9.81),
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
            prepared: false,
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
        .pose(planar_pose(transform));
        let half = config.size * transform.scale * 0.5;
        let collider = ColliderBuilder::cuboid(half.x, half.y)
            .translation((config.center * transform.scale).truncate())
            .mass(config.mass)
            .friction(config.friction)
            .restitution(config.restitution)
            .collision_groups(InteractionGroups::new(
                Group::from_bits_truncate(1 << (config.layers.layer - 1)),
                Group::from_bits_truncate(config.layers.mask),
                InteractionTestMode::And,
            ))
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
        self.world.bodies[handle].set_next_kinematic_position(planar_pose(target));
    }

    pub(crate) fn apply_commands(
        &mut self,
        handle: RigidBodyHandle,
        commands: &crate::script::Commands,
    ) {
        let body = &mut self.world.bodies[handle];
        if let Some(velocity) = commands.velocity {
            body.set_linvel(velocity.truncate(), true);
        }
        if commands.impulse != Vec3::ZERO {
            body.apply_impulse(commands.impulse.truncate(), true);
        }
    }

    pub(crate) fn prepare_queries(&mut self) {
        self.world.detect_collisions(&(), &self.collector);
        // The collision-only pass consumes modifications before the solver can register active bodies.
        self.world.bodies.iter_mut().for_each(drop);
        self.prepared = true;
    }

    pub(crate) fn step(&mut self) {
        self.recent.clear();
        self.world.step_with_events(&(), &self.collector);
        self.prepared = true;
        for event in self.incoming.try_iter() {
            let (Some(first), Some(second)) = (
                self.world.colliders.get(event.collider1()),
                self.world.colliders.get(event.collider2()),
            ) else {
                continue;
            };
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
        transform.translation = body.translation().extend(transform.translation.z);
        transform.rotation = Quat::from_rotation_z(body.rotation().angle());
    }

    pub(crate) fn count(&self) -> usize {
        self.world.bodies.len()
    }

    pub(crate) fn remove(&mut self, body: RigidBodyHandle) {
        self.world.remove_body(body);
    }

    pub(crate) fn set_enabled(&mut self, body: RigidBodyHandle, enabled: bool) {
        self.world.bodies[body].set_enabled(enabled);
    }

    pub(crate) fn events(&self) -> &VecDeque<PhysicsEvent> {
        &self.events
    }

    pub(crate) fn raycast(
        &self,
        origin: Vec3,
        direction: Vec3,
        distance: f32,
        mask: u32,
    ) -> Option<RayHit> {
        if !origin.is_finite() || !distance.is_finite() || distance <= 0.0 || mask == 0 {
            return None;
        }
        let direction = direction.truncate().try_normalize()?;
        let ray = Ray::new(origin.truncate(), direction);
        let (collider, impact) =
            self.world
                .cast_ray_and_get_normal(&ray, distance, true, query_filter(mask))?;
        Some(RayHit {
            entity: Uuid::from_u128(self.world.colliders[collider].user_data),
            distance: impact.time_of_impact,
            position: origin + direction.extend(0.0) * impact.time_of_impact,
            normal: impact.normal.extend(0.0),
        })
    }

    pub(crate) fn move_capsule(
        &self,
        dt: f32,
        center: Vec3,
        size: [f32; 2],
        delta: Vec3,
        mask: u32,
    ) -> (Vec3, bool) {
        if self.count() > 0 && !self.prepared {
            return (Vec3::ZERO, false);
        }
        let [radius, height] = size;
        let capsule = Capsule::new_y(height * 0.5 - radius, radius);
        let controller = KinematicCharacterController {
            // Leave enough separation for the next cast when the capsule touches a wall and floor.
            normal_nudge_factor: 0.001,
            ..Default::default()
        };
        let result = controller.move_shape(
            dt,
            &self.world.query_pipeline_with_filter(query_filter(mask)),
            &capsule,
            &Pose::from_translation(center.truncate()),
            delta.truncate(),
            |_| {},
        );
        (result.translation.extend(0.0), result.grounded)
    }
}

fn query_filter(mask: u32) -> QueryFilter<'static> {
    QueryFilter::default()
        .exclude_sensors()
        .groups(InteractionGroups::new(
            Group::ALL,
            Group::from_bits_truncate(mask),
            InteractionTestMode::And,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planar_body_lands_and_triggers_across_render_depths() {
        let mut physics = PlanarPhysics::new();
        let floor = Uuid::new_v4();
        let sensor = Uuid::new_v4();
        let falling = Uuid::new_v4();
        physics.insert(
            floor,
            Transform {
                translation: Vec3::new(0.0, -0.25, -6.0),
                ..Transform::default()
            },
            Body {
                motion: Motion::Fixed,
                size: Vec3::new(6.0, 0.5, 0.5),
                ..Body::default()
            },
        );
        physics.insert(
            sensor,
            Transform {
                translation: Vec3::new(0.0, 1.5, 8.0),
                ..Transform::default()
            },
            Body {
                motion: Motion::Fixed,
                size: Vec3::new(4.0, 0.5, 0.5),
                sensor: true,
                ..Body::default()
            },
        );
        let mut pose = Transform {
            translation: Vec3::new(0.0, 3.0, 2.0),
            ..Transform::default()
        };
        let body = physics.insert(falling, pose, Body::default());
        physics.prepare_queries();
        for _ in 0..120 {
            physics.step();
        }
        physics.pose(body, &mut pose);
        assert_eq!(pose.translation.z, 2.0);
        assert!((0.45..0.55).contains(&pose.translation.y), "{pose:?}");
        assert!(physics.events().iter().any(|event| event.started
            && event.sensor
            && event.first == sensor
            && event.second == falling));
        let ray = physics
            .raycast(Vec3::new(2.0, 3.0, 2.0), Vec3::NEG_Y, 5.0, u32::MAX)
            .unwrap();
        assert_eq!(ray.entity, floor);
        assert_eq!(ray.position.z, 2.0);
    }
}
