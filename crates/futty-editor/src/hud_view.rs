use std::collections::HashMap;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Vec2, vec2};
use futty_core::AssetKind;
use futty_runtime::{
    HudAnchor, HudKind, HudWidget, MAX_HUD_WIDGETS, Runtime, Scene, ScriptValue, range_hud_value,
};
use uuid::Uuid;

use crate::{Editor, viewport::Viewport};

const PANEL: Color32 = Color32::from_rgba_premultiplied(12, 22, 29, 234);

fn scale(canvas: Rect) -> f32 {
    (canvas.width() / 640.0)
        .min(canvas.height() / 360.0)
        .clamp(0.1, 1.0)
}

fn bounds(canvas: Rect, widget: &HudWidget) -> Rect {
    let fraction = match widget.anchor {
        HudAnchor::TopLeft => Vec2::ZERO,
        HudAnchor::TopRight => Vec2::X,
        HudAnchor::BottomLeft => Vec2::Y,
        HudAnchor::BottomRight => Vec2::splat(1.0),
        HudAnchor::Center => Vec2::splat(0.5),
    };
    let inward = |value| if value == 1.0 { -1.0 } else { 1.0 };
    let direction = vec2(inward(fraction.x), inward(fraction.y));
    let unit = scale(canvas);
    let size = vec2(widget.size.x, widget.size.y) * unit;
    let offset = vec2(widget.offset.x, widget.offset.y) * unit;
    Rect::from_min_size(
        canvas.min + fraction * (canvas.size() - size) + direction * offset,
        size,
    )
}

pub(crate) fn over_button(ctx: &egui::Context, canvas: Rect, hud: &[HudWidget]) -> bool {
    ctx.input(|input| {
        input.pointer.hover_pos().is_some_and(|pos| {
            canvas.contains(pos)
                && hud.iter().any(|widget| {
                    widget.kind == HudKind::Button
                        && !widget.binding.is_empty()
                        && bounds(canvas, widget).contains(pos)
                })
        })
    })
}

