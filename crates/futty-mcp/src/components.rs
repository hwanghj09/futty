use futty_core::{Light, LightKind};
use futty_runtime::{Body, CharacterController, Motion, Object};
use rmcp::{ErrorData, schemars};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::authoring::invalid;

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ComponentKind {
    Light,
    Body,
    Character,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ComponentQuery {
    pub path: String,
    #[schemars(with = "String")]
    pub object_id: Uuid,
    pub component: ComponentKind,
}

impl ComponentKind {
    pub fn read(self, object: &Object) -> Value {
        match self {
            Self::Light => json!(object.light),
            Self::Body => json!(object.body),
            Self::Character => json!(object.character),
        }
    }

    pub fn remove(self, object: &mut Object) -> Result<(), ErrorData> {
        check_editable(object)?;
        match self {
            Self::Light => object.light = None,
            Self::Body => object.body = None,
            Self::Character => object.character = None,
        }
        Ok(())
    }
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) enum LightType {
    Directional,
    Point,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub(crate) enum BodyMotion {
    Dynamic,
    Fixed,
    Kinematic,
}

/// Omitted settings keep the current value, or use defaults when adding a component.
#[derive(Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum ComponentPatch {
    Light {
        light_type: Option<LightType>,
        color: Option<[f32; 3]>,
        intensity: Option<f32>,
        range: Option<f32>,
        cast_shadows: Option<bool>,
        shadow_distance: Option<f32>,
    },
    Body {
        motion: Option<BodyMotion>,
        size: Option<[f32; 3]>,
        center: Option<[f32; 3]>,
        mass: Option<f32>,
        gravity_scale: Option<f32>,
        friction: Option<f32>,
        restitution: Option<f32>,
        sensor: Option<bool>,
        collision_layer: Option<u8>,
        collision_mask: Option<u32>,
    },
    Character {
        radius: Option<f32>,
        height: Option<f32>,
        origin_height: Option<f32>,
        speed: Option<f32>,
        jump_speed: Option<f32>,
        collision_mask: Option<u32>,
    },
}

impl ComponentPatch {
    pub fn apply(self, object: &mut Object) -> Result<(), ErrorData> {
        check_editable(object)?;
        match self {
            Self::Light {
                light_type,
                color,
                intensity,
                range,
                cast_shadows,
                shadow_distance,
            } => {
                let kind = match light_type {
                    Some(LightType::Directional) => LightKind::Directional,
                    Some(LightType::Point) => LightKind::Point,
                    None => object.light.map_or(LightKind::Point, |light| light.kind),
                };
                let current = object.light.unwrap_or_else(|| Light::new(kind));
                object.light = Some(Light {
                    kind,
                    color: color.unwrap_or(current.color),
                    intensity: intensity.unwrap_or(current.intensity),
                    range: range.unwrap_or(current.range),
                    cast_shadows: cast_shadows.unwrap_or(current.cast_shadows),
                    shadow_distance: shadow_distance.unwrap_or(current.shadow_distance),
                });
            }
            Self::Body {
                motion,
                size,
                center,
                mass,
                gravity_scale,
                friction,
                restitution,
                sensor,
                collision_layer,
                collision_mask,
            } => {
                let current = object.body.unwrap_or_else(|| Body {
                    motion: Motion::Fixed,
                    ..Body::default()
                });
                let motion = match motion {
                    Some(BodyMotion::Dynamic) => Motion::Dynamic,
                    Some(BodyMotion::Fixed) => Motion::Fixed,
                    Some(BodyMotion::Kinematic) => Motion::Kinematic,
                    None => current.motion,
                };
                object.body = Some(Body {
                    motion,
                    size: size.map_or(current.size, Into::into),
                    center: center.map_or(current.center, Into::into),
                    mass: mass.unwrap_or(current.mass),
                    gravity_scale: gravity_scale.unwrap_or(current.gravity_scale),
                    friction: friction.unwrap_or(current.friction),
                    restitution: restitution.unwrap_or(current.restitution),
                    sensor: sensor.unwrap_or(current.sensor),
                    layers: futty_runtime::CollisionLayers {
                        layer: collision_layer.unwrap_or(current.layers.layer),
                        mask: collision_mask.unwrap_or(current.layers.mask),
                    },
                });
            }
            Self::Character {
                radius,
                height,
                origin_height,
                speed,
                jump_speed,
                collision_mask,
            } => {
                let current = object.character.unwrap_or_default();
                object.character = Some(CharacterController {
                    radius: radius.unwrap_or(current.radius),
                    height: height.unwrap_or(current.height),
                    origin_height: origin_height.unwrap_or(current.origin_height),
                    speed: speed.unwrap_or(current.speed),
                    jump_speed: jump_speed.unwrap_or(current.jump_speed),
                    collision_mask: collision_mask.unwrap_or(current.collision_mask),
                });
            }
        }
        Ok(())
    }
}

fn check_editable(object: &Object) -> Result<(), ErrorData> {
    if object.prefab.is_some() {
        return Err(invalid(
            "Edit the prefab source object to change its components",
        ));
    }
    Ok(())
}
