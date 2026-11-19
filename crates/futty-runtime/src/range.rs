use futty_core::{Camera, Projection, Transform};
use glam::{Quat, Vec2, Vec3};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{GameInput, Object, Scene, SceneResult};

pub const ROUND_SECONDS: f32 = 45.0;
pub const MAGAZINE: u8 = 8;
const SHOT_INTERVAL: f32 = 0.18;
const RELOAD_SECONDS: f32 = 0.72;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RangeConfig {
    pub weapon: Uuid,
    pub flash: Uuid,
    pub drones: Vec<Uuid>,
}

impl RangeConfig {
    fn actors<'a>(
        &self,
        scene: &'a Scene,
    ) -> SceneResult<(&'a Object, &'a Object, &'a Object, Vec<&'a Object>)> {
        let camera = scene.camera_index(None).map(|i| &scene.entities[i]);
        let find = |id| {
            scene.entities.iter().find(|object| {
                object.uuid == id
                    && object.has_renderer()
                    && object.body.is_none()
                    && object.script.is_none()
            })
        };
        let Some(camera) = camera.filter(|object| {
            object.parent.is_none()
                && object.body.is_none()
                && object.script.is_none()
                && object
                    .camera
                    .is_some_and(|lens| lens.projection == Projection::Perspective)
        }) else {
            return Err("Training Range needs an unparented perspective camera".into());
        };
        let weapon = find(self.weapon)
            .filter(|object| object.parent == Some(camera.uuid))
            .ok_or("Training Range needs a weapon parented to its camera")?;
        let flash = find(self.flash)
            .filter(|object| object.parent == Some(weapon.uuid))
            .ok_or("Training Range needs a flash parented to its weapon")?;
        if self.drones.is_empty() || self.drones.len() > 16 {
            return Err("Training Range needs 1 to 16 drones".into());
        }
        let mut drones = Vec::with_capacity(self.drones.len());
        for &id in &self.drones {
            let drone = find(id)
                .filter(|object| object.parent.is_none() && !drones.contains(object))
                .ok_or("Training Range drones must be distinct root objects")?;
            drones.push(drone);
        }
        Ok((camera, weapon, flash, drones))
    }

    pub(crate) fn validate(&self, scene: &Scene) -> SceneResult<()> {
        self.actors(scene).map(|_| ())
    }
}

#[derive(Clone, Copy, Default)]
pub struct RangeInput {
    pub active: bool,
    pub look: Vec2,
    pub fire: bool,
    pub reload: bool,
    pub retry: bool,
}

#[derive(Clone, Copy)]
pub struct RangeStatus {
    pub remaining: f32,
    pub ammo: u8,
    pub reloading: bool,
    pub points: u32,
    pub hits: u32,
    pub shots: u32,
    pub streak: u32,
    pub hit_flash: bool,
}

struct Actor {
    id: Uuid,
    base: Transform,
    pose: Transform,
}

impl From<&Object> for Actor {
    fn from(object: &Object) -> Self {
        Self {
            id: object.uuid,
            base: object.transform,
            pose: object.transform,
        }
    }
}

struct Drone {
    actor: Actor,
    respawn: f32,
}

pub(crate) struct Range {
    camera: Actor,
    weapon: Actor,
    flash: Actor,
    drones: Vec<Drone>,
    view: Camera,
    status: RangeStatus,
    cooldown: f32,
    reload: f32,
    marker: f32,
}

impl Range {
    pub(crate) fn new(scene: &Scene, config: &RangeConfig) -> SceneResult<Self> {
        let (camera, weapon, flash, drones) = config.actors(scene)?;
        let mut range = Self {
            camera: camera.into(),
            weapon: weapon.into(),
            flash: flash.into(),
            drones: drones
                .into_iter()
                .map(|object| Drone {
                    actor: object.into(),
                    respawn: 0.0,
                })
                .collect(),
            view: Camera::default(),
            status: Self::initial_status(),
            cooldown: 0.0,
            reload: 0.0,
            marker: 0.0,
        };
        range.reset();
        Ok(range)
    }

    fn initial_status() -> RangeStatus {
        RangeStatus {
            remaining: ROUND_SECONDS,
            ammo: MAGAZINE,
            reloading: false,
            points: 0,
            hits: 0,
            shots: 0,
            streak: 0,
            hit_flash: false,
        }
    }