pub(crate) fn paint(
    ui: &mut egui::Ui,
    canvas: Rect,
    scene: &Scene,
    viewport: &mut Viewport,
    assets: Option<&HashMap<Uuid, Vec<u8>>>,
    mut runtime: Option<&mut Runtime>,
    interactive: bool,
) {
    let unit = scale(canvas);
    for widget in &scene.hud {
        let rect = bounds(canvas, widget);
        let clip = rect.intersect(canvas).intersect(ui.clip_rect());
        if !clip.is_positive() {
            continue;
        }
        let painter = ui.painter().with_clip_rect(clip);
        let [r, g, b] = widget.color;
        let accent = Color32::from_rgb(r, g, b);
        let font = FontId::monospace(widget.text_size * unit);
        let value = runtime
            .as_deref()
            .and_then(|runtime| runtime.hud_value(&widget.binding))
            .or_else(|| {
                (runtime.is_none() && scene.training_range.is_some())
                    .then(|| range_hud_value(&widget.binding, None))
                    .flatten()
            });
        match widget.kind {
            HudKind::Text => {
                if widget.label.is_empty() && value.is_none() {
                    continue;
                }
                painter.rect_filled(rect, 6.0 * unit, PANEL);
                if widget.binding.is_empty() {
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        &widget.label,
                        font,
                        accent,
                    );
                } else {
                    let text = match value {
                        Some(ScriptValue::Text(text)) => text,
                        Some(ScriptValue::Number(value)) => format!("{value:.2}")
                            .trim_end_matches('0')
                            .trim_end_matches('.')
                            .to_owned(),
                        Some(ScriptValue::Bool(value)) => value.to_string(),
                        None => "–".into(),
                    };
                    if widget.label.is_empty() {
                        painter.text(rect.center(), Align2::CENTER_CENTER, text, font, accent);
                    } else {
                        painter.text(
                            rect.min + vec2(11.0, 7.0) * unit,
                            Align2::LEFT_TOP,
                            &widget.label,
                            FontId::monospace(11.0 * unit),
                            accent,
                        );
                        painter.text(
                            rect.min + vec2(11.0, 22.0) * unit,
                            Align2::LEFT_TOP,
                            text,
                            font,
                            Color32::WHITE,
                        );
                    }
                }
            }
            HudKind::Image => {
                if let Some((texture, dimensions)) = widget.image.and_then(|id| {
                    viewport.hud_image(
                        ui.ctx(),
                        id,
                        assets.and_then(|assets| assets.get(&id)).map(Vec::as_slice),
                    )
                }) {
                    let ratio = (rect.size() / dimensions).min_elem();
                    let fitted = Rect::from_center_size(rect.center(), dimensions * ratio);
                    painter.image(
                        texture,
                        fitted,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        accent,
                    );
                } else if runtime.is_none() {
                    painter.rect_filled(rect, 4.0 * unit, PANEL);
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        "Choose a PNG",
                        FontId::monospace(12.0 * unit),
                        accent,
                    );
                }
            }
            HudKind::Button => {
                if interactive
                    && !widget.binding.is_empty()
                    && let Some(runtime) = runtime.as_deref_mut()
                {
                    let button = egui::Button::new(
                        egui::RichText::new(&widget.label).font(font).color(accent),
                    )
                    .fill(PANEL)
                    .truncate();
                    if ui
                        .push_id(widget.id, |ui| ui.put(clip, button))
                        .inner
                        .clicked()
                        && ui.ctx().current_pass_index() == 0
                    {
                        runtime.click_hud(&widget.binding);
                    }
                } else {
                    painter.rect_filled(rect, 6.0 * unit, PANEL);
                    painter.text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        &widget.label,
                        font,
                        accent,
                    );
                }
            }
            HudKind::Gauge => {
                let progress = match value {
                    Some(ScriptValue::Number(value)) => value.clamp(0.0, 1.0) as f32,
                    _ => 0.0,
                };
                let corner = (rect.height() * 0.5).min(4.0 * unit);
                painter.rect_filled(rect, corner, PANEL);
                if progress > 0.0 {
                    let fill =
                        Rect::from_min_size(rect.min, vec2(rect.width() * progress, rect.height()));
                    painter.rect_filled(fill, corner, accent);
                }
            }
        }
    }
    let second = scene.local_player_two();
    if let Some(runtime) = runtime {
        let players = [scene.player_index(), second];
        let painter = ui
            .painter()
            .with_clip_rect(canvas.intersect(ui.clip_rect()));
        for (slot, index) in players.into_iter().enumerate() {
            let Some((_, mut prompt)) = index
                .and_then(|index| runtime.interaction_focus(scene.entities[index].uuid))
                .filter(|(_, prompt)| !prompt.is_empty())
            else {
                continue;
            };
            if second.is_some() {
                prompt = format!("P{} · {prompt}", slot + 1);
            }
            let label =
                painter.layout_no_wrap(prompt, FontId::monospace(15.0 * unit), Color32::WHITE);
            let position = Pos2::new(
                canvas.center().x - label.size().x * 0.5,
                canvas.center().y + (28.0 + 30.0 * slot as f32) * unit,
            );
            painter.rect_filled(
                Rect::from_min_size(position, label.size()).expand(7.0 * unit),
                4.0 * unit,
                PANEL,
            );
            painter.galley(position, label, Color32::WHITE);
        }
    }
}

impl Editor {
    pub(crate) fn hud_window(&mut self, ctx: &egui::Context) {
        let widgets = &mut self.doc.scene.hud;
        let assets = &self.doc.scene.assets;
        let editable = self.play.is_none();
        if self.hud_open
            && !widgets
                .iter()
                .any(|widget| Some(widget.id) == self.hud_selection)
        {
            self.hud_selection = widgets.first().map(|widget| widget.id);
        }
        let position = self.game_rect.map_or(Pos2::new(16.0, 96.0), |rect| {
            let right_limit = ctx.content_rect().right() - 356.0;
            Pos2::new((rect.right() + 8.0).min(right_limit).max(0.0), rect.top())
        });
        egui::Window::new("Game HUD")
            .open(&mut self.hud_open)
            .default_width(345.0)
            .default_pos(position)
            .show(ctx, |ui| {
                ui.add_enabled_ui(editable, |ui| {
                    ui.add_enabled_ui(widgets.len() < MAX_HUD_WIDGETS, |ui| {
                        ui.menu_button("+ Add widget", |ui| {
                            for kind in HudKind::ALL {
                                if ui.button(kind.label()).clicked() {
                                    let widget = HudWidget::new(kind);
                                    self.hud_selection = Some(widget.id);
                                    widgets.push(widget);
                                    ui.close();
                                }
                            }
                        });
                    });
                    egui::ScrollArea::vertical()
                        .max_height(126.0)
                        .show(ui, |ui| {
                            for widget in &*widgets {
                                let label = if widget.label.is_empty() {
                                    widget.kind.label()
                                } else {
                                    &widget.label
                                };
                                ui.selectable_value(
                                    &mut self.hud_selection,
                                    Some(widget.id),
                                    label,
                                );
                            }
                        });
                    let Some(index) = widgets
                        .iter()
                        .position(|widget| Some(widget.id) == self.hud_selection)
                    else {
                        return;
                    };
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(index > 0, egui::Button::new("Earlier"))
                            .clicked()
                        {
                            widgets.swap(index - 1, index);
                        } else if ui
                            .add_enabled(index + 1 < widgets.len(), egui::Button::new("Later"))
                            .clicked()
                        {
                            widgets.swap(index, index + 1);
                        } else if ui.button("Remove").clicked() {
                            widgets.remove(index);
                            self.hud_selection = widgets
                                .get(index.min(widgets.len().saturating_sub(1)))
                                .map(|widget| widget.id);
                        }
                    });
                    if let Some(widget) = widgets
                        .iter_mut()
                        .find(|widget| Some(widget.id) == self.hud_selection)
                    {
                        ui.separator();
                        let id = widget.id;
                        ui.push_id(id, |ui| {
                            widget_fields(ui, widget);
                            if widget.kind == HudKind::Image {
                                let label = assets
                                    .iter()
                                    .find(|asset| Some(asset.id) == widget.image)
                                    .map_or("None", |asset| asset.name.as_str());
                                egui::ComboBox::from_label("PNG image")
                                    .selected_text(label)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut widget.image, None, "None");
                                        for asset in assets
                                            .iter()
                                            .filter(|asset| asset.kind == AssetKind::Texture)
                                        {
                                            ui.selectable_value(
                                                &mut widget.image,
                                                Some(asset.id),
                                                &asset.name,
                                            );
                                        }
                                    });
                            }
                        });
                    }
                });
            });
    }
}

