use futty_core::AssetKind;
use glam::Vec2;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{MAGAZINE, ROUND_SECONDS, RangeStatus, ScriptValue};

pub const MAX_HUD_WIDGETS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HudKind {
    Text,
    Image,
    Button,
    Gauge,
}

impl HudKind {
    pub const ALL: [Self; 4] = [Self::Text, Self::Image, Self::Button, Self::Gauge];

    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Image => "Image",
            Self::Button => "Button",
            Self::Gauge => "Gauge",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HudAnchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HudWidget {
    pub id: Uuid,
    pub kind: HudKind,
    pub label: String,
    pub binding: String,
    pub anchor: HudAnchor,
    pub offset: Vec2,
    pub size: Vec2,
    pub color: [u8; 3],
    pub text_size: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<Uuid>,
}

impl HudWidget {
    pub fn new(kind: HudKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            label: kind.label().into(),
            binding: String::new(),
            anchor: HudAnchor::TopLeft,
            offset: Vec2::splat(16.0),
            size: match kind {
                HudKind::Image => Vec2::splat(120.0),
                HudKind::Gauge => Vec2::new(144.0, 16.0),
                _ => Vec2::new(144.0, 54.0),
            },
            color: if kind == HudKind::Image {
                [255; 3]
            } else {
                [108, 239, 206]
            },
            text_size: 21.0,
            image: None,
        }
    }

    pub(crate) fn valid(&self, asset_kind: impl Fn(Uuid) -> Option<AssetKind>) -> bool {
        !self.id.is_nil()
            && self.label.len() <= 128
            && (self.binding.is_empty() || valid_binding(&self.binding))
            && self
                .offset
                .to_array()
                .into_iter()
                .all(|value| (-4096.0..=4096.0).contains(&value))
            && self
                .size
                .to_array()
                .into_iter()
                .all(|value| (4.0..=4096.0).contains(&value))
            && (8.0..=96.0).contains(&self.text_size)
            && self.image.is_none_or(|image| {
                self.kind == HudKind::Image && asset_kind(image) == Some(AssetKind::Texture)
            })
    }
}

pub(crate) fn valid_binding(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
}

pub fn range_hud_value(name: &str, status: Option<RangeStatus>) -> Option<ScriptValue> {
    let remaining = status.map_or(ROUND_SECONDS, |status| status.remaining);
    match name {
        "range.score" => Some(ScriptValue::Text(format!(
            "{:04}",
            status.map_or(0, |status| status.points)
        ))),
        "range.ammo" if status.is_some_and(|status| status.reloading) => {
            Some(ScriptValue::Text("Reloading".into()))
        }
        "range.ammo" => Some(ScriptValue::Text(format!(
            "{:02} / {MAGAZINE:02}",
            status.map_or(MAGAZINE, |status| status.ammo)
        ))),
        "range.time" => Some(ScriptValue::Text(format!(
            "00:{:02}",
            remaining.ceil() as u8
        ))),
        "range.time_fraction" => Some(ScriptValue::Number(f64::from(remaining / ROUND_SECONDS))),
        "range.streak" => status
            .filter(|status| status.streak > 1)
            .map(|status| ScriptValue::Text(format!("{} HITS IN A ROW", status.streak))),
        _ => None,
    }
}