    fn reset(&mut self) {
        let forward = self.camera.base.rotation * Vec3::NEG_Z;
        self.view = Camera {
            eye: self.camera.base.translation,
            yaw: forward.x.atan2(-forward.z),
            pitch: forward.y.clamp(-1.0, 1.0).asin(),
        };
        self.status = Self::initial_status();
        self.cooldown = 0.0;
        self.reload = 0.0;
        self.marker = 0.0;
        self.camera.pose = self.camera.base;
        self.weapon.pose = self.weapon.base;
        self.flash.pose.translation.y = -50.0;
        for drone in &mut self.drones {
            drone.actor.pose = drone.actor.base;
            drone.respawn = 0.0;
        }
    }

    pub(crate) fn status(&self) -> RangeStatus {
        self.status
    }

    pub(crate) fn pose(&self, id: Uuid) -> Option<Transform> {
        [&self.camera, &self.weapon, &self.flash]
            .into_iter()
            .chain(self.drones.iter().map(|drone| &drone.actor))
            .find(|actor| actor.id == id)
            .map(|actor| actor.pose)
    }

    pub(crate) fn update(&mut self, dt: f32, input: &GameInput) -> [Option<Uuid>; 2] {
        let controls = input.range;
        if !controls.active {
            return [None; 2];
        }
        if self.status.remaining <= 0.0 {
            if controls.retry {
                self.reset();
            }
            return [None; 2];
        }
        self.move_player(dt, input);
        self.status.remaining = (self.status.remaining - dt).max(0.0);
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.marker = (self.marker - dt).max(0.0);
        if self.reload > 0.0 {
            self.reload = (self.reload - dt).max(0.0);
            if self.reload == 0.0 {
                self.status.ammo = MAGAZINE;
            }
        }
        if controls.reload && self.status.ammo < MAGAZINE && self.reload == 0.0 {
            self.reload = RELOAD_SECONDS;
        }
        self.move_drones(dt);
        let mut sounds = [None; 2];
        if controls.fire
            && self.cooldown == 0.0
            && self.reload == 0.0
            && self.status.remaining > 0.0
        {
            if self.status.ammo == 0 {
                self.reload = RELOAD_SECONDS;
            } else {
                sounds = [Some(self.weapon.id), self.shoot()];
            }
        }
        self.status.reloading = self.reload > 0.0;
        self.status.hit_flash = self.marker > 0.0;
        let recoil = (self.cooldown / SHOT_INTERVAL).powi(2);
        let reload_dip = (self.reload / RELOAD_SECONDS * std::f32::consts::PI).sin();
        self.weapon.pose.translation =
            self.weapon.base.translation + Vec3::new(0.0, -0.24 * reload_dip, 0.12 * recoil);
        self.weapon.pose.rotation =
            self.weapon.base.rotation * Quat::from_rotation_x(0.16 * recoil - 0.4 * reload_dip);
        self.flash.pose = self.flash.base;
        if self.cooldown < SHOT_INTERVAL - 0.055 {
            self.flash.pose.translation.y = -50.0;
        }
        sounds
    }

    fn move_player(&mut self, dt: f32, input: &GameInput) {
        if input.range.look.is_finite() {
            self.view.yaw =
                (self.view.yaw + input.range.look.x * 0.0024).rem_euclid(std::f32::consts::TAU);
            self.view.pitch = (self.view.pitch - input.range.look.y * 0.0024).clamp(-1.4, 1.4);
        }
        let axis =
            |positive: bool, negative: bool| i8::from(positive) as f32 - i8::from(negative) as f32;
        let flat = Vec3::new(self.view.yaw.sin(), 0.0, -self.view.yaw.cos());
        let movement = flat * axis(input.forward, input.backward)
            + flat.cross(Vec3::Y) * axis(input.right, input.left);
        let center = self.camera.base.translation;
        // shortcut: movement follows the rectangular firing deck, use scene colliders for other layouts.
        self.view.eye = (self.view.eye + movement.normalize_or_zero() * dt * 4.4).clamp(
            center + Vec3::new(-6.35, 0.0, -4.75),
            center + Vec3::new(6.35, 0.0, 1.35),
        );
        self.camera.pose.translation = self.view.eye;
        self.camera.pose.rotation = self.view.view().rotation;
    }

    fn move_drones(&mut self, dt: f32) {
        let elapsed = ROUND_SECONDS - self.status.remaining;
        for (index, drone) in self.drones.iter_mut().enumerate() {
            drone.respawn = (drone.respawn - dt).max(0.0);
            let phase = index as f32 * 1.29;
            let speed = 0.85 + elapsed * 0.014 + (index % 3) as f32 * 0.12;
            drone.actor.pose.translation = drone.actor.base.translation
                + Vec3::new(
                    ((elapsed * speed + phase).sin() - phase.sin()) * 0.84,
                    ((elapsed * 1.45 + phase).sin() - phase.sin()) * 0.38,
                    (elapsed * 0.8 + phase).sin() * 0.24,
                );
            if drone.respawn > 0.0 {
                drone.actor.pose.translation.y = -30.0;
            }
            drone.actor.pose.rotation = drone.actor.base.rotation
                * Quat::from_rotation_z((elapsed * 2.0 + phase).sin() * 0.13);
        }
    }