fn widget_fields(ui: &mut egui::Ui, widget: &mut HudWidget) {
    ui.label("Label");
    ui.add(egui::TextEdit::singleline(&mut widget.label).char_limit(32));
    if widget.kind != HudKind::Image {
        ui.label(if widget.kind == HudKind::Button {
            "Button action"
        } else {
            "Value binding"
        });
        if ui
            .add(egui::TextEdit::singleline(&mut widget.binding).char_limit(64))
            .changed()
        {
            widget
                .binding
                .retain(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
        }
    }
    let anchors = [
        (HudAnchor::TopLeft, "Top left"),
        (HudAnchor::TopRight, "Top right"),
        (HudAnchor::BottomLeft, "Bottom left"),
        (HudAnchor::BottomRight, "Bottom right"),
        (HudAnchor::Center, "Center"),
    ];
    let current = anchors
        .into_iter()
        .find(|(anchor, _)| *anchor == widget.anchor)
        .map_or("Top left", |(_, label)| label);
    egui::ComboBox::from_label("Anchor")
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (anchor, label) in anchors {
                ui.selectable_value(&mut widget.anchor, anchor, label);
            }
        });
    ui.horizontal(|ui| {
        ui.label("Offset");
        ui.add(
            egui::DragValue::new(&mut widget.offset.x)
                .range(-4096.0..=4096.0)
                .prefix("X "),
        );
        ui.add(
            egui::DragValue::new(&mut widget.offset.y)
                .range(-4096.0..=4096.0)
                .prefix("Y "),
        );
    });
    ui.horizontal(|ui| {
        ui.label("Size");
        ui.add(
            egui::DragValue::new(&mut widget.size.x)
                .range(4.0..=4096.0)
                .prefix("W "),
        );
        ui.add(
            egui::DragValue::new(&mut widget.size.y)
                .range(4.0..=4096.0)
                .prefix("H "),
        );
    });
    ui.horizontal(|ui| {
        ui.label("Color");
        ui.color_edit_button_srgb(&mut widget.color);
    });
    if matches!(widget.kind, HudKind::Text | HudKind::Button) {
        ui.add(
            egui::DragValue::new(&mut widget.text_size)
                .range(8.0..=96.0)
                .prefix("Text size "),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn right_anchor_keeps_its_margin_and_scales_in_small_game_views() {
        let mut widget = HudWidget::new(HudKind::Text);
        widget.anchor = HudAnchor::TopRight;
        for (size, margin, width) in [
            (vec2(1280.0, 720.0), 16.0, 144.0),
            (vec2(320.0, 180.0), 8.0, 72.0),
        ] {
            let canvas = Rect::from_min_size(Pos2::new(12.0, 24.0), size);
            let rect = bounds(canvas, &widget);
            assert_eq!(canvas.right() - rect.right(), margin);
            assert_eq!(rect.top() - canvas.top(), margin);
            assert_eq!(rect.width(), width);
        }
    }
}
