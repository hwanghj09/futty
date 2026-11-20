use std::collections::BTreeMap;

use futty_runtime::{HudAnchor, HudKind, HudWidget};
use rmcp::schemars;
use serde::{Deserialize, Deserializer};
use uuid::Uuid;

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetInput {
    pub path: String,
    /// Complete override map for this scene, keyed by action name.
    #[serde(default)]
    pub bindings: BTreeMap<String, String>,
    /// Scene sensitivity from 0.1 to 4.0. Omit to use the player's setting.
    pub sensitivity: Option<f32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateHud {
    pub path: String,
    /// Text, Image, Button or Gauge.
    #[schemars(with = "String")]
    pub kind: HudKind,
    pub settings: HudPatch,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateHud {
    pub path: String,
    #[schemars(with = "String")]
    pub widget_id: Uuid,
    pub settings: HudPatch,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct BindHud {
    pub path: String,
    #[schemars(with = "String")]
    pub widget_id: Uuid,
    /// Rhai HUD value key or button action. Empty clears the binding.
    pub binding: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct HudPatch {
    label: Option<String>,
    binding: Option<String>,
    /// TopLeft, TopRight, BottomLeft, BottomRight or Center.
    #[schemars(with = "Option<String>")]
    anchor: Option<HudAnchor>,
    offset: Option<[f32; 2]>,
    size: Option<[f32; 2]>,
    color: Option<[u8; 3]>,
    text_size: Option<f32>,
    /// Registered PNG UUID for an Image widget. Null clears it.
    #[serde(default, deserialize_with = "image_patch")]
    #[schemars(with = "Option<String>")]
    image: Option<Option<Uuid>>,
}

fn image_patch<'de, D: Deserializer<'de>>(input: D) -> Result<Option<Option<Uuid>>, D::Error> {
    Option::deserialize(input).map(Some)
}

impl HudPatch {
    pub fn apply(self, widget: &mut HudWidget) {
        if let Some(label) = self.label {
            widget.label = label;
        }
        if let Some(binding) = self.binding {
            widget.binding = binding;
        }
        widget.anchor = self.anchor.unwrap_or(widget.anchor);
        widget.offset = self.offset.map_or(widget.offset, Into::into);
        widget.size = self.size.map_or(widget.size, Into::into);
        widget.color = self.color.unwrap_or(widget.color);
        widget.text_size = self.text_size.unwrap_or(widget.text_size);
        widget.image = self.image.unwrap_or(widget.image);
    }
}
