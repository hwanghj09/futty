use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2, pos2, vec2};
use futty_runtime::{MAGAZINE, ROUND_SECONDS, RangeInput, RangeStatus};

const INK: Color32 = Color32::from_rgb(12, 22, 29);
const MINT: Color32 = Color32::from_rgb(108, 239, 206);
const AMBER: Color32 = Color32::from_rgb(255, 164, 94);
const PAPER: Color32 = Color32::from_rgb(239, 246, 244);

#[derive(Default)]
pub(crate) struct RangeControls {
    grabbed: bool,
    fresh: bool,
}

impl RangeControls {
    pub(crate) fn input(&mut self, ctx: &egui::Context, visible: bool) -> RangeInput {
        if self.grabbed
            && (!visible
                || ctx.input(|input| !input.focused || input.key_pressed(egui::Key::Escape)))
        {
            self.grabbed = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::CursorGrab(egui::CursorGrab::None));
            ctx.send_viewport_cmd(egui::ViewportCommand::CursorVisible(true));
        }
        if !self.grabbed {
            return RangeInput::default();
        }
        let fresh = std::mem::take(&mut self.fresh);
        ctx.input(|input| {
            let motion = input
                .pointer
                .motion()
                .unwrap_or_else(|| input.pointer.delta());
            RangeInput {
                active: true,
                look: if fresh {
                    glam::Vec2::ZERO
                } else {
                    glam::vec2(motion.x, motion.y)
                },
                fire: !fresh
                    && (input.pointer.primary_down()
                        || input.pointer.button_pressed(egui::PointerButton::Primary)),
                reload: input.key_pressed(egui::Key::R),
                retry: input.key_pressed(egui::Key::Enter),
            }
        })
    }

    pub(crate) fn paint(&mut self, ui: &egui::Ui, response: &egui::Response, status: RangeStatus) {
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, true, "Activate Pulse Range")
        });
        if !self.grabbed && response.clicked() {
            self.grabbed = true;
            self.fresh = true;
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CursorGrab(
                    egui::CursorGrab::Confined,
                ));
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CursorVisible(false));
        }

        let rect = response.rect;
        let painter = ui.painter().with_clip_rect(rect);
        let padding = if rect.width() < 480.0 { 8.0 } else { 17.0 };
        let score = Rect::from_min_size(rect.min + Vec2::splat(padding), vec2(122.0, 54.0));
        plate(
            &painter,
            score,
            MINT,
            "SCORE",
            &format!("{:04}", status.points),
        );
        if status.streak > 1 {
            painter.text(
                score.left_bottom() + vec2(0.0, 8.0),
                Align2::LEFT_TOP,
                format!("{} HITS IN A ROW", status.streak),
                FontId::monospace(12.0),
                MINT,
            );
        }
        let clock = Rect::from_min_size(
            pos2(rect.right() - padding - 108.0, rect.top() + padding),
            vec2(108.0, 54.0),
        );
        let time_color = if status.remaining < 10.0 {
            AMBER
        } else {
            PAPER
        };
        plate(
            &painter,
            clock,
            time_color,
            "TIME LEFT",
            &format!("00:{:02}", status.remaining.ceil() as u8),
        );
        let meter = Rect::from_min_size(
            clock.left_bottom(),
            vec2(clock.width() * (status.remaining / ROUND_SECONDS), 3.0),
        );
        painter.rect_filled(meter, 0.0, time_color);

        let ammo = Rect::from_min_size(
            rect.right_bottom() - vec2(padding + 122.0, padding + 56.0),
            vec2(122.0, 56.0),
        );
        let ammo_color = if status.ammo == 0 || status.reloading {
            AMBER
        } else {
            MINT
        };
        let ammo_label = if status.reloading {
            "RECHARGING"
        } else if status.ammo == 0 {
            "R  RELOAD"
        } else {
            "PULSE CELLS"
        };
        plate(
            &painter,
            ammo,
            ammo_color,
            ammo_label,
            &format!("{:02} / {MAGAZINE:02}", status.ammo),
        );
        if self.grabbed && status.remaining > 0.0 {
            crosshair(&painter, rect.center(), status.hit_flash);
            if rect.width() > 505.0 {
                painter.text(
                    rect.left_bottom() + vec2(padding, -padding),
                    Align2::LEFT_BOTTOM,
                    "WASD MOVE    R RELOAD    ESC PAUSE",
                    FontId::monospace(11.0),
                    PAPER,
                );
            }
        } else {
            self.paint_card(&painter, rect, status);
        }
    }

    fn paint_card(&self, painter: &egui::Painter, rect: Rect, status: RangeStatus) {
        let finished = status.remaining <= 0.0;
        let fresh = status.remaining >= ROUND_SECONDS;
        let card = Rect::from_center_size(
            rect.center(),
            vec2((rect.width() - 16.0).clamp(0.0, 420.0), 158.0),
        );
        painter.rect_filled(card, 10.0, INK.gamma_multiply(0.94));
        painter.line_segment(
            [
                card.left_top() + vec2(12.0, 1.0),
                card.right_top() - vec2(12.0, -1.0),
            ],
            Stroke::new(2.0, MINT),
        );
        let title = if finished {
            "ROUND COMPLETE"
        } else if fresh {
            "PULSE RANGE"
        } else {
            "PAUSED"
        };
        let detail = if finished {
            let accuracy = (status.hits * 100).checked_div(status.shots).unwrap_or(0);
            format!(
                "{} POINTS   /   {} HITS   /   {}% ACC",
                status.points, status.hits, accuracy
            )
        } else {
            "WASD Move   /   Mouse Aim   /   Click Fire".into()
        };
        let prompt = if finished && self.grabbed {
            "ENTER  PLAY AGAIN"
        } else if finished {
            "CLICK, THEN ENTER TO RETRY"
        } else if fresh {
            "CLICK TO START"
        } else {
            "CLICK TO RESUME"
        };
        let font_size = if card.width() >= 345.0 { 31.0 } else { 22.0 };
        for (y, text, font, color) in [
            (38.0, title, FontId::proportional(font_size), PAPER),
            (77.0, detail.as_str(), FontId::monospace(11.0), PAPER),
            (117.0, prompt, FontId::monospace(15.0), MINT),
        ] {
            painter.text(
                pos2(card.center().x, card.top() + y),
                Align2::CENTER_CENTER,
                text,
                font,
                color,
            );
        }
    }
}