    fn shoot(&mut self) -> Option<Uuid> {
        self.status.ammo -= 1;
        self.status.shots += 1;
        self.cooldown = SHOT_INTERVAL;
        let direction = self.view.forward();
        // shortcut: the firing lanes are open, test scene colliders when adding cover.
        let nearest = self
            .drones
            .iter_mut()
            .filter(|drone| drone.respawn <= 0.0)
            .filter_map(|drone| {
                let radius = drone.actor.pose.scale.max_element() * 0.51;
                ray_distance(
                    self.view.eye,
                    direction,
                    drone.actor.pose.translation,
                    radius,
                )
                .map(|distance| (distance, drone))
            })
            .min_by(|left, right| left.0.total_cmp(&right.0));
        let Some((_, drone)) = nearest else {
            self.status.streak = 0;
            return None;
        };
        self.status.hits += 1;
        self.status.streak += 1;
        self.status.points += 100 + 25 * (self.status.streak.min(5) - 1);
        self.marker = 0.19;
        drone.respawn = 0.95;
        drone.actor.pose.translation.y = -30.0;
        Some(drone.actor.id)
    }
}

fn ray_distance(origin: Vec3, direction: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let offset = center - origin;
    let along = offset.dot(direction);
    let perpendicular = (offset - along * direction).length_squared();
    let intersection = radius * radius - perpendicular;
    (along >= 0.0 && intersection >= 0.0).then(|| (along - intersection.sqrt()).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Runtime;

    #[test]
    fn round_pauses_hits_nearest_target_reloads_and_restarts() {
        let mut scene = Scene::default();
        let mut camera = Object::cube("Camera".into(), Vec3::Y * 1.65);
        camera.camera = Some(futty_core::CameraLens::default());
        scene.main_camera = Some(camera.uuid);
        let mut weapon = Object::cube("Blaster".into(), Vec3::ZERO);
        weapon.parent = scene.main_camera;
        let mut flash = Object::cube("Pulse".into(), Vec3::ZERO);
        flash.parent = Some(weapon.uuid);
        let distant = Object::cube("Far".into(), Vec3::new(0.0, 1.65, -10.0));
        let near = Object::cube("Near".into(), Vec3::new(0.0, 1.65, -5.0));
        let near_id = near.uuid;
        scene.training_range = Some(RangeConfig {
            weapon: weapon.uuid,
            flash: flash.uuid,
            drones: vec![distant.uuid, near_id],
        });
        scene.entities = vec![camera, weapon, flash, distant, near];
        let mut runtime = Runtime::new(&scene).unwrap();
        runtime.tick(0.1);
        assert_eq!(runtime.range_status().unwrap().remaining, ROUND_SECONDS);

        runtime.game_input(GameInput {
            range: RangeInput {
                active: true,
                fire: true,
                ..Default::default()
            },
            ..Default::default()
        });
        runtime.tick(0.016);
        let status = runtime.range_status().unwrap();
        assert_eq!(
            (status.points, status.hits, status.shots, status.ammo),
            (100, 1, 1, MAGAZINE - 1)
        );
        assert!(
            runtime
                .extract()
                .any(|(id, pose, _)| id == near_id && pose.translation.y < 0.0)
        );
        assert_eq!(scene.entities[4].transform.translation.y, 1.65);

        runtime.game_input(GameInput {
            range: RangeInput {
                active: true,
                reload: true,
                ..Default::default()
            },
            ..Default::default()
        });
        runtime.tick(0.1);
        assert!(runtime.range_status().unwrap().reloading);
        for _ in 0..8 {
            runtime.tick(0.1);
        }
        assert_eq!(runtime.range_status().unwrap().ammo, MAGAZINE);
        assert!(
            runtime
                .extract()
                .any(|(id, pose, _)| id == near_id && pose.translation.y < 0.0)
        );
        for _ in 0..460 {
            runtime.tick(0.1);
        }
        assert_eq!(runtime.range_status().unwrap().remaining, 0.0);
        runtime.game_input(GameInput {
            range: RangeInput {
                active: true,
                retry: true,
                ..Default::default()
            },
            ..Default::default()
        });
        runtime.tick(0.016);
        let status = runtime.range_status().unwrap();
        assert_eq!(status.remaining, ROUND_SECONDS);
        assert_eq!((status.points, status.ammo), (0, MAGAZINE));
    }
}