fn plate(painter: &egui::Painter, rect: Rect, accent: Color32, label: &str, value: &str) {
    painter.rect_filled(rect, 6.0, INK.gamma_multiply(0.91));
    let x = rect.left() + 11.0;
    painter.text(
        pos2(x, rect.top() + 7.0),
        Align2::LEFT_TOP,
        label,
        FontId::monospace(11.0),
        accent,
    );
    painter.text(
        pos2(x, rect.top() + 22.0),
        Align2::LEFT_TOP,
        value,
        FontId::monospace(21.0),
        PAPER,
    );
}

fn crosshair(painter: &egui::Painter, center: Pos2, hit: bool) {
    let axes = if hit {
        [vec2(0.71, 0.71), vec2(-0.71, 0.71)]
    } else {
        [Vec2::X, Vec2::Y]
    };
    for axis in axes.into_iter().flat_map(|axis| [axis, -axis]) {
        let line = [
            center + axis * 5.0,
            center + axis * if hit { 13.0 } else { 10.0 },
        ];
        painter.line_segment(line, Stroke::new(3.0, INK));
        painter.line_segment(line, Stroke::new(1.25, if hit { MINT } else { PAPER }));
    }
    painter.circle_filled(center, 1.25, if hit { MINT } else { PAPER });
}
